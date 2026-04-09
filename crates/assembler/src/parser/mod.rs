pub mod common;
mod default;
mod directive;
mod llvm;

use {
    crate::{
        SbpfArch,
        ast::AST,
        astnode::{ASTNode, Label},
        dynsym::{DynamicSymbolMap, RelDynMap},
        errors::CompileError,
        section::{CodeSection, DataSection, DebugSection},
    },
    directive::{process_directive_statement, process_rodata_directive},
    pest::{Parser, iterators::Pair},
    pest_derive::Parser,
    sbpf_common::{inst_param::Number, instruction::Instruction},
    std::collections::HashMap,
};

#[derive(Parser)]
#[grammar = "sbpf.pest"]
pub struct SbpfParser;

/// Context containing all mutable state during parsing
pub(crate) struct ParseContext<'a> {
    pub ast: &'a mut AST,
    pub const_map: &'a mut HashMap<String, Number>,
    pub label_spans: &'a mut HashMap<String, std::ops::Range<usize>>,
    pub errors: Vec<CompileError>,
    pub rodata_phase: bool,
    pub text_offset: u64,
    pub rodata_offset: u64,
    pub missing_text_directive: bool,
}

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

    pub arch: SbpfArch,

    // Debug sections we came across while byteparsing
    pub debug_sections: Vec<DebugSection>,
}

pub fn parse(source: &str, arch: SbpfArch) -> Result<ParseResult, Vec<CompileError>> {
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

    let (text_offset, rodata_offset, errors) = {
        let mut ctx = ParseContext {
            ast: &mut ast,
            const_map: &mut const_map,
            label_spans: &mut label_spans,
            errors: Vec::new(),
            rodata_phase: false,
            text_offset: 0,
            rodata_offset: 0,
            missing_text_directive: false,
        };

        for pair in pairs {
            match pair.as_rule() {
                Rule::program_default | Rule::program_llvm => {
                    for statement in pair.into_inner() {
                        if statement.as_rule() == Rule::EOI {
                            continue;
                        }
                        process_statement(statement, &mut ctx);
                    }
                }
                _ => {}
            }
        }

        (ctx.text_offset, ctx.rodata_offset, ctx.errors)
    };

    if !errors.is_empty() {
        return Err(errors);
    }

    ast.set_text_size(text_offset);
    ast.set_rodata_size(rodata_offset);

    ast.build_program(arch)
}

fn process_statement(pair: Pair<Rule>, ctx: &mut ParseContext) {
    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::label_default | Rule::label_llvm => {
                process_label(inner, ctx);
            }
            Rule::directive => {
                process_directive_statement(inner, ctx);
            }
            Rule::instr_default | Rule::instr_llvm => {
                let span = inner.as_span();
                let span_range = span.start()..span.end();
                let is_llvm = inner.as_rule() == Rule::instr_llvm;

                match process_instruction(inner, ctx.const_map, is_llvm) {
                    Ok(instruction) => {
                        if !ctx.rodata_phase {
                            let size = instruction.get_size();
                            ctx.ast.nodes.push(ASTNode::Instruction {
                                instruction,
                                offset: ctx.text_offset,
                            });
                            ctx.text_offset += size;
                        }
                    }
                    Err(e) => ctx.errors.push(e),
                }

                if ctx.rodata_phase && !ctx.missing_text_directive {
                    ctx.missing_text_directive = true;
                    ctx.errors.push(CompileError::MissingTextDirective {
                        span: span_range,
                        custom_label: None,
                    });
                }
            }
            _ => {}
        }
    }
}

fn process_label(pair: Pair<Rule>, ctx: &mut ParseContext) {
    let is_llvm = pair.as_rule() == Rule::label_llvm;
    let mut label_opt = None;
    let mut directive_opt = None;
    let mut instruction_opt = None;

    for item in pair.into_inner() {
        match item.as_rule() {
            Rule::identifier | Rule::numeric_label => match extract_label_from_pair(item) {
                Ok(label) => label_opt = Some(label),
                Err(e) => ctx.errors.push(e),
            },
            Rule::directive_inner => {
                directive_opt = Some(item);
            }
            Rule::instr_default | Rule::instr_llvm => {
                instruction_opt = Some(item);
            }
            _ => {}
        }
    }

    if let Some((label_name, label_span)) = label_opt {
        // Check for duplicate labels
        if let Some(original_span) = ctx.label_spans.get(&label_name) {
            ctx.errors.push(CompileError::DuplicateLabel {
                label: label_name,
                span: label_span,
                original_span: original_span.clone(),
                custom_label: Some("Label already defined".to_string()),
            });
            return;
        }
        ctx.label_spans
            .insert(label_name.clone(), label_span.clone());

        if ctx.rodata_phase {
            // Handle rodata label with directive
            if let Some(dir_pair) = directive_opt {
                match process_rodata_directive(label_name.clone(), label_span.clone(), dir_pair) {
                    Ok(rodata) => {
                        let size = rodata.get_size();
                        ctx.ast.rodata_nodes.push(ASTNode::ROData {
                            rodata,
                            offset: ctx.rodata_offset,
                        });
                        ctx.rodata_offset += size;
                    }
                    Err(e) => ctx.errors.push(e),
                }
            } else if let Some(inst_pair) = instruction_opt {
                if let Err(e) = process_instruction(inst_pair, ctx.const_map, is_llvm) {
                    ctx.errors.push(e);
                }
                if !ctx.missing_text_directive {
                    ctx.missing_text_directive = true;
                    ctx.errors.push(CompileError::MissingTextDirective {
                        span: label_span,
                        custom_label: None,
                    });
                }
            }
        } else {
            ctx.ast.nodes.push(ASTNode::Label {
                label: Label {
                    name: label_name,
                    span: label_span,
                },
                offset: ctx.text_offset,
            });

            if let Some(inst_pair) = instruction_opt {
                match process_instruction(inst_pair, ctx.const_map, is_llvm) {
                    Ok(instruction) => {
                        let size = instruction.get_size();
                        ctx.ast.nodes.push(ASTNode::Instruction {
                            instruction,
                            offset: ctx.text_offset,
                        });
                        ctx.text_offset += size;
                    }
                    Err(e) => ctx.errors.push(e),
                }
            }
        }
    }
}

fn process_instruction(
    pair: Pair<Rule>,
    const_map: &HashMap<String, Number>,
    is_llvm: bool,
) -> Result<Instruction, CompileError> {
    if is_llvm {
        llvm::process_instruction(pair, const_map)
    } else {
        default::process_instruction(pair, const_map)
    }
}

fn extract_label_from_pair(
    pair: Pair<Rule>,
) -> Result<(String, std::ops::Range<usize>), CompileError> {
    let span = pair.as_span();
    Ok((pair.as_str().to_string(), span.start()..span.end()))
}
