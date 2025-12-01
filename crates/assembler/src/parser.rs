use {
    crate::{
        ast::AST,
        astnode::{ASTNode, ExternDecl, GlobalDecl, Label, ROData, RodataDecl},
        dynsym::{DynamicSymbolMap, RelDynMap},
        errors::CompileError,
        section::{CodeSection, DataSection},
    },
    either::Either,
    pest::{Parser, iterators::Pair},
    pest_derive::Parser,
    sbpf_common::{
        inst_param::{Number, Register},
        instruction::Instruction,
        opcode::Opcode,
    },
    std::{collections::HashMap, str::FromStr},
};
#[derive(Parser)]
#[grammar = "sbpf.pest"]
pub struct SbpfParser;

/// BPF_X flag: Converts immediate variant opcodes to register variant opcodes
const BPF_X: u8 = 0x08;

/// Token types used in the AST
#[derive(Debug, Clone)]
pub enum Token {
    Directive(String, std::ops::Range<usize>),
    Identifier(String, std::ops::Range<usize>),
    ImmediateValue(Number, std::ops::Range<usize>),
    StringLiteral(String, std::ops::Range<usize>),
    VectorLiteral(Vec<Number>, std::ops::Range<usize>),
}

pub struct ParseResult {
    // TODO: parse result is basically 1. static part 2. dynamic part of the program
    pub code_section: CodeSection,

    pub data_section: DataSection,

    pub dynamic_symbols: DynamicSymbolMap,

    pub relocation_data: RelDynMap,

    // TODO: this can be removed and dynamic-ness should just be
    // determined by if there's any dynamic symbol
    pub prog_is_static: bool,
}

