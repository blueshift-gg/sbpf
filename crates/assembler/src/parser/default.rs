use {
    super::{BPF_X, Rule, Section, common::*},
    crate::errors::CompileError,
    pest::iterators::Pair,
    sbpf_common::{inst_param::Number, instruction::Instruction, opcode::Opcode},
    std::{collections::HashMap, str::FromStr},
};

pub(crate) fn process_instruction(
    pair: Pair<Rule>,
    const_map: &HashMap<String, Number>,
    label_offset_map: &HashMap<String, (Number, Section)>,
) -> Result<Instruction, CompileError> {
    let outer_span = pair.as_span();
    let outer_span_range = outer_span.start()..outer_span.end();

    for inner in pair.into_inner() {
        let span = inner.as_span();
        let span_range = span.start()..span.end();

        match inner.as_rule() {
            Rule::instr_exit => return process_exit(span_range),
            Rule::instr_lddw => {
                return process_lddw(inner, const_map, label_offset_map, span_range);
            }
            Rule::instr_call => return process_call(inner, const_map, span_range),
            Rule::instr_callx => return process_callx(inner, span_range),
            Rule::instr_neg32 => return process_neg32(inner, span_range),
            Rule::instr_neg64 => return process_neg64(inner, span_range),
            Rule::instr_alu64_imm | Rule::instr_alu32_imm => {
                return process_alu_imm(inner, const_map, label_offset_map, span_range);
            }
            Rule::instr_alu64_reg | Rule::instr_alu32_reg => {
                return process_alu_reg(inner, span_range);
            }
            Rule::instr_load => return process_load(inner, const_map, span_range),
            Rule::instr_store_imm => {
                return process_store_imm(inner, const_map, label_offset_map, span_range);
            }
            Rule::instr_store_reg => return process_store_reg(inner, const_map, span_range),
            Rule::instr_jump_imm => {
                return process_jump_imm(inner, const_map, label_offset_map, span_range);
            }
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
    label_offset_map: &HashMap<String, (Number, Section)>,
    span: std::ops::Range<usize>,
) -> Result<Instruction, CompileError> {
    let mut opcode = None;
    let mut dst = None;
    let mut off = None;
    let mut imm = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::store_op_imm => opcode = Opcode::from_str(inner.as_str()).ok(),
            Rule::memory_ref => {
                let (d, o) = parse_memory_ref(inner, const_map)?;
                dst = Some(d);
                off = Some(o);
            }
            Rule::operand => imm = Some(parse_operand(inner, const_map, label_offset_map)?),
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
            Rule::store_op_reg => opcode = Opcode::from_str(inner.as_str()).ok(),
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
    label_offset_map: &HashMap<String, (Number, Section)>,
    span: std::ops::Range<usize>,
) -> Result<Instruction, CompileError> {
    let mut opcode = None;
    let mut dst = None;
    let mut imm = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::alu_64_op | Rule::alu_32_op => opcode = Opcode::from_str(inner.as_str()).ok(),
            Rule::register => dst = Some(parse_register(inner)?),
            Rule::operand => imm = Some(parse_operand(inner, const_map, label_offset_map)?),
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
    label_offset_map: &HashMap<String, (Number, Section)>,
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
            Rule::operand => imm = Some(parse_operand(inner, const_map, label_offset_map)?),
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
