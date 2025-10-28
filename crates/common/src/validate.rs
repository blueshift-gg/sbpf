use crate::{errors::SBPFError, instruction::Instruction};

// TODO validate fields that are supposed to be None

pub fn validate_load_immediate(inst: &Instruction) -> Result<(), SBPFError> {
    match (&inst.dst, &inst.imm) {
        (Some(_dst), Some(_imm)) => Ok(()),
        // Ok(format!("{} r{}, {}", inst.opcode, dst.n, imm)),
        _ => Err(SBPFError::BytecodeError {
            error: "Lddw instruction missing destination register or immediate value".to_string(),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn validate_load_memory(inst: &Instruction) -> Result<(), SBPFError> {
    match &inst.dst {
        Some(_dst) =>  Ok(()),
        // Ok(format!("{} r{}, {}", inst.opcode, dst.n, inst.src_off())),
        None => Err(SBPFError::BytecodeError {
            error: format!("{} instruction missing destination register", inst.opcode),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn validate_store_immediate(inst: &Instruction) -> Result<(), SBPFError> {
    match &inst.imm {
        Some(_imm) => Ok(()),
        // Ok(format!("{} {}, {}", inst.opcode, inst.dst_off(), imm)),
        None => Err(SBPFError::BytecodeError {
            error: format!("{} instruction missing immediate value", inst.opcode),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn validate_store_register(inst: &Instruction) -> Result<(), SBPFError> {
    match &inst.src {
        Some(_src) => Ok(()),
        // Ok(format!("{} {}, r{}", inst.opcode, inst.dst_off(), src.n)),
        None => Err(SBPFError::BytecodeError {
            error: format!("{} instruction missing source register", inst.opcode),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn validate_unary(inst: &Instruction) -> Result<(), SBPFError> {
    match &inst.dst {
        Some(_dst) => Ok(()),
        // Ok(format!("{} r{}", inst.opcode, dst.n)),
        None => Err(SBPFError::BytecodeError {
            error: format!("{} instruction missing destination register", inst.opcode),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn validate_binary_immediate(inst: &Instruction) -> Result<(), SBPFError> {
//     match inst.opcode {
//         Opcode::Le | Opcode::Be => match &inst.dst {
//             Some(dst) => Ok(format!("{}{}", inst.op_imm_bits()?, dst.n)),
//             None => Err(SBPFError::BytecodeError {
//                 error: format!("{} instruction missing destination register", inst.opcode),
//                 span: inst.span.clone(),
//                 custom_label: None,
//             }),
//         },
//         _ =>
        match (&inst.dst, &inst.imm) {
            (Some(_dst), Some(_imm)) => Ok(()),
            // Ok(format!("{} r{}, {}", inst.opcode, dst.n, imm)),
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
// }

pub fn validate_binary_register(inst: &Instruction) -> Result<(), SBPFError> {
    match (&inst.dst, &inst.src) {
        (Some(_dst), Some(_src)) => Ok(()),
        // Ok(format!("{} r{}, r{}", inst.opcode, dst.n, src.n)),
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

pub fn validate_jump(_inst: &Instruction) -> Result<(), SBPFError> {
    Ok(())
    // Ok(format!("{} {}", inst.opcode, inst.off_str()))
}

pub fn validate_jump_immediate(inst: &Instruction) -> Result<(), SBPFError> {
    match (&inst.dst, &inst.imm) {
        (Some(_dst), Some(_imm)) => Ok(()),
        // Ok(format!("{} r{}, {}, {}", inst.opcode, dst.n, imm, inst.off_str())),
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

pub fn validate_jump_register(inst: &Instruction) -> Result<(), SBPFError> {
    match (&inst.dst, &inst.src) {
        (Some(_dst), Some(_src)) => Ok(()),
        // Ok(format!("{} r{}, r{}, {}", inst.opcode, dst.n, src.n, inst.off_str())),
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

pub fn validate_call_immediate(inst: &Instruction) -> Result<(), SBPFError> {
    match &inst.imm {
        Some(_imm) => Ok(()),
        // Ok(format!("call {}", imm)),
        None => Err(SBPFError::BytecodeError {
            error: "Call instruction missing immediate value".to_string(),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn validate_call_register(inst: &Instruction) -> Result<(), SBPFError> {
    match &inst.src {
        Some(_src) => Ok(()),
        // Ok(format!("call r{}", src.n)),
        None => Err(SBPFError::BytecodeError {
            error: "Callx instruction missing source register".to_string(),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn validate_exit(_inst: &Instruction) -> Result<(), SBPFError> {
    Ok(())
    // Ok(format!("{}", inst.opcode))
}