pub fn parse(source: &str) -> Result<ParseResult, Vec<CompileError>> {
    let pairs = SbpfParser::parse(Rule::program, source).map_err(|e| {
        vec![CompileError::ParseError {
            error: e.to_string(),
            span: 0..source.len(),
            custom_label: None,
        }]
    })?;

    let mut ast = AST::new();
    let mut const_map = HashMap::<String, Number>::new();
    let mut label_spans = HashMap::<String, std::ops::Range<usize>>::new();
    let mut rodata_phase = false;
    let mut text_offset = 0u64;
    let mut rodata_offset = 0u64;
    let mut errors = Vec::new();
    let mut missing_text_directive = false;

    for pair in pairs {
        if pair.as_rule() == Rule::program {
            for statement in pair.into_inner() {
                if statement.as_rule() == Rule::EOI {
                    continue;
                }

                match process_statement(
                    statement,
                    &mut ast,
                    &mut const_map,
                    &mut label_spans,
                    &mut rodata_phase,
                    &mut text_offset,
                    &mut rodata_offset,
                    &mut missing_text_directive,
                ) {
                    Ok(_) => {}
                    Err(e) => errors.push(e),
                }
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    ast.set_text_size(text_offset);
    ast.set_rodata_size(rodata_offset);

    ast.build_program()
}

fn process_statement(
    pair: Pair<Rule>,
    ast: &mut AST,
    const_map: &mut HashMap<String, Number>,
    label_spans: &mut HashMap<String, std::ops::Range<usize>>,
    rodata_phase: &mut bool,
    text_offset: &mut u64,
    rodata_offset: &mut u64,
    missing_text_directive: &mut bool,
) -> Result<(), CompileError> {
    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::label => {
                let mut label_opt = None;
                let mut directive_opt = None;
                let mut instruction_opt = None;

                for item in inner.into_inner() {
                    match item.as_rule() {
                        Rule::identifier | Rule::numeric_label => {
                            label_opt = Some(extract_label_from_pair(item)?);
                        }
                        Rule::directive_inner => {
                            directive_opt = Some(item);
                        }
                        Rule::instruction => {
                            instruction_opt = Some(item);
                        }
                        _ => {}
                    }
                }

                if let Some((label_name, label_span)) = label_opt {
                    // Check for duplicate labels
                    if let Some(original_span) = label_spans.get(&label_name) {
                        return Err(CompileError::DuplicateLabel {
                            label: label_name,
                            span: label_span,
                            original_span: original_span.clone(),
                            custom_label: Some("Label already defined".to_string()),
                        });
                    }
                    label_spans.insert(label_name.clone(), label_span.clone());

                    if *rodata_phase {
                        // Handle rodata label with directive
                        if let Some(dir_pair) = directive_opt {
                            let rodata = process_rodata_directive(
                                label_name.clone(),
                                label_span.clone(),
                                dir_pair,
                            )?;
                            let size = rodata.get_size();
                            ast.rodata_nodes.push(ASTNode::ROData {
                                rodata,
                                offset: *rodata_offset,
                            });
                            *rodata_offset += size;
                        } else if instruction_opt.is_some() && !*missing_text_directive {
                            // Error - instruction found after .rodata without .text
                            *missing_text_directive = true;
                            return Err(CompileError::MissingTextDirective {
                                span: label_span,
                                custom_label: None,
                            });
                        }
                    } else {
                        ast.nodes.push(ASTNode::Label {
                            label: Label {
                                name: label_name,
                                span: label_span,
                            },
                            offset: *text_offset,
                        });

                        if let Some(inst_pair) = instruction_opt {
                            let instruction = process_instruction(inst_pair, const_map)?;
                            let size = instruction.get_size();
                            ast.nodes.push(ASTNode::Instruction {
                                instruction,
                                offset: *text_offset,
                            });
                            *text_offset += size;
                        }
                    }
                }
            }
            Rule::directive => {
                process_directive_statement(inner, ast, const_map, rodata_phase)?;
            }
            Rule::instruction => {
                if *rodata_phase && !*missing_text_directive {
                    *missing_text_directive = true;
                    let span = inner.as_span();
                    return Err(CompileError::MissingTextDirective {
                        span: span.start()..span.end(),
                        custom_label: None,
                    });
                }
                let instruction = process_instruction(inner, const_map)?;
                let size = instruction.get_size();
                ast.nodes.push(ASTNode::Instruction {
                    instruction,
                    offset: *text_offset,
                });
                *text_offset += size;
            }
            _ => {}
        }
    }
    Ok(())
}

fn extract_label_from_pair(
    pair: Pair<Rule>,
) -> Result<(String, std::ops::Range<usize>), CompileError> {
    let span = pair.as_span();
    Ok((pair.as_str().to_string(), span.start()..span.end()))
}

fn process_directive_statement(
    pair: Pair<Rule>,
    ast: &mut AST,
    const_map: &mut HashMap<String, Number>,
    rodata_phase: &mut bool,
) -> Result<(), CompileError> {
    for directive_inner_pair in pair.into_inner() {
        process_directive_inner(directive_inner_pair, ast, const_map, rodata_phase)?;
    }
    Ok(())
}

fn process_directive_inner(
    pair: Pair<Rule>,
    ast: &mut AST,
    const_map: &mut HashMap<String, Number>,
    rodata_phase: &mut bool,
) -> Result<(), CompileError> {
    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::directive_globl => {
                let span = inner.as_span();
                for globl_inner in inner.into_inner() {
                    if globl_inner.as_rule() == Rule::globl_symbol {
                        let entry_label = globl_inner.as_str().to_string();
                        ast.entry_label = Some(entry_label.clone());
                        ast.nodes.push(ASTNode::GlobalDecl {
                            global_decl: GlobalDecl {
                                entry_label,
                                span: span.start()..span.end(),
                            },
                        });
                    }
                }
            }
            Rule::directive_extern => {
                let span = inner.as_span();
                let mut symbols = Vec::new();
                for extern_inner in inner.into_inner() {
                    if extern_inner.as_rule() == Rule::symbol {
                        let symbol_span = extern_inner.as_span();
                        symbols.push(Token::Identifier(
                            extern_inner.as_str().to_string(),
                            symbol_span.start()..symbol_span.end(),
                        ));
                    }
                }
                ast.nodes.push(ASTNode::ExternDecl {
                    extern_decl: ExternDecl {
                        args: symbols,
                        span: span.start()..span.end(),
                    },
                });
            }
            Rule::directive_equ => {
                let mut ident = None;
                let mut value = None;

                for equ_inner in inner.into_inner() {
                    match equ_inner.as_rule() {
                        Rule::identifier => {
                            ident = Some(equ_inner.as_str().to_string());
                        }
                        Rule::expression => {
                            value = Some(eval_expression(equ_inner, const_map)?);
                        }
                        _ => {}
                    }
                }

                if let (Some(name), Some(val)) = (ident, value) {
                    const_map.insert(name, val);
                }
            }
            Rule::directive_section => {
                let section_name = inner.as_str().trim_start_matches('.');
                match section_name {
                    "text" => *rodata_phase = false,
                    "rodata" => {
                        *rodata_phase = true;
                        let span = inner.as_span();
                        ast.nodes.push(ASTNode::RodataDecl {
                            rodata_decl: RodataDecl {
                                span: span.start()..span.end(),
                            },
                        });
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn process_rodata_directive(
    label_name: String,
    label_span: std::ops::Range<usize>,
    pair: Pair<Rule>,
) -> Result<ROData, CompileError> {
    let inner_pair = if pair.as_rule() == Rule::directive_inner {
        pair
    } else {
        pair.into_inner()
            .next()
            .ok_or_else(|| CompileError::ParseError {
                error: "No directive content found".to_string(),
                span: label_span.clone(),
                custom_label: None,
            })?
    };

    for inner in inner_pair.into_inner() {
        let directive_span = inner.as_span();

        match inner.as_rule() {
            Rule::directive_ascii => {
                for ascii_inner in inner.into_inner() {
                    if ascii_inner.as_rule() == Rule::string_literal {
                        for content_inner in ascii_inner.into_inner() {
                            if content_inner.as_rule() == Rule::string_content {
                                let content = content_inner.as_str().to_string();
                                let content_span = content_inner.as_span();
                                return Ok(ROData {
                                    name: label_name,
                                    args: vec![
                                        Token::Directive(
                                            "ascii".to_string(),
                                            directive_span.start()..directive_span.end(),
                                        ),
                                        Token::StringLiteral(
                                            content,
                                            content_span.start()..content_span.end(),
                                        ),
                                    ],
                                    span: label_span,
                                });
                            }
                        }
                    }
                }
            }
            Rule::directive_byte
            | Rule::directive_word
            | Rule::directive_long
            | Rule::directive_quad => {
                let directive_name = match inner.as_rule() {
                    Rule::directive_byte => "byte",
                    Rule::directive_word => "word",
                    Rule::directive_long => "long",
                    Rule::directive_quad => "quad",
                    _ => "byte",
                };

                let mut values = Vec::new();
                for byte_inner in inner.into_inner() {
                    if byte_inner.as_rule() == Rule::number {
                        values.push(parse_number(byte_inner)?);
                    }
                }

                let values_span = directive_span.start()..directive_span.end();
                return Ok(ROData {
                    name: label_name,
                    args: vec![
                        Token::Directive(
                            directive_name.to_string(),
                            directive_span.start()..directive_span.end(),
                        ),
                        Token::VectorLiteral(values, values_span),
                    ],
                    span: label_span,
                });
            }
            _ => {}
        }
    }

    Err(CompileError::InvalidRodataDecl {
        span: label_span,
        custom_label: None,
    })
}

fn process_instruction(
    pair: Pair<Rule>,
    const_map: &HashMap<String, Number>,
) -> Result<Instruction, CompileError> {
    let outer_span = pair.as_span();
    let outer_span_range = outer_span.start()..outer_span.end();

    for inner in pair.into_inner() {
        let span = inner.as_span();
        let span_range = span.start()..span.end();

        match inner.as_rule() {
            Rule::instr_exit => {
                return Ok(Instruction {
                    opcode: Opcode::Exit,
                    dst: None,
                    src: None,
                    off: None,
                    imm: None,
                    span: span_range,
                });
            }
            Rule::instr_lddw => return process_lddw(inner, const_map, span_range),
            Rule::instr_call => return process_call(inner, const_map, span_range),
            Rule::instr_callx => return process_callx(inner, span_range),
            Rule::instr_neg32 => return process_neg32(inner, span_range),
            Rule::instr_neg64 => return process_neg64(inner, span_range),
            Rule::instr_alu64_imm | Rule::instr_alu32_imm => {
                return process_alu_imm(inner, const_map, span_range);
            }
            Rule::instr_alu64_reg | Rule::instr_alu32_reg => {
                return process_alu_reg(inner, span_range);
            }
            Rule::instr_load => return process_load(inner, const_map, span_range),
            Rule::instr_store_imm => return process_store_imm(inner, const_map, span_range),
            Rule::instr_store_reg => return process_store_reg(inner, const_map, span_range),
            Rule::instr_jump_imm => return process_jump_imm(inner, const_map, span_range),
            Rule::instr_jump_reg => return process_jump_reg(inner, span_range),
            Rule::instr_jump_uncond => return process_jump_uncond(inner, const_map, span_range),
            Rule::instr_endian => return process_endian(inner, span_range),
            _ => {}
        }
    }

    Err(CompileError::ParseError {
        error: "Invalid instruction".to_string(),
        span: outer_span_range,
        custom_label: None,
    })
}

fn process_lddw(
    pair: Pair<Rule>,
    const_map: &HashMap<String, Number>,
    span: std::ops::Range<usize>,
) -> Result<Instruction, CompileError> {
    let mut dst = None;
    let mut imm = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::register => dst = Some(parse_register(inner)?),
            Rule::operand => imm = Some(parse_operand(inner, const_map)?),
            _ => {}
        }
    }

    Ok(Instruction {
        opcode: Opcode::Lddw,
        dst,
        src: None,
        off: None,
        imm,
        span,
    })
}

fn process_load(
    pair: Pair<Rule>,
    const_map: &HashMap<String, Number>,
    span: std::ops::Range<usize>,
) -> Result<Instruction, CompileError> {
    let mut opcode = None;
    let mut dst = None;
    let mut src = None;
    let mut off = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::load_op => opcode = Opcode::from_str(inner.as_str()).ok(),
            Rule::register => dst = Some(parse_register(inner)?),
            Rule::memory_ref => {
                let (s, o) = parse_memory_ref(inner, const_map)?;
                src = Some(s);
                off = Some(o);
            }
            _ => {}
        }
    }

    Ok(Instruction {
        opcode: opcode.unwrap_or(Opcode::Exit),
        dst,
        src,
        off,
        imm: None,
        span,
    })
}

fn process_store_imm(
    pair: Pair<Rule>,
    const_map: &HashMap<String, Number>,
    span: std::ops::Range<usize>,
) -> Result<Instruction, CompileError> {
    let mut opcode = None;
    let mut dst = None;
    let mut off = None;
    let mut imm = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::store_op => opcode = Opcode::from_str(inner.as_str()).ok(),
            Rule::memory_ref => {
                let (d, o) = parse_memory_ref(inner, const_map)?;
                dst = Some(d);
                off = Some(o);
            }
            Rule::operand => imm = Some(parse_operand(inner, const_map)?),
            _ => {}
        }
    }

    Ok(Instruction {
        opcode: opcode.unwrap_or(Opcode::Exit),
        dst,
        src: None,
        off,
        imm,
        span,
    })
}

