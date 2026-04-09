use {
    super::Rule,
    crate::errors::CompileError,
    either::Either,
    pest::iterators::Pair,
    sbpf_common::{
        inst_param::{Number, Register},
        instruction::Instruction,
        opcode::Opcode,
    },
    std::collections::HashMap,
};

// Shared parse functions.

pub fn parse_register(pair: Pair<Rule>) -> Result<Register, CompileError> {
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

pub fn parse_operand(
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

pub fn parse_jump_target(
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
            Rule::number | Rule::signed_number => {
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

pub fn parse_memory_ref(
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

pub fn parse_number(pair: Pair<Rule>) -> Result<Number, CompileError> {
    let span = pair.as_span();
    let span_range = span.start()..span.end();
    let raw = pair.as_str();
    let number_str = raw.strip_prefix('+').unwrap_or(raw).replace('_', "");

    // Try parsing as i64 first
    if let Ok(value) = number_str.parse::<i64>() {
        return Ok(Number::Int(value));
    }

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
    }

    Err(CompileError::InvalidNumber {
        number: number_str,
        span: span_range,
        custom_label: None,
    })
}

// Shared process functions.

pub fn process_exit(span: std::ops::Range<usize>) -> Result<Instruction, CompileError> {
    Ok(Instruction {
        opcode: Opcode::Exit,
        dst: None,
        src: None,
        off: None,
        imm: None,
        span,
    })
}

pub fn process_lddw(
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

pub fn process_endian(
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

pub fn process_call(
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

pub fn process_callx(
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
