pub mod common;
mod default;
pub(crate) mod directive;
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
    std::{
        collections::{HashMap, HashSet},
        path::{Path, PathBuf},
    },
};

#[derive(Parser)]
#[grammar = "sbpf.pest"]
pub struct SbpfParser;

/// A location in a source file — used both for label definitions and
/// `.include` directive sites in the include chain.
#[derive(Debug, Clone)]
pub struct SourceLocation {
    pub file: String,
    pub span: std::ops::Range<usize>,
}

/// Context containing all mutable state during parsing.
pub(crate) struct ParseContext<'a> {
    pub ast: &'a mut AST,
    pub const_map: &'a mut HashMap<String, Number>,
    pub label_spans: &'a mut HashMap<String, SourceLocation>,
    pub errors: Vec<CompileError>,
    pub rodata_phase: bool,
    pub text_offset: u64,
    pub rodata_offset: u64,
    pub missing_text_directive: bool,

    /// Directory used to resolve `.include` paths. Updated when we descend
    /// into an included file (paths are resolved relative to the including
    /// file's directory).
    pub base_path: Option<PathBuf>,
    /// The root base path (directory of the main source file). Used to
    /// compute include identifiers relative to the project root so that
    /// nested includes like `modules/a.s` including `b.s` register as
    /// `modules/b.s` rather than just `b.s`.
    pub root_base_path: Option<PathBuf>,
    /// File currently being parsed. For the main source this is the main
    /// file name; for included files it is the relative path from the
    /// project root.
    pub current_file: String,
    /// Chain of `.include` sites leading to the currently-parsed source.
    /// Empty when parsing the main file. Used to annotate diagnostics with
    /// the point(s) where an included file was pulled in.
    pub include_stack: Vec<SourceLocation>,
    /// Registry of all sources read so far: `file -> content`. Populated on
    /// each `.include`, plus the main file. Used by the build command to
    /// emit multi-file diagnostics.
    pub files: &'a mut HashMap<String, String>,
    /// Canonicalized paths of files currently being parsed, used to detect
    /// cyclic includes. A file is added before recursing and removed after.
    pub include_set: HashSet<PathBuf>,
}

/// If the error has no file attribution yet, fill in the given file name
/// so that errors originating from included files carry the correct attribution.
pub(crate) fn attach_file_to_error(error: CompileError, file: &str) -> CompileError {
    error.with_file(file)
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

    /// Registry of every source file read during parsing (main + includes).
    /// `file -> content`. Empty if `.include` was not used.
    pub sources: HashMap<String, String>,
}

/// Parse `source` as a standalone program. `.include` directives inside
/// `source` will produce an error because no base path is available.
pub fn parse(source: &str, arch: SbpfArch) -> Result<ParseResult, Vec<CompileError>> {
    let mut sources_out = HashMap::new();
    parse_with_base_path(source, arch, None, "source", &mut sources_out)
}

/// Parse `source` with support for `.include` directives.
///
/// `base_path` is the directory used to resolve `.include "<path>"` lines
/// in the main source. `main_file_name` is the identifier that will be
/// used for the main file in diagnostics and debug info (typically the
/// file name without the directory).
///
/// `sources_out` is populated with every source read during parsing — the
/// main file plus any files pulled in via `.include` — keyed by the file
/// identifier used in diagnostics. It is populated in both the success
/// and failure paths, so callers can emit multi-file diagnostics even
/// when assembly fails (e.g. `DuplicateLabel` across includes).
pub fn parse_with_base_path(
    source: &str,
    arch: SbpfArch,
    base_path: Option<&Path>,
    main_file_name: &str,
    sources_out: &mut HashMap<String, String>,
) -> Result<ParseResult, Vec<CompileError>> {
    let mut ast = AST::new();
    let mut const_map = HashMap::<String, Number>::new();
    let mut label_spans = HashMap::<String, SourceLocation>::new();
    sources_out.insert(main_file_name.to_string(), source.to_string());

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
            base_path: base_path.map(|p| p.to_path_buf()),
            root_base_path: base_path.map(|p| p.canonicalize().unwrap_or_else(|_| p.to_path_buf())),
            current_file: main_file_name.to_string(),
            include_stack: Vec::new(),
            files: sources_out,
            include_set: HashSet::new(),
        };

        process_source(source, &mut ctx);

        (ctx.text_offset, ctx.rodata_offset, ctx.errors)
    };

    if !errors.is_empty() {
        return Err(errors);
    }

    ast.set_text_size(text_offset);
    ast.set_rodata_size(rodata_offset);

    let mut result = ast.build_program(arch)?;
    result.sources = sources_out.clone();
    Ok(result)
}