fn process_store_reg(
    pair: Pair<Rule>,
    const_map: &HashMap<String, Number>,
    span: std::ops::Range<usize>,
) -> Result<Instruction, CompileError> {
    let mut opcode = None;
    let mut dst = None;
    let mut src = None;
    let mut off = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::store_op => opcode = Opcode::from_str(inner.as_str()).ok(),
            Rule::memory_ref => {
                let (d, o) = parse_memory_ref(inner, const_map)?;
                dst = Some(d);
                off = Some(o);
            }
            Rule::register => src = Some(parse_register(inner)?),
            _ => {}
        }
    }

    Ok(Instruction {
        opcode: opcode.unwrap_or(Opcode::Exit),
        dst,
        src,
        off,
        imm: None,
        span,
    })
}

fn process_alu_imm(
    pair: Pair<Rule>,
    const_map: &HashMap<String, Number>,
    span: std::ops::Range<usize>,
) -> Result<Instruction, CompileError> {
    let mut opcode = None;
    let mut dst = None;
    let mut imm = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::alu_64_op | Rule::alu_32_op => opcode = Opcode::from_str(inner.as_str()).ok(),
            Rule::register => dst = Some(parse_register(inner)?),
            Rule::operand => imm = Some(parse_operand(inner, const_map)?),
            _ => {}
        }
    }

    Ok(Instruction {
        opcode: opcode.unwrap_or(Opcode::Exit),
        dst,
        src: None,
        off: None,
        imm,
        span,
    })
}

