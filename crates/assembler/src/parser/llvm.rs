use {
    super::{BPF_X, Rule, common::*},
    crate::errors::CompileError,
    pest::iterators::Pair,
    sbpf_common::{
        inst_param::Number,
        instruction::Instruction,
        opcode::{MemOpKind, Opcode},
    },
    std::collections::HashMap,
};

pub(crate) fn process_instruction(
    pair: Pair<Rule>,
    const_map: &HashMap<String, Number>,
) -> Result<Instruction, CompileError> {
    let outer_span = pair.as_span();
    let outer_span_range = outer_span.start()..outer_span.end();

    for inner in pair.into_inner() {
        let span = inner.as_span();
        let span_range = span.start()..span.end();

        match inner.as_rule() {
            Rule::instr_llvm_alu64 => return process_alu(inner, const_map, span_range, true),
            Rule::instr_llvm_alu32 => return process_alu(inner, const_map, span_range, false),
            Rule::instr_llvm_neg64 => return process_neg(inner, span_range, true),
            Rule::instr_llvm_neg32 => return process_neg(inner, span_range, false),
            Rule::instr_llvm_load => return process_load(inner, const_map, span_range),
            Rule::instr_llvm_store_imm => return process_store_imm(inner, const_map, span_range),
            Rule::instr_llvm_store_reg => return process_store_reg(inner, const_map, span_range),
            Rule::instr_llvm_lddw => return process_lddw(inner, const_map, span_range),
            Rule::instr_llvm_endian => return process_endian(inner, span_range),
            Rule::instr_llvm_jump_uncond => {
                return process_jump_uncond(inner, const_map, span_range);
            }
            Rule::instr_llvm_jump_reg => return process_jump_reg(inner, span_range),
            Rule::instr_llvm_jump_imm => return process_jump_imm(inner, const_map, span_range),
            Rule::instr_exit => return process_exit(span_range),
            Rule::instr_call => return process_call(inner, const_map, span_range),
            Rule::instr_callx => return process_callx(inner, span_range),
            _ => {}
        }
    }

    Err(CompileError::ParseError {
        error: "Invalid LLVM instruction".to_string(),
        span: outer_span_range,
        file: None,
        custom_label: None,
    })
}

fn resolve_alu_opcode(op: &str, is_64bit: bool) -> Option<Opcode> {
    match (op, is_64bit) {
        ("+=", true) => Some(Opcode::Add64Imm),
        ("-=", true) => Some(Opcode::Sub64Imm),
        ("*=", true) => Some(Opcode::Mul64Imm),
        ("/=", true) => Some(Opcode::Div64Imm),
        ("|=", true) => Some(Opcode::Or64Imm),
        ("&=", true) => Some(Opcode::And64Imm),
        ("^=", true) => Some(Opcode::Xor64Imm),
        ("<<=", true) => Some(Opcode::Lsh64Imm),
        (">>=", true) => Some(Opcode::Rsh64Imm),
        ("%=", true) => Some(Opcode::Mod64Imm),
        ("=", true) => Some(Opcode::Mov64Imm),
        ("s>>=", true) => Some(Opcode::Arsh64Imm),
        ("+=", false) => Some(Opcode::Add32Imm),
        ("-=", false) => Some(Opcode::Sub32Imm),
        ("*=", false) => Some(Opcode::Mul32Imm),
        ("/=", false) => Some(Opcode::Div32Imm),
        ("|=", false) => Some(Opcode::Or32Imm),
        ("&=", false) => Some(Opcode::And32Imm),
        ("^=", false) => Some(Opcode::Xor32Imm),
        ("<<=", false) => Some(Opcode::Lsh32Imm),
        (">>=", false) => Some(Opcode::Rsh32Imm),
        ("%=", false) => Some(Opcode::Mod32Imm),
        ("=", false) => Some(Opcode::Mov32Imm),
        ("s>>=", false) => Some(Opcode::Arsh32Imm),
        _ => None,
    }
}