/// Parse `source` into `ctx`. Used both for the main file and recursively
/// for included files. Spans in this source are relative to the source
/// string; `ctx.current_file` identifies which file they belong to.
pub(crate) fn process_source(source: &str, ctx: &mut ParseContext) {
    let pairs = match SbpfParser::parse(Rule::program, source) {
        Ok(p) => p,
        Err(e) => {
            // Include the file name in the error so the user knows which
            // file has the syntax problem — especially important for
            // included files where the span is relative to that file's
            // text, not the main source.
            let error_msg = if ctx.include_stack.is_empty() {
                e.to_string()
            } else {
                format!("in \"{}\": {}", ctx.current_file, e)
            };
            ctx.errors.push(CompileError::ParseError {
                error: error_msg,
                span: 0..source.len(),
                file: Some(ctx.current_file.clone()),
                custom_label: None,
            });
            return;
        }
    };

    for pair in pairs {
        match pair.as_rule() {
            Rule::program_default | Rule::program_llvm => {
                for statement in pair.into_inner() {
                    if statement.as_rule() == Rule::EOI {
                        continue;
                    }
                    process_statement(statement, ctx);
                }
            }
            _ => {}
        }
    }
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
                                file: Some(ctx.current_file.clone()),
                            });
                            ctx.text_offset += size;
                        }
                    }
                    Err(e) => ctx.errors.push(attach_file_to_error(e, &ctx.current_file)),
                }

                if ctx.rodata_phase && !ctx.missing_text_directive {
                    ctx.missing_text_directive = true;
                    ctx.errors.push(CompileError::MissingTextDirective {
                        span: span_range,
                        custom_label: None,
                        file: None,
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
                Err(e) => ctx.errors.push(attach_file_to_error(e, &ctx.current_file)),
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
        if let Some(original) = ctx.label_spans.get(&label_name) {
            ctx.errors.push(CompileError::DuplicateLabel {
                label: label_name,
                span: label_span,
                original_span: original.span.clone(),
                multi: Some(Box::new(crate::errors::DuplicateLabelMulti {
                    span_file: ctx.current_file.clone(),
                    original_span_file: original.file.clone(),
                    include_chain: ctx
                        .include_stack
                        .iter()
                        .map(|s| (s.file.clone(), s.span.clone()))
                        .collect(),
                })),
                custom_label: Some("Label already defined".to_string()),
                file: Some(ctx.current_file.clone()),
            });
            return;
        }
        ctx.label_spans.insert(
            label_name.clone(),
            SourceLocation {
                file: ctx.current_file.clone(),
                span: label_span.clone(),
            },
        );

        if ctx.rodata_phase {
            // Handle rodata label with directive
            if let Some(dir_pair) = directive_opt {
                match process_rodata_directive(label_name.clone(), label_span.clone(), dir_pair) {
                    Ok(rodata) => {
                        let size = rodata.get_size();
                        ctx.ast.rodata_nodes.push(ASTNode::ROData {
                            rodata,
                            offset: ctx.rodata_offset,
                            file: Some(ctx.current_file.clone()),
                        });
                        ctx.rodata_offset += size;
                    }
                    Err(e) => ctx.errors.push(attach_file_to_error(e, &ctx.current_file)),
                }
            } else if let Some(inst_pair) = instruction_opt {
                if let Err(e) = process_instruction(inst_pair, ctx.const_map, is_llvm) {
                    ctx.errors.push(attach_file_to_error(e, &ctx.current_file));
                }
                if !ctx.missing_text_directive {
                    ctx.missing_text_directive = true;
                    ctx.errors.push(CompileError::MissingTextDirective {
                        span: label_span,
                        custom_label: None,
                        file: None,
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
                file: Some(ctx.current_file.clone()),
            });

            if let Some(inst_pair) = instruction_opt {
                match process_instruction(inst_pair, ctx.const_map, is_llvm) {
                    Ok(instruction) => {
                        let size = instruction.get_size();
                        ctx.ast.nodes.push(ASTNode::Instruction {
                            instruction,
                            offset: ctx.text_offset,
                            file: Some(ctx.current_file.clone()),
                        });
                        ctx.text_offset += size;
                    }
                    Err(e) => ctx.errors.push(attach_file_to_error(e, &ctx.current_file)),
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