fn process_alu_reg(
    pair: Pair<Rule>,
    span: std::ops::Range<usize>,
) -> Result<Instruction, CompileError> {
    let mut opcode = None;
    let mut dst = None;
    let mut src = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::alu_64_op | Rule::alu_32_op => {
                let op_str = inner.as_str();
                let inner_span = inner.as_span();
                if let Ok(opc) = Opcode::from_str(op_str) {
                    // Convert to register variant using BPF_X flag
                    let reg_opcode = Into::<u8>::into(opc) | BPF_X;
                    opcode =
                        Some(
                            reg_opcode
                                .try_into()
                                .map_err(|e| CompileError::BytecodeError {
                                    error: format!("Invalid opcode 0x{:02x}: {}", reg_opcode, e),
                                    span: inner_span.start()..inner_span.end(),
                                    custom_label: None,
                                })?,
                        );
                }
            }
            Rule::register => {
                if dst.is_none() {
                    dst = Some(parse_register(inner)?);
                } else {
                    src = Some(parse_register(inner)?);
                }
            }
            _ => {}
        }
    }

    Ok(Instruction {
        opcode: opcode.unwrap_or(Opcode::Exit),
        dst,
        src,
        off: None,
        imm: None,
        span,
    })
}

fn process_jump_imm(
    pair: Pair<Rule>,
    const_map: &HashMap<String, Number>,
    span: std::ops::Range<usize>,
) -> Result<Instruction, CompileError> {
    let mut opcode = None;
    let mut dst = None;
    let mut imm = None;
    let mut off = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::jump_op => opcode = Opcode::from_str(inner.as_str()).ok(),
            Rule::register => dst = Some(parse_register(inner)?),
            Rule::operand => imm = Some(parse_operand(inner, const_map)?),
            Rule::jump_target => off = Some(parse_jump_target(inner, const_map)?),
            _ => {}
        }
    }

    Ok(Instruction {
        opcode: opcode.unwrap_or(Opcode::Exit),
        dst,
        src: None,
        off,
        imm,
        span,
    })
}

