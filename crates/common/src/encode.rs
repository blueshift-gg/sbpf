use crate::{errors::SBPFError, instruction::Instruction, opcode::Opcode};

pub fn encode_load_immediate(inst: &Instruction) -> Result<String, SBPFError> {
    match (&inst.dst, &inst.imm) {
        (Some(dst), Some(imm)) => Ok(format!("{} r{}, {}", inst.opcode, dst.n, imm)),
        _ => Err(SBPFError::BytecodeError {
            error: "Lddw instruction missing destination register or immediate value".to_string(),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn encode_load_memory(inst: &Instruction) -> Result<String, SBPFError> {
    match &inst.dst {
        Some(dst) => Ok(format!("{} r{}, {}", inst.opcode, dst.n, inst.src_off())),
        None => Err(SBPFError::BytecodeError {
            error: format!("{} instruction missing destination register", inst.opcode),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn encode_store_immediate(inst: &Instruction) -> Result<String, SBPFError> {
    match &inst.imm {
        Some(imm) => Ok(format!("{} {}, {}", inst.opcode, inst.dst_off(), imm)),
        None => Err(SBPFError::BytecodeError {
            error: format!("{} instruction missing immediate value", inst.opcode),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn encode_store_register(inst: &Instruction) -> Result<String, SBPFError> {
    match &inst.src {
        Some(src) => Ok(format!("{} {}, r{}", inst.opcode, inst.dst_off(), src.n)),
        None => Err(SBPFError::BytecodeError {
            error: format!("{} instruction missing source register", inst.opcode),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn encode_unary(inst: &Instruction) -> Result<String, SBPFError> {
    match &inst.dst {
        Some(dst) => Ok(format!("{} r{}", inst.opcode, dst.n)),
        None => Err(SBPFError::BytecodeError {
            error: format!("{} instruction missing destination register", inst.opcode),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn encode_binary_immediate(inst: &Instruction) -> Result<String, SBPFError> {
    match inst.opcode {
        Opcode::Le | Opcode::Be => match &inst.dst {
            Some(dst) => Ok(format!("{}{}", inst.op_imm_bits()?, dst.n)),
            None => Err(SBPFError::BytecodeError {
                error: format!("{} instruction missing destination register", inst.opcode),
                span: inst.span.clone(),
                custom_label: None,
            }),
        },
        _ => match (&inst.dst, &inst.imm) {
            (Some(dst), Some(imm)) => Ok(format!("{} r{}, {}", inst.opcode, dst.n, imm)),
            _ => Err(SBPFError::BytecodeError {
                error: format!(
                    "{} instruction missing destination register or immediate value",
                    inst.opcode
                ),
                span: inst.span.clone(),
                custom_label: None,
            }),
        },
    }
}

pub fn encode_binary_register(inst: &Instruction) -> Result<String, SBPFError> {
    match (&inst.dst, &inst.src) {
        (Some(dst), Some(src)) => Ok(format!("{} r{}, r{}", inst.opcode, dst.n, src.n)),
        _ => Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction missing destination or source register",
                inst.opcode
            ),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn encode_jump(inst: &Instruction) -> Result<String, SBPFError> {
    Ok(format!("{} {}", inst.opcode, inst.off_str()))
}

pub fn encode_jump_immediate(inst: &Instruction) -> Result<String, SBPFError> {
    match (&inst.dst, &inst.imm) {
        (Some(dst), Some(imm)) => Ok(format!(
            "{} r{}, {}, {}",
            inst.opcode,
            dst.n,
            imm,
            inst.off_str()
        )),
        _ => Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction missing destination register or immediate value",
                inst.opcode
            ),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn encode_jump_register(inst: &Instruction) -> Result<String, SBPFError> {
    match (&inst.dst, &inst.src) {
        (Some(dst), Some(src)) => Ok(format!(
            "{} r{}, r{}, {}",
            inst.opcode,
            dst.n,
            src.n,
            inst.off_str()
        )),
        _ => Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction missing destination or source register",
                inst.opcode
            ),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn encode_call_immediate(inst: &Instruction) -> Result<String, SBPFError> {
    match &inst.imm {
        Some(imm) => Ok(format!("call {}", imm)),
        None => Err(SBPFError::BytecodeError {
            error: "Call instruction missing immediate value".to_string(),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn encode_call_register(inst: &Instruction) -> Result<String, SBPFError> {
    match &inst.src {
        Some(src) => Ok(format!("call r{}", src.n)),
        None => Err(SBPFError::BytecodeError {
            error: "Callx instruction missing source register".to_string(),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn encode_exit(inst: &Instruction) -> Result<String, SBPFError> {
    Ok(format!("{}", inst.opcode))
}
