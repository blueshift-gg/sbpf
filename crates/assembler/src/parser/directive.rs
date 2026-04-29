use {
    super::{ParseContext, Rule, Token, common::parse_number},
    crate::{
        astnode::{ASTNode, ExternDecl, GlobalDecl, ROData, RodataDecl},
        errors::CompileError,
    },
    pest::iterators::Pair,
    sbpf_common::inst_param::Number,
    std::collections::HashMap,
};

pub fn process_directive_statement(pair: Pair<Rule>, ctx: &mut ParseContext) {
    for directive_inner_pair in pair.into_inner() {
        process_directive_inner(directive_inner_pair, ctx);
    }
}

pub fn process_directive_inner(pair: Pair<Rule>, ctx: &mut ParseContext) {
    let pair_clone = pair.clone();
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
            // Data directives (.ascii, .byte, etc.) — handle as rodata if
            // we're in the rodata phase and there's a pending label.
            Rule::directive_ascii
            | Rule::directive_byte
            | Rule::directive_short
            | Rule::directive_word
            | Rule::directive_int
            | Rule::directive_long
            | Rule::directive_quad => {
                if ctx.rodata_phase
                    && let Some((label_name, label_span)) = ctx.pending_rodata_label.take()
                {
                    match process_rodata_directive(label_name, label_span, pair_clone) {
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
                    return;
                }
            }
            _ => {}
        }
    }
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