fn process_jump_reg(
    pair: Pair<Rule>,
    span: std::ops::Range<usize>,
) -> Result<Instruction, CompileError> {
    let mut opcode = None;
    let mut dst = None;
    let mut src = None;
    let mut off = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::jump_op => {
                let op_str = inner.as_str();
                let inner_span = inner.as_span();
                if let Ok(opc) = Opcode::from_str(op_str) {
                    // Convert Imm variant to Reg variant using BPF_X flag
                    let reg_opcode = Into::<u8>::into(opc) | BPF_X;
                    opcode =
                        Some(
                            reg_opcode
                                .try_into()
                                .map_err(|e| CompileError::BytecodeError {
                                    error: format!("Invalid opcode 0x{:02x}: {}", reg_opcode, e),
                                    span: inner_span.start()..inner_span.end(),
                                    custom_label: None,
                                })?,
                        );
                }
            }
            Rule::register => {
                if dst.is_none() {
                    dst = Some(parse_register(inner)?);
                } else {
                    src = Some(parse_register(inner)?);
                }
            }
            Rule::jump_target => off = Some(parse_jump_target(inner, &HashMap::new())?),
            _ => {}
        }
    }

    Ok(Instruction {
        opcode: opcode.unwrap_or(Opcode::Exit),
        dst,
        src,
        off,
        imm: None,
        span,
    })
}