fn resolve_cmp_opcode(op: &str) -> Option<Opcode> {
    match op {
        "==" => Some(Opcode::JeqImm),
        "!=" => Some(Opcode::JneImm),
        ">" => Some(Opcode::JgtImm),
        ">=" => Some(Opcode::JgeImm),
        "<" => Some(Opcode::JltImm),
        "<=" => Some(Opcode::JleImm),
        "s>" => Some(Opcode::JsgtImm),
        "s>=" => Some(Opcode::JsgeImm),
        "s<" => Some(Opcode::JsltImm),
        "s<=" => Some(Opcode::JsleImm),
        "&" => Some(Opcode::JsetImm),
        _ => None,
    }
}

fn process_alu(
    pair: Pair<Rule>,
    const_map: &HashMap<String, Number>,
    span: std::ops::Range<usize>,
    is_64bit: bool,
) -> Result<Instruction, CompileError> {
    let mut dst = None;
    let mut op = None;
    let mut src = None;
    let mut imm = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::register | Rule::register_32 => {
                if dst.is_none() {
                    dst = Some(parse_register(inner)?);
                } else {
                    src = Some(parse_register(inner)?);
                }
            }
            Rule::alu_op => op = Some(inner.as_str().to_string()),
            Rule::operand => imm = Some(parse_operand(inner, const_map)?),
            _ => {}
        }
    }

    let op_str = op.as_deref().unwrap_or("=");
    let is_reg = src.is_some();

    let mut opcode =
        resolve_alu_opcode(op_str, is_64bit).ok_or_else(|| CompileError::ParseError {
            error: format!("Unknown ALU operator: {}", op_str),
            span: span.clone(),
            file: None,
            custom_label: None,
        })?;

    if is_reg {
        // Convert to register variant using BPF_X flag
        let reg_opcode_byte = Into::<u8>::into(opcode) | BPF_X;
        opcode = reg_opcode_byte
            .try_into()
            .map_err(|e| CompileError::BytecodeError {
                error: format!("Invalid opcode 0x{:02x}: {}", reg_opcode_byte, e),
                span: span.clone(),
                custom_label: None,
                file: None,
            })?;
    }

    Ok(Instruction {
        opcode,
        dst,
        src,
        off: None,
        imm,
        span,
    })
}

fn process_neg(
    pair: Pair<Rule>,
    span: std::ops::Range<usize>,
    is_64bit: bool,
) -> Result<Instruction, CompileError> {
    let mut dst = None;

    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::register || inner.as_rule() == Rule::register_32 {
            dst = Some(parse_register(inner)?);
        }
    }

    Ok(Instruction {
        opcode: if is_64bit {
            Opcode::Neg64
        } else {
            Opcode::Neg32
        },
        dst,
        src: None,
        off: None,
        imm: None,
        span,
    })
}

