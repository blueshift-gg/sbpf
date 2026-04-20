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
    pest::{
        Parser,
        error::{ErrorVariant, InputLocation},
        iterators::Pair,
    },
    pest_derive::Parser,
    sbpf_common::{inst_param::Number, instruction::Instruction},
    std::collections::HashMap,
};

#[derive(Parser)]
#[grammar = "sbpf.pest"]
pub struct SbpfParser;

/// Which section a label belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Section {
    Text,
    Rodata,
}

/// Context containing all mutable state during parsing
pub(crate) struct ParseContext<'a> {
    pub ast: &'a mut AST,
    pub const_map: &'a mut HashMap<String, Number>,
    pub label_spans: &'a mut HashMap<String, std::ops::Range<usize>>,
    pub label_offset_map: &'a mut HashMap<String, (Number, Section)>,
    pub errors: Vec<CompileError>,
    pub rodata_phase: bool,
    pub text_offset: u64,
    pub rodata_offset: u64,
    pub missing_text_directive: bool,
    /// A rodata label on its own line, waiting for the next data directive.
    pub pending_rodata_label: Option<(String, std::ops::Range<usize>)>,
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
        // Extract the actual byte position from the pest error so the source
        // map can resolve it back to the original file/line.
        let span = match e.location {
            InputLocation::Pos(pos) => pos..pos + 1,
            InputLocation::Span((start, end)) => start..end,
        };

        // Build a clean message without pest's embedded source context,
        // which would show expanded-source line numbers.
        let message = match &e.variant {
            ErrorVariant::ParsingError {
                positives,
                negatives,
            } => {
                let pos: Vec<String> = positives.iter().filter_map(rule_display_name).collect();
                let neg: Vec<String> = negatives.iter().filter_map(rule_display_name).collect();
                let mut parts = Vec::new();
                if !pos.is_empty() {
                    parts.push(format!("expected {}", pos.join(", ")));
                }
                if !neg.is_empty() {
                    parts.push(format!("unexpected {}", neg.join(", ")));
                }
                if parts.is_empty() {
                    "Parse error".to_string()
                } else {
                    parts.join("; ")
                }
            }
            ErrorVariant::CustomError { message } => message.clone(),
        };

        vec![CompileError::ParseError {
            error: message,
            span,
            custom_label: None,
        }]
    })?;

    let mut ast = AST::new();
    let mut const_map = HashMap::<String, Number>::new();
    let mut label_spans = HashMap::<String, std::ops::Range<usize>>::new();

    // Pass 1: collect all label offsets so forward references work in expressions.
    let pairs_clone = pairs.clone();
    let mut label_offset_map = collect_label_offsets(pairs_clone);

    // Pass 2: full processing with label_offset_map already populated.
    let (text_offset, rodata_offset, errors) = {
        let mut ctx = ParseContext {
            ast: &mut ast,
            const_map: &mut const_map,
            label_spans: &mut label_spans,
            label_offset_map: &mut label_offset_map,
            errors: Vec::new(),
            rodata_phase: false,
            text_offset: 0,
            rodata_offset: 0,
            missing_text_directive: false,
            pending_rodata_label: None,
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

/// Pass 1: lightweight scan of the parse tree to collect all label offsets.
/// This enables forward references in operand expressions (e.g. rodata labels
/// referenced from the text section that appears earlier in the source).
fn collect_label_offsets(
    pairs: pest::iterators::Pairs<Rule>,
) -> HashMap<String, (Number, Section)> {
    let mut map = HashMap::new();
    let mut rodata_phase = false;
    let mut text_offset: u64 = 0;
    let mut rodata_offset: u64 = 0;

    for pair in pairs {
        match pair.as_rule() {
            Rule::program_default | Rule::program_llvm => {
                for statement in pair.into_inner() {
                    if statement.as_rule() == Rule::EOI {
                        continue;
                    }
                    scan_statement_for_labels(
                        statement,
                        &mut map,
                        &mut rodata_phase,
                        &mut text_offset,
                        &mut rodata_offset,
                    );
                }
            }
            _ => {}
        }
    }
    map
}

/// Scan a single statement to find labels and track offsets.
fn scan_statement_for_labels(
    pair: Pair<Rule>,
    map: &mut HashMap<String, (Number, Section)>,
    rodata_phase: &mut bool,
    text_offset: &mut u64,
    rodata_offset: &mut u64,
) {
    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::label_default | Rule::label_llvm => {
                scan_label(inner, map, rodata_phase, text_offset, rodata_offset);
            }
            Rule::directive => {
                // Track section switches and standalone data directive sizes
                for dir_inner in inner.into_inner() {
                    let dir_inner_clone = dir_inner.clone();
                    for dir_item in dir_inner.into_inner() {
                        if dir_item.as_rule() == Rule::directive_section {
                            let section_name = dir_item.as_str().trim_start_matches('.');
                            match section_name {
                                "text" => *rodata_phase = false,
                                "rodata" => *rodata_phase = true,
                                _ => {}
                            }
                        } else if *rodata_phase {
                            // Standalone data directive in rodata — account for its size
                            match dir_item.as_rule() {
                                Rule::directive_ascii
                                | Rule::directive_byte
                                | Rule::directive_short
                                | Rule::directive_word
                                | Rule::directive_int
                                | Rule::directive_long
                                | Rule::directive_quad => {
                                    *rodata_offset += rodata_directive_size(&dir_inner_clone);
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            Rule::instr_default | Rule::instr_llvm => {
                if !*rodata_phase {
                    let size = instr_size(&inner);
                    *text_offset += size;
                }
            }
            _ => {}
        }
    }
}

/// Scan a label node: record its offset and account for any attached
/// instruction/directive size.
fn scan_label(
    pair: Pair<Rule>,
    map: &mut HashMap<String, (Number, Section)>,
    rodata_phase: &mut bool,
    text_offset: &mut u64,
    rodata_offset: &mut u64,
) {
    let mut label_name = None;

    for item in pair.into_inner() {
        match item.as_rule() {
            Rule::identifier | Rule::numeric_label => {
                label_name = Some(item.as_str().to_string());
            }
            Rule::directive_inner => {
                // Rodata directive attached to label — compute data size
                if *rodata_phase {
                    if let Some(ref name) = label_name {
                        map.insert(
                            name.clone(),
                            (Number::Int(*rodata_offset as i64), Section::Rodata),
                        );
                    }
                    let size = rodata_directive_size(&item);
                    *rodata_offset += size;
                }
                return;
            }
            Rule::instr_default | Rule::instr_llvm => {
                if !*rodata_phase {
                    if let Some(ref name) = label_name {
                        map.insert(
                            name.clone(),
                            (Number::Int(*text_offset as i64), Section::Text),
                        );
                    }
                    let size = instr_size(&item);
                    *text_offset += size;
                }
                return;
            }
            _ => {}
        }
    }

    // Bare label (no directive or instruction attached)
    if let Some(name) = label_name {
        if *rodata_phase {
            map.insert(name, (Number::Int(*rodata_offset as i64), Section::Rodata));
        } else {
            map.insert(name, (Number::Int(*text_offset as i64), Section::Text));
        }
    }
}

/// Determine instruction size from the parse tree (lddw = 16 bytes, all others = 8).
fn instr_size(pair: &Pair<Rule>) -> u64 {
    for inner in pair.clone().into_inner() {
        match inner.as_rule() {
            Rule::instr_lddw | Rule::instr_llvm_lddw => return 16,
            _ => {}
        }
    }
    8
}

/// Determine the byte size of a rodata directive from the parse tree.
fn rodata_directive_size(pair: &Pair<Rule>) -> u64 {
    for inner in pair.clone().into_inner() {
        match inner.as_rule() {
            Rule::directive_ascii => {
                for ascii_inner in inner.into_inner() {
                    if ascii_inner.as_rule() == Rule::string_literal {
                        for content in ascii_inner.into_inner() {
                            if content.as_rule() == Rule::string_content {
                                return content.as_str().len() as u64;
                            }
                        }
                    }
                }
            }
            Rule::directive_byte => {
                return inner
                    .into_inner()
                    .filter(|p| p.as_rule() == Rule::number)
                    .count() as u64;
            }
            Rule::directive_short | Rule::directive_word => {
                return inner
                    .into_inner()
                    .filter(|p| p.as_rule() == Rule::number)
                    .count() as u64
                    * 2;
            }
            Rule::directive_int | Rule::directive_long => {
                return inner
                    .into_inner()
                    .filter(|p| p.as_rule() == Rule::number)
                    .count() as u64
                    * 4;
            }
            Rule::directive_quad => {
                return inner
                    .into_inner()
                    .filter(|p| p.as_rule() == Rule::number)
                    .count() as u64
                    * 8;
            }
            _ => {}
        }
    }
    0
}

/// Map internal pest rule names to human-readable descriptions for error messages.
fn rule_display_name(rule: &Rule) -> Option<String> {
    let name = match rule {
        // Top-level
        Rule::program_default | Rule::program_llvm => return None,
        Rule::statement_default | Rule::statement_llvm => "statement",
        Rule::label_default | Rule::label_llvm => "label",

        // Directives
        Rule::directive | Rule::directive_inner => "directive",
        Rule::directive_globl => ".globl",
        Rule::directive_extern => ".extern",
        Rule::directive_equ => ".equ",
        Rule::directive_section => "section (.text, .rodata)",
        Rule::directive_ascii => ".ascii",
        Rule::directive_byte => ".byte",
        Rule::directive_short => ".short",
        Rule::directive_word => ".word",
        Rule::directive_int => ".int",
        Rule::directive_long => ".long",
        Rule::directive_quad => ".quad",

        // Instructions
        Rule::instr_default | Rule::instr_llvm => "instruction",
        Rule::instr_lddw | Rule::instr_llvm_lddw => "lddw",
        Rule::instr_call => "call",
        Rule::instr_callx => "callx",
        Rule::instr_exit => "exit",

        // Operands
        Rule::register => "register",
        Rule::operand => "operand",
        Rule::number => "number",
        Rule::symbol => "symbol",
        Rule::identifier => "identifier",
        Rule::expression => "expression",
        Rule::string_literal => "string literal",

        // Memory
        Rule::memory_ref | Rule::llvm_memory_ref => "memory reference",
        Rule::jump_target => "jump target",

        // Whitespace / structure
        Rule::EOI => "end of input",
        _ => return Some(format!("{:?}", rule)),
    };
    Some(name.to_string())
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

                match process_instruction(inner, ctx.const_map, ctx.label_offset_map, is_llvm) {
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
            // Record label offset for expression evaluation
            ctx.label_offset_map.insert(
                label_name.clone(),
                (Number::Int(ctx.rodata_offset as i64), Section::Rodata),
            );

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
                if let Err(e) =
                    process_instruction(inst_pair, ctx.const_map, ctx.label_offset_map, is_llvm)
                {
                    ctx.errors.push(e);
                }
                if !ctx.missing_text_directive {
                    ctx.missing_text_directive = true;
                    ctx.errors.push(CompileError::MissingTextDirective {
                        span: label_span,
                        custom_label: None,
                    });
                }
            } else {
                // Bare rodata label (no directive on same line) — store it
                // so the next data directive can pick it up.
                ctx.pending_rodata_label = Some((label_name, label_span));
            }
        } else {
            // Record label offset for expression evaluation
            ctx.label_offset_map.insert(
                label_name.clone(),
                (Number::Int(ctx.text_offset as i64), Section::Text),
            );

            ctx.ast.nodes.push(ASTNode::Label {
                label: Label {
                    name: label_name,
                    span: label_span,
                },
                offset: ctx.text_offset,
            });

            if let Some(inst_pair) = instruction_opt {
                match process_instruction(inst_pair, ctx.const_map, ctx.label_offset_map, is_llvm) {
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
    label_offset_map: &HashMap<String, (Number, Section)>,
    is_llvm: bool,
) -> Result<Instruction, CompileError> {
    if is_llvm {
        llvm::process_instruction(pair, const_map, label_offset_map)
    } else {
        default::process_instruction(pair, const_map, label_offset_map)
    }
}

fn extract_label_from_pair(
    pair: Pair<Rule>,
) -> Result<(String, std::ops::Range<usize>), CompileError> {
    let span = pair.as_span();
    Ok((pair.as_str().to_string(), span.start()..span.end()))
}