fn process_jump_uncond(
    pair: Pair<Rule>,
    const_map: &HashMap<String, Number>,
    span: std::ops::Range<usize>,
) -> Result<Instruction, CompileError> {
    let mut off = None;

    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::jump_target {
            off = Some(parse_jump_target(inner, const_map)?);
        }
    }

    Ok(Instruction {
        opcode: Opcode::Ja,
        dst: None,
        src: None,
        off,
        imm: None,
        span,
    })
}

fn process_call(
    pair: Pair<Rule>,
    const_map: &HashMap<String, Number>,
    span: std::ops::Range<usize>,
) -> Result<Instruction, CompileError> {
    let mut imm = None;

    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::symbol {
            if let Some(symbol) = const_map.get(inner.as_str()) {
                imm = Some(Either::Right(symbol.to_owned()));
            } else {
                imm = Some(Either::Left(inner.as_str().to_string()));
            }
        }
    }

    Ok(Instruction {
        opcode: Opcode::Call,
        dst: None,
        src: None,
        off: None,
        imm,
        span,
    })
}

fn process_callx(
    pair: Pair<Rule>,
    span: std::ops::Range<usize>,
) -> Result<Instruction, CompileError> {
    let mut dst = None;

    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::register {
            dst = Some(parse_register(inner)?);
        }
    }

    Ok(Instruction {
        opcode: Opcode::Callx,
        dst,
        src: None,
        off: None,
        imm: None,
        span,
    })
}

fn process_neg32(
    pair: Pair<Rule>,
    span: std::ops::Range<usize>,
) -> Result<Instruction, CompileError> {
    let mut dst = None;

    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::register {
            dst = Some(parse_register(inner)?);
        }
    }

    Ok(Instruction {
        opcode: Opcode::Neg32,
        dst,
        src: None,
        off: None,
        imm: None,
        span,
    })
}

fn process_neg64(
    pair: Pair<Rule>,
    span: std::ops::Range<usize>,
) -> Result<Instruction, CompileError> {
    let mut dst = None;

    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::register {
            dst = Some(parse_register(inner)?);
        }
    }

    Ok(Instruction {
        opcode: Opcode::Neg64,
        dst,
        src: None,
        off: None,
        imm: None,
        span,
    })
}