fn process_load(
    pair: Pair<Rule>,
    const_map: &HashMap<String, Number>,
    span: std::ops::Range<usize>,
) -> Result<Instruction, CompileError> {
    let mut dst = None;
    let mut size = None;
    let mut src = None;
    let mut off = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::llvm_register => dst = Some(parse_register(inner)?),
            Rule::mem_size => size = Some(inner.as_str().to_string()),
            Rule::llvm_memory_ref => {
                let (s, o) = parse_memory_ref(inner, const_map)?;
                src = Some(s);
                off = Some(o);
            }
            _ => {}
        }
    }

    let opcode =
        Opcode::from_size(size.as_deref().unwrap_or(""), MemOpKind::Load).ok_or_else(|| {
            CompileError::ParseError {
                error: "Invalid memory size for load".to_string(),
                span: span.clone(),
                file: None,
                custom_label: None,
            }
        })?;

    Ok(Instruction {
        opcode,
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
    let mut size = None;
    let mut dst = None;
    let mut off = None;
    let mut imm = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::mem_size => size = Some(inner.as_str().to_string()),
            Rule::llvm_memory_ref => {
                let (d, o) = parse_memory_ref(inner, const_map)?;
                dst = Some(d);
                off = Some(o);
            }
            Rule::operand => imm = Some(parse_operand(inner, const_map)?),
            _ => {}
        }
    }

    let opcode =
        Opcode::from_size(size.as_deref().unwrap_or(""), MemOpKind::StoreImm).ok_or_else(|| {
            CompileError::ParseError {
                error: "Invalid memory size for store".to_string(),
                span: span.clone(),
                file: None,
                custom_label: None,
            }
        })?;

    Ok(Instruction {
        opcode,
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
    let mut size = None;
    let mut dst = None;
    let mut off = None;
    let mut src = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::mem_size => size = Some(inner.as_str().to_string()),
            Rule::llvm_memory_ref => {
                let (d, o) = parse_memory_ref(inner, const_map)?;
                dst = Some(d);
                off = Some(o);
            }
            Rule::llvm_register => src = Some(parse_register(inner)?),
            _ => {}
        }
    }

    let opcode =
        Opcode::from_size(size.as_deref().unwrap_or(""), MemOpKind::StoreReg).ok_or_else(|| {
            CompileError::ParseError {
                error: "Invalid memory size for store".to_string(),
                span: span.clone(),
                file: None,
                custom_label: None,
            }
        })?;

    Ok(Instruction {
        opcode,
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

fn process_jump_reg(
    pair: Pair<Rule>,
    span: std::ops::Range<usize>,
) -> Result<Instruction, CompileError> {
    let mut dst = None;
    let mut op = None;
    let mut src = None;
    let mut off = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::register => {
                if dst.is_none() {
                    dst = Some(parse_register(inner)?);
                } else {
                    src = Some(parse_register(inner)?);
                }
            }
            Rule::cmp_op => op = Some(inner.as_str().to_string()),
            Rule::jump_target => off = Some(parse_jump_target(inner, &HashMap::new())?),
            _ => {}
        }
    }

    let op_str = op.as_deref().unwrap_or("==");
    let imm_opcode = resolve_cmp_opcode(op_str).ok_or_else(|| CompileError::ParseError {
        error: format!("Unknown comparison operator: {}", op_str),
        span: span.clone(),
        file: None,
        custom_label: None,
    })?;
    // Convert Imm variant to Reg variant using BPF_X flag
    let reg_opcode_byte = Into::<u8>::into(imm_opcode) | BPF_X;
    let opcode = reg_opcode_byte
        .try_into()
        .map_err(|e| CompileError::BytecodeError {
            error: format!("Invalid opcode 0x{:02x}: {}", reg_opcode_byte, e),
            span: span.clone(),
            custom_label: None,
            file: None,
        })?;

    Ok(Instruction {
        opcode,
        dst,
        src,
        off,
        imm: None,
        span,
    })
}

fn process_jump_imm(
    pair: Pair<Rule>,
    const_map: &HashMap<String, Number>,
    span: std::ops::Range<usize>,
) -> Result<Instruction, CompileError> {
    let mut dst = None;
    let mut op = None;
    let mut imm = None;
    let mut off = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::register => dst = Some(parse_register(inner)?),
            Rule::cmp_op => op = Some(inner.as_str().to_string()),
            Rule::operand => imm = Some(parse_operand(inner, const_map)?),
            Rule::jump_target => off = Some(parse_jump_target(inner, const_map)?),
            _ => {}
        }
    }

    let op_str = op.as_deref().unwrap_or("==");
    let opcode = resolve_cmp_opcode(op_str).ok_or_else(|| CompileError::ParseError {
        error: format!("Unknown comparison operator: {}", op_str),
        span: span.clone(),
        file: None,
        custom_label: None,
    })?;

    Ok(Instruction {
        opcode,
        dst,
        src: None,
        off,
        imm,
        span,
    })
}
