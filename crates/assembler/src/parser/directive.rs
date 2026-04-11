use {
    super::{IncludeSite, ParseContext, Rule, Token, common::parse_number, process_source},
    crate::{
        astnode::{ASTNode, ExternDecl, GlobalDecl, ROData, RodataDecl},
        errors::CompileError,
    },
    pest::iterators::Pair,
    sbpf_common::inst_param::Number,
    std::{collections::HashMap, fs, path::PathBuf},
};

pub fn process_directive_statement(pair: Pair<Rule>, ctx: &mut ParseContext) {
    for directive_inner_pair in pair.into_inner() {
        process_directive_inner(directive_inner_pair, ctx);
    }
}

pub fn process_directive_inner(pair: Pair<Rule>, ctx: &mut ParseContext) {
    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::directive_globl => {
                let span = inner.as_span();
                for globl_inner in inner.into_inner() {
                    if globl_inner.as_rule() == Rule::globl_symbol {
                        let entry_label = globl_inner.as_str().to_string();
                        ctx.ast.nodes.push(ASTNode::GlobalDecl {
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
                ctx.ast.nodes.push(ASTNode::ExternDecl {
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
                        Rule::expression => match eval_expression(equ_inner, ctx.const_map) {
                            Ok(v) => value = Some(v),
                            Err(e) => ctx.errors.push(e),
                        },
                        _ => {}
                    }
                }

                if let (Some(name), Some(val)) = (ident, value) {
                    ctx.const_map.insert(name, val);
                }
            }
            Rule::directive_section => {
                let section_name = inner.as_str().trim_start_matches('.');
                match section_name {
                    "text" => ctx.rodata_phase = false,
                    "rodata" => {
                        ctx.rodata_phase = true;
                        let span = inner.as_span();
                        ctx.ast.nodes.push(ASTNode::RodataDecl {
                            rodata_decl: RodataDecl {
                                span: span.start()..span.end(),
                            },
                        });
                    }
                    _ => {}
                }
            }
            Rule::directive_include => {
                process_directive_include(inner, ctx);
            }
            _ => {}
        }
    }
}

/// Handle `.include "path"` by reading the referenced file and parsing it
/// into the same `ParseContext` so its nodes are merged into one AST.
///
/// Paths are resolved relative to the directory of the file containing
/// the `.include` directive. Nested includes are supported: an included
/// file may itself use `.include` with paths relative to its own location.
///
/// The current file name (`ctx.current_file`) and base path are swapped
/// for the duration of the recursive parse, and the `.include` site is
/// pushed onto `ctx.include_stack` so diagnostics (e.g. `DuplicateLabel`)
/// can annotate errors with the chain of `.include` directives that led
/// to the problem.
fn process_directive_include(pair: Pair<Rule>, ctx: &mut ParseContext) {
    let span = pair.as_span();
    let include_span = span.start()..span.end();

    // Extract the `"path"` string literal content.
    let mut raw_path: Option<String> = None;
    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::string_literal {
            for lit_inner in inner.into_inner() {
                if lit_inner.as_rule() == Rule::string_content {
                    raw_path = Some(lit_inner.as_str().to_string());
                    break;
                }
            }
        }
    }

    let path = match raw_path {
        Some(p) => p,
        None => {
            ctx.errors.push(CompileError::ParseError {
                error: "Invalid .include directive: expected \"path\"".to_string(),
                span: include_span,
                custom_label: None,
            });
            return;
        }
    };

    // Resolve the path relative to the including file's directory.
    let base = match &ctx.base_path {
        Some(b) => b.clone(),
        None => {
            ctx.errors.push(CompileError::ParseError {
                error: format!(
                    "Cannot resolve .include \"{}\": no base path available. \
                     Use assemble_with_base_path to enable .include support.",
                    path
                ),
                span: include_span,
                custom_label: None,
            });
            return;
        }
    };
    let include_path = base.join(&path);

    // Read the included file.
    let content = match fs::read_to_string(&include_path) {
        Ok(c) => c,
        Err(e) => {
            ctx.errors.push(CompileError::ParseError {
                error: format!("Failed to read include \"{}\": {}", path, e),
                span: include_span,
                custom_label: None,
            });
            return;
        }
    };

    // Normalize the identifier we use for this file in diagnostics and
    // debug info: the path as written, with any `./` prefix stripped.
    let include_id = path.trim_start_matches("./").to_string();

    // Register the file content (skip if already read via an earlier
    // include of the same path — this is harmless because duplicate label
    // detection catches re-parses).
    ctx.files
        .entry(include_id.clone())
        .or_insert_with(|| content.clone());

    // Compute the new base path for nested includes inside the included
    // file: paths are relative to the included file's own directory.
    let new_base: PathBuf = include_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    // Push the include site, swap in the new context, recurse, restore.
    ctx.include_stack.push(IncludeSite {
        file: ctx.current_file.clone(),
        span: include_span,
    });
    let old_base = ctx.base_path.replace(new_base);
    let old_file = std::mem::replace(&mut ctx.current_file, include_id);

    process_source(&content, ctx);

    ctx.current_file = old_file;
    ctx.base_path = old_base;
    ctx.include_stack.pop();
}

pub fn process_rodata_directive(
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
            | Rule::directive_short
            | Rule::directive_word
            | Rule::directive_int
            | Rule::directive_long
            | Rule::directive_quad => {
                let directive_name = match inner.as_rule() {
                    Rule::directive_byte => "byte",
                    Rule::directive_short => "short",
                    Rule::directive_word => "word",
                    Rule::directive_int => "int",
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