fn process_endian(
    pair: Pair<Rule>,
    span: std::ops::Range<usize>,
) -> Result<Instruction, CompileError> {
    let mut opcode = None;
    let mut dst = None;
    let mut imm = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::endian_op => {
                let op_str = inner.as_str();
                let inner_span = inner.as_span();
                // Extract opcode and size from instruction (example: "be16" = be opcode, 16 bits)
                let (opc, size) = if let Some(size_str) = op_str.strip_prefix("be") {
                    let size = size_str
                        .parse::<i64>()
                        .map_err(|_| CompileError::ParseError {
                            error: format!("Invalid endian size in '{}'", op_str),
                            span: inner_span.start()..inner_span.end(),
                            custom_label: None,
                        })?;
                    (Opcode::Be, size)
                } else if let Some(size_str) = op_str.strip_prefix("le") {
                    let size = size_str
                        .parse::<i64>()
                        .map_err(|_| CompileError::ParseError {
                            error: format!("Invalid endian size in '{}'", op_str),
                            span: inner_span.start()..inner_span.end(),
                            custom_label: None,
                        })?;
                    (Opcode::Le, size)
                } else {
                    return Err(CompileError::ParseError {
                        error: format!("Invalid endian operation '{}'", op_str),
                        span: inner_span.start()..inner_span.end(),
                        custom_label: None,
                    });
                };
                opcode = Some(opc);
                imm = Some(Either::Right(Number::Int(size)));
            }
            Rule::register => dst = Some(parse_register(inner)?),
            _ => {}
        }
    }

    Ok(Instruction {
        opcode: opcode.unwrap_or(Opcode::Exit),
        dst,
        src: None,
        off: None,
        imm,
        span,
    })
}

fn parse_register(pair: Pair<Rule>) -> Result<Register, CompileError> {
    let reg_str = pair.as_str();
    let span = pair.as_span();

    if let Ok(n) = reg_str[1..].parse::<u8>() {
        Ok(Register { n })
    } else {
        Err(CompileError::InvalidRegister {
            register: reg_str.to_string(),
            span: span.start()..span.end(),
            custom_label: None,
        })
    }
}

fn parse_operand(
    pair: Pair<Rule>,
    const_map: &HashMap<String, Number>,
) -> Result<Either<String, Number>, CompileError> {
    let span = pair.as_span();
    let span_range = span.start()..span.end();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::number => return Ok(Either::Right(parse_number(inner)?)),
            Rule::symbol => {
                let name = inner.as_str().to_string();
                if let Some(value) = const_map.get(&name) {
                    return Ok(Either::Right(value.clone()));
                }
                return Ok(Either::Left(name));
            }
            Rule::operand_expr => {
                let mut sym_name = None;
                let mut num_value = None;

                for expr_inner in inner.into_inner() {
                    match expr_inner.as_rule() {
                        Rule::symbol => sym_name = Some(expr_inner.as_str().to_string()),
                        Rule::number => num_value = Some(parse_number(expr_inner)?),
                        _ => {}
                    }
                }

                if let (Some(sym), Some(num)) = (sym_name, num_value) {
                    if let Some(base_value) = const_map.get(&sym) {
                        let result = base_value.clone() + num;
                        return Ok(Either::Right(result));
                    } else {
                        return Ok(Either::Left(sym));
                    }
                }
            }
            _ => {}
        }
    }

    Err(CompileError::ParseError {
        error: "Invalid operand".to_string(),
        span: span_range,
        custom_label: None,
    })
}

fn parse_jump_target(
    pair: Pair<Rule>,
    _const_map: &HashMap<String, Number>,
) -> Result<Either<String, i16>, CompileError> {
    let span = pair.as_span();
    let span_range = span.start()..span.end();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::symbol | Rule::numeric_label_ref => {
                return Ok(Either::Left(inner.as_str().to_string()));
            }
            Rule::number => {
                let num = parse_number(inner)?;
                return Ok(Either::Right(num.to_i16()));
            }
            _ => {}
        }
    }

    Err(CompileError::ParseError {
        error: "Invalid jump target".to_string(),
        span: span_range,
        custom_label: None,
    })
}

fn parse_memory_ref(
    pair: Pair<Rule>,
    const_map: &HashMap<String, Number>,
) -> Result<(Register, Either<String, i16>), CompileError> {
    let mut reg = None;
    let mut accumulated_offset: i16 = 0;
    let mut unresolved_symbol: Option<String> = None;
    let mut sign: i16 = 1;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::register => {
                reg = Some(parse_register(inner)?);
            }
            Rule::memory_op => {
                sign = if inner.as_str() == "+" { 1 } else { -1 };
            }
            Rule::memory_offset => {
                for offset_inner in inner.into_inner() {
                    match offset_inner.as_rule() {
                        Rule::number => {
                            let num = parse_number(offset_inner)?;
                            accumulated_offset =
                                accumulated_offset.wrapping_add(sign * num.to_i16());
                        }
                        Rule::symbol => {
                            let name = offset_inner.as_str().to_string();
                            if let Some(value) = const_map.get(&name) {
                                accumulated_offset =
                                    accumulated_offset.wrapping_add(sign * value.to_i16());
                            } else if unresolved_symbol.is_none() {
                                unresolved_symbol = Some(name);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    let offset = if let Some(sym) = unresolved_symbol {
        Either::Left(sym)
    } else {
        Either::Right(accumulated_offset)
    };

    Ok((reg.unwrap_or(Register { n: 0 }), offset))
}

fn parse_number(pair: Pair<Rule>) -> Result<Number, CompileError> {
    let span = pair.as_span();
    let span_range = span.start()..span.end();
    let number_str = pair.as_str().replace('_', "");

    let mut sign: i64 = 1;
    let value = if number_str.starts_with('-') {
        sign = -1;
        number_str.strip_prefix('-').unwrap()
    } else {
        number_str.as_str()
    };

    if value.starts_with("0x") {
        let hex_str = value.trim_start_matches("0x");
        if let Ok(value) = u64::from_str_radix(hex_str, 16) {
            return Ok(Number::Addr(sign * (value as i64)));
        }
    } else if let Ok(value) = value.parse::<i64>() {
        return Ok(Number::Int(sign * value));
    }

    Err(CompileError::InvalidNumber {
        number: number_str,
        span: span_range,
        custom_label: None,
    })
}

fn eval_expression(
    pair: Pair<Rule>,
    const_map: &HashMap<String, Number>,
) -> Result<Number, CompileError> {
    let span = pair.as_span();
    let span_range = span.start()..span.end();

    let mut stack = Vec::new();
    let mut op_stack = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::term => {
                let val = eval_term(inner, const_map)?;
                stack.push(val);
            }
            Rule::bin_op => {
                op_stack.push(inner.as_str());
            }
            _ => {}
        }
    }

    // Apply operators
    while let Some(op) = op_stack.pop() {
        if stack.len() >= 2 {
            let b = stack.pop().unwrap();
            let a = stack.pop().unwrap();
            let result = match op {
                "+" => a + b,
                "-" => a - b,
                "*" => a * b,
                "/" => a / b,
                _ => a,
            };
            stack.push(result);
        }
    }

    stack.pop().ok_or_else(|| CompileError::ParseError {
        error: "Invalid expression".to_string(),
        span: span_range,
        custom_label: None,
    })
}

fn eval_term(
    pair: Pair<Rule>,
    const_map: &HashMap<String, Number>,
) -> Result<Number, CompileError> {
    let span = pair.as_span();
    let span_range = span.start()..span.end();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::expression => {
                return eval_expression(inner, const_map);
            }
            Rule::number => {
                return parse_number(inner);
            }
            Rule::symbol => {
                let name = inner.as_str().to_string();
                if let Some(value) = const_map.get(&name) {
                    return Ok(value.clone());
                }
                return Err(CompileError::ParseError {
                    error: format!("Undefined constant: {}", name),
                    span: inner.as_span().start()..inner.as_span().end(),
                    custom_label: None,
                });
            }
            _ => {}
        }
    }

    Err(CompileError::ParseError {
        error: "Invalid term".to_string(),
        span: span_range,
        custom_label: None,
    })
}
