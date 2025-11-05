use crate::{errors::SBPFError, instruction::Instruction};

pub fn validate_load_immediate(inst: &Instruction) -> Result<(), SBPFError> {
    match (&inst.dst, &inst.src, &inst.off, &inst.imm) {
        (Some(_dst), None, None, Some(_imm)) => Ok(()),
        _ => Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction requires destination register and immediate value only",
                inst.opcode
            ),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn validate_load_memory(inst: &Instruction) -> Result<(), SBPFError> {
    match (&inst.dst, &inst.src, &inst.off, &inst.imm) {
        (Some(_dst), Some(_src), Some(_off), None) => Ok(()),
        _ => Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction requires destination register, source register, offset, and no \
                 immediate value",
                inst.opcode
            ),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn validate_store_immediate(inst: &Instruction) -> Result<(), SBPFError> {
    match (&inst.dst, &inst.src, &inst.off, &inst.imm) {
        (Some(_dst), None, Some(_off), Some(_imm)) => Ok(()),
        _ => Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction requires destination register, offset, immediate value, and no \
                 source register",
                inst.opcode
            ),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn validate_store_register(inst: &Instruction) -> Result<(), SBPFError> {
    match (&inst.dst, &inst.src, &inst.off, &inst.imm) {
        (Some(_dst), Some(_src), Some(_off), None) => Ok(()),
        _ => Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction requires destination register, source register, offset, and no \
                 immediate value",
                inst.opcode
            ),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn validate_unary(inst: &Instruction) -> Result<(), SBPFError> {
    match (&inst.dst, &inst.src, &inst.off, &inst.imm) {
        (Some(_dst), None, None, None) => Ok(()),
        _ => Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction requires destination register only",
                inst.opcode
            ),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn validate_binary_immediate(inst: &Instruction) -> Result<(), SBPFError> {
    match (&inst.dst, &inst.src, &inst.off, &inst.imm) {
        (Some(_dst), None, None, Some(_imm)) => Ok(()),
        _ => Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction requires destination register and immediate value only",
                inst.opcode
            ),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn validate_binary_register(inst: &Instruction) -> Result<(), SBPFError> {
    match (&inst.dst, &inst.src, &inst.off, &inst.imm) {
        (Some(_dst), Some(_src), None, None) => Ok(()),
        _ => Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction requires destination and source registers only",
                inst.opcode
            ),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn validate_jump(inst: &Instruction) -> Result<(), SBPFError> {
    match (&inst.dst, &inst.src, &inst.off, &inst.imm) {
        (_, None, _, None) => Ok(()),
        _ => Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction requires no source register or immediate value",
                inst.opcode
            ),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn validate_jump_immediate(inst: &Instruction) -> Result<(), SBPFError> {
    match (&inst.dst, &inst.src, &inst.off, &inst.imm) {
        (Some(_dst), None, _, Some(_imm)) => Ok(()),
        _ => Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction requires destination register and immediate value, and no source \
                 register",
                inst.opcode
            ),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn validate_jump_register(inst: &Instruction) -> Result<(), SBPFError> {
    match (&inst.dst, &inst.src, &inst.off, &inst.imm) {
        (Some(_dst), Some(_src), Some(_off), None) => Ok(()),
        _ => Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction requires destination register, source register, and offset",
                inst.opcode
            ),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn validate_call_immediate(inst: &Instruction) -> Result<(), SBPFError> {
    match (&inst.dst, &inst.src, &inst.off, &inst.imm) {
        (None, None, None, Some(_imm)) => Ok(()),
        _ => Err(SBPFError::BytecodeError {
            error: format!("{} instruction requires immediate value only", inst.opcode),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn validate_call_register(inst: &Instruction) -> Result<(), SBPFError> {
    match (&inst.dst, &inst.src, &inst.off, &inst.imm) {
        (None, Some(_src), None, None) => Ok(()),
        _ => Err(SBPFError::BytecodeError {
            error: format!("{} instruction requires source register only", inst.opcode),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

pub fn validate_exit(inst: &Instruction) -> Result<(), SBPFError> {
    match (&inst.dst, &inst.src, &inst.off, &inst.imm) {
        (None, None, None, None) => Ok(()),
        _ => Err(SBPFError::BytecodeError {
            error: format!("{} instruction requires no operands", inst.opcode),
            span: inst.span.clone(),
            custom_label: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            inst_param::{Number, Register},
            instruction::Instruction,
            opcode::Opcode,
        },
        either::Either,
    };

    #[test]
    fn test_validate_load_immediate_valid() {
        let valid_inst = Instruction {
            opcode: Opcode::Lddw,
            dst: Some(Register { n: 0 }),
            src: None,
            off: None,
            imm: Some(Either::Right(Number::Int(42))),
            span: 0..8,
        };
        assert!(validate_load_immediate(&valid_inst).is_ok());
    }

    #[test]
    fn test_validate_load_immediate_missing_dst() {
        let inst = Instruction {
            opcode: Opcode::Lddw,
            dst: None,
            src: None,
            off: None,
            imm: Some(Either::Right(Number::Int(42))),
            span: 0..8,
        };
        let result = validate_load_immediate(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register and immediate value only",
                    Opcode::Lddw
                )
            );
        }
    }

    #[test]
    fn test_validate_load_immediate_missing_imm() {
        let inst = Instruction {
            opcode: Opcode::Lddw,
            dst: Some(Register { n: 0 }),
            src: None,
            off: None,
            imm: None,
            span: 0..8,
        };
        let result = validate_load_immediate(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register and immediate value only",
                    Opcode::Lddw
                )
            );
        }
    }

    #[test]
    fn test_validate_load_immediate_has_src() {
        let inst = Instruction {
            opcode: Opcode::Lddw,
            dst: Some(Register { n: 0 }),
            src: Some(Register { n: 1 }),
            off: None,
            imm: Some(Either::Right(Number::Int(42))),
            span: 0..8,
        };
        let result = validate_load_immediate(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register and immediate value only",
                    Opcode::Lddw
                )
            );
        }
    }

    #[test]
    fn test_validate_load_immediate_has_offset() {
        let inst = Instruction {
            opcode: Opcode::Lddw,
            dst: Some(Register { n: 0 }),
            src: None,
            off: Some(Either::Right(10)),
            imm: Some(Either::Right(Number::Int(42))),
            span: 0..8,
        };
        let result = validate_load_immediate(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register and immediate value only",
                    Opcode::Lddw
                )
            );
        }
    }

    #[test]
    fn test_validate_load_memory_valid() {
        let valid_inst = Instruction {
            opcode: Opcode::Ldxw,
            dst: Some(Register { n: 0 }),
            src: Some(Register { n: 1 }),
            off: Some(Either::Right(8)),
            imm: None,
            span: 0..8,
        };
        assert!(validate_load_memory(&valid_inst).is_ok());
    }

    #[test]
    fn test_validate_load_memory_missing_dst() {
        let inst = Instruction {
            opcode: Opcode::Ldxw,
            dst: None,
            src: Some(Register { n: 1 }),
            off: Some(Either::Right(8)),
            imm: None,
            span: 0..8,
        };
        let result = validate_load_memory(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register, source register, offset, and \
                     no immediate value",
                    Opcode::Ldxw
                )
            );
        }
    }

    #[test]
    fn test_validate_load_memory_has_imm() {
        let inst = Instruction {
            opcode: Opcode::Ldxw,
            dst: Some(Register { n: 0 }),
            src: Some(Register { n: 1 }),
            off: Some(Either::Right(8)),
            imm: Some(Either::Right(Number::Int(42))),
            span: 0..8,
        };
        let result = validate_load_memory(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register, source register, offset, and \
                     no immediate value",
                    Opcode::Ldxw
                )
            );
        }
    }

    #[test]
    fn test_validate_load_memory_missing_src() {
        let inst = Instruction {
            opcode: Opcode::Ldxw,
            dst: Some(Register { n: 0 }),
            src: None,
            off: Some(Either::Right(8)),
            imm: None,
            span: 0..8,
        };
        let result = validate_load_memory(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register, source register, offset, and \
                     no immediate value",
                    Opcode::Ldxw
                )
            );
        }
    }

    #[test]
    fn test_validate_store_immediate_valid() {
        let valid_inst = Instruction {
            opcode: Opcode::Stw,
            dst: Some(Register { n: 0 }),
            src: None,
            off: Some(Either::Right(8)),
            imm: Some(Either::Right(Number::Int(42))),
            span: 0..8,
        };
        assert!(validate_store_immediate(&valid_inst).is_ok());
    }

    #[test]
    fn test_validate_store_immediate_missing_imm() {
        let inst = Instruction {
            opcode: Opcode::Stw,
            dst: Some(Register { n: 0 }),
            src: None,
            off: Some(Either::Right(8)),
            imm: None,
            span: 0..8,
        };
        let result = validate_store_immediate(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register, offset, immediate value, and \
                     no source register",
                    Opcode::Stw
                )
            );
        }
    }

    #[test]
    fn test_validate_store_immediate_has_src() {
        let inst = Instruction {
            opcode: Opcode::Stw,
            dst: Some(Register { n: 0 }),
            src: Some(Register { n: 1 }),
            off: Some(Either::Right(8)),
            imm: Some(Either::Right(Number::Int(42))),
            span: 0..8,
        };
        let result = validate_store_immediate(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register, offset, immediate value, and \
                     no source register",
                    Opcode::Stw
                )
            );
        }
    }

    #[test]
    fn test_validate_store_register_valid() {
        let valid_inst = Instruction {
            opcode: Opcode::Stxw,
            dst: Some(Register { n: 0 }),
            src: Some(Register { n: 1 }),
            off: Some(Either::Right(8)),
            imm: None,
            span: 0..8,
        };
        assert!(validate_store_register(&valid_inst).is_ok());
    }

    #[test]
    fn test_validate_store_register_missing_src() {
        let inst = Instruction {
            opcode: Opcode::Stxw,
            dst: Some(Register { n: 0 }),
            src: None,
            off: Some(Either::Right(8)),
            imm: None,
            span: 0..8,
        };
        let result = validate_store_register(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register, source register, offset, and \
                     no immediate value",
                    Opcode::Stxw
                )
            );
        }
    }

    #[test]
    fn test_validate_store_register_has_imm() {
        let inst = Instruction {
            opcode: Opcode::Stxw,
            dst: Some(Register { n: 0 }),
            src: Some(Register { n: 1 }),
            off: Some(Either::Right(8)),
            imm: Some(Either::Right(Number::Int(42))),
            span: 0..8,
        };
        let result = validate_store_register(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register, source register, offset, and \
                     no immediate value",
                    Opcode::Stxw
                )
            );
        }
    }

    #[test]
    fn test_validate_unary_valid() {
        let valid_inst = Instruction {
            opcode: Opcode::Neg64,
            dst: Some(Register { n: 0 }),
            src: None,
            off: None,
            imm: None,
            span: 0..8,
        };
        assert!(validate_unary(&valid_inst).is_ok());
    }

    #[test]
    fn test_validate_unary_missing_dst() {
        let inst = Instruction {
            opcode: Opcode::Neg64,
            dst: None,
            src: None,
            off: None,
            imm: None,
            span: 0..8,
        };
        let result = validate_unary(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register only",
                    Opcode::Neg64
                )
            );
        }
    }

    #[test]
    fn test_validate_unary_has_src() {
        let inst = Instruction {
            opcode: Opcode::Neg64,
            dst: Some(Register { n: 0 }),
            src: Some(Register { n: 1 }),
            off: None,
            imm: None,
            span: 0..8,
        };
        let result = validate_unary(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register only",
                    Opcode::Neg64
                )
            );
        }
    }

    #[test]
    fn test_validate_unary_has_offset() {
        let inst = Instruction {
            opcode: Opcode::Neg64,
            dst: Some(Register { n: 0 }),
            src: None,
            off: Some(Either::Right(10)),
            imm: None,
            span: 0..8,
        };
        let result = validate_unary(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register only",
                    Opcode::Neg64
                )
            );
        }
    }

    #[test]
    fn test_validate_unary_has_imm() {
        let inst = Instruction {
            opcode: Opcode::Neg64,
            dst: Some(Register { n: 0 }),
            src: None,
            off: None,
            imm: Some(Either::Right(Number::Int(42))),
            span: 0..8,
        };
        let result = validate_unary(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register only",
                    Opcode::Neg64
                )
            );
        }
    }

    #[test]
    fn test_validate_binary_immediate_valid() {
        let valid_inst = Instruction {
            opcode: Opcode::Le,
            dst: Some(Register { n: 0 }),
            src: None,
            off: None,
            imm: Some(Either::Right(Number::Int(16))),
            span: 0..8,
        };
        assert!(validate_binary_immediate(&valid_inst).is_ok());
    }

    #[test]
    fn test_validate_binary_immediate_missing_dst() {
        let inst = Instruction {
            opcode: Opcode::Le,
            dst: None,
            src: None,
            off: None,
            imm: Some(Either::Right(Number::Int(16))),
            span: 0..8,
        };
        let result = validate_binary_immediate(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register and immediate value only",
                    Opcode::Le
                )
            );
        }
    }

    #[test]
    fn test_validate_binary_immediate_missing_imm() {
        let inst = Instruction {
            opcode: Opcode::Le,
            dst: Some(Register { n: 0 }),
            src: None,
            off: None,
            imm: None,
            span: 0..8,
        };
        let result = validate_binary_immediate(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register and immediate value only",
                    Opcode::Le
                )
            );
        }
    }

    #[test]
    fn test_validate_binary_immediate_has_src() {
        let inst = Instruction {
            opcode: Opcode::Le,
            dst: Some(Register { n: 0 }),
            src: Some(Register { n: 1 }),
            off: None,
            imm: Some(Either::Right(Number::Int(16))),
            span: 0..8,
        };
        let result = validate_binary_immediate(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register and immediate value only",
                    Opcode::Le
                )
            );
        }
    }

    #[test]
    fn test_validate_binary_immediate_has_offset() {
        let inst = Instruction {
            opcode: Opcode::Le,
            dst: Some(Register { n: 0 }),
            src: None,
            off: Some(Either::Right(10)),
            imm: Some(Either::Right(Number::Int(16))),
            span: 0..8,
        };
        let result = validate_binary_immediate(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register and immediate value only",
                    Opcode::Le
                )
            );
        }
    }

    #[test]
    fn test_validate_binary_register_valid() {
        let valid_inst = Instruction {
            opcode: Opcode::Add64Reg,
            dst: Some(Register { n: 0 }),
            src: Some(Register { n: 1 }),
            off: None,
            imm: None,
            span: 0..8,
        };
        assert!(validate_binary_register(&valid_inst).is_ok());
    }

    #[test]
    fn test_validate_binary_register_missing_dst() {
        let inst = Instruction {
            opcode: Opcode::Add64Reg,
            dst: None,
            src: Some(Register { n: 1 }),
            off: None,
            imm: None,
            span: 0..8,
        };
        let result = validate_binary_register(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination and source registers only",
                    Opcode::Add64Reg
                )
            );
        }
    }

    #[test]
    fn test_validate_binary_register_missing_src() {
        let inst = Instruction {
            opcode: Opcode::Add64Reg,
            dst: Some(Register { n: 0 }),
            src: None,
            off: None,
            imm: None,
            span: 0..8,
        };
        let result = validate_binary_register(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination and source registers only",
                    Opcode::Add64Reg
                )
            );
        }
    }

    #[test]
    fn test_validate_binary_register_has_offset() {
        let inst = Instruction {
            opcode: Opcode::Add64Reg,
            dst: Some(Register { n: 0 }),
            src: Some(Register { n: 1 }),
            off: Some(Either::Right(10)),
            imm: None,
            span: 0..8,
        };
        let result = validate_binary_register(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination and source registers only",
                    Opcode::Add64Reg
                )
            );
        }
    }

    #[test]
    fn test_validate_binary_register_has_imm() {
        let inst = Instruction {
            opcode: Opcode::Add64Reg,
            dst: Some(Register { n: 0 }),
            src: Some(Register { n: 1 }),
            off: None,
            imm: Some(Either::Right(Number::Int(42))),
            span: 0..8,
        };
        let result = validate_binary_register(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination and source registers only",
                    Opcode::Add64Reg
                )
            );
        }
    }

    #[test]
    fn test_validate_jump_valid() {
        let valid_inst = Instruction {
            opcode: Opcode::Ja,
            dst: None,
            src: None,
            off: Some(Either::Right(10)),
            imm: None,
            span: 0..8,
        };
        assert!(validate_jump(&valid_inst).is_ok());
    }

    #[test]
    fn test_validate_jump_valid_with_dst() {
        let valid_inst = Instruction {
            opcode: Opcode::Ja,
            dst: Some(Register { n: 0 }),
            src: None,
            off: Some(Either::Right(10)),
            imm: None,
            span: 0..8,
        };
        assert!(validate_jump(&valid_inst).is_ok());
    }

    #[test]
    fn test_validate_jump_has_src() {
        let inst = Instruction {
            opcode: Opcode::Ja,
            dst: None,
            src: Some(Register { n: 1 }),
            off: Some(Either::Right(10)),
            imm: None,
            span: 0..8,
        };
        let result = validate_jump(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires no source register or immediate value",
                    Opcode::Ja
                )
            );
        }
    }

    #[test]
    fn test_validate_jump_has_imm() {
        let inst = Instruction {
            opcode: Opcode::Ja,
            dst: None,
            src: None,
            off: Some(Either::Right(10)),
            imm: Some(Either::Right(Number::Int(42))),
            span: 0..8,
        };
        let result = validate_jump(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires no source register or immediate value",
                    Opcode::Ja
                )
            );
        }
    }

    #[test]
    fn test_validate_jump_immediate_valid() {
        let valid_inst = Instruction {
            opcode: Opcode::JeqImm,
            dst: Some(Register { n: 0 }),
            src: None,
            off: Some(Either::Right(10)),
            imm: Some(Either::Right(Number::Int(42))),
            span: 0..8,
        };
        assert!(validate_jump_immediate(&valid_inst).is_ok());
    }

    #[test]
    fn test_validate_jump_immediate_missing_dst() {
        let inst = Instruction {
            opcode: Opcode::JeqImm,
            dst: None,
            src: None,
            off: Some(Either::Right(10)),
            imm: Some(Either::Right(Number::Int(42))),
            span: 0..8,
        };
        let result = validate_jump_immediate(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register and immediate value, and no \
                     source register",
                    Opcode::JeqImm
                )
            );
        }
    }

    #[test]
    fn test_validate_jump_immediate_missing_imm() {
        let inst = Instruction {
            opcode: Opcode::JeqImm,
            dst: Some(Register { n: 0 }),
            src: None,
            off: Some(Either::Right(10)),
            imm: None,
            span: 0..8,
        };
        let result = validate_jump_immediate(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register and immediate value, and no \
                     source register",
                    Opcode::JeqImm
                )
            );
        }
    }

    #[test]
    fn test_validate_jump_register_valid() {
        let valid_inst = Instruction {
            opcode: Opcode::JeqReg,
            dst: Some(Register { n: 0 }),
            src: Some(Register { n: 1 }),
            off: Some(Either::Right(10)),
            imm: None,
            span: 0..8,
        };
        assert!(validate_jump_register(&valid_inst).is_ok());
    }

    #[test]
    fn test_validate_jump_register_missing_dst() {
        let inst = Instruction {
            opcode: Opcode::JeqReg,
            dst: None,
            src: Some(Register { n: 1 }),
            off: Some(Either::Right(10)),
            imm: None,
            span: 0..8,
        };
        let result = validate_jump_register(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register, source register, and offset",
                    Opcode::JeqReg
                )
            );
        }
    }

    #[test]
    fn test_validate_jump_register_missing_src() {
        let inst = Instruction {
            opcode: Opcode::JeqReg,
            dst: Some(Register { n: 0 }),
            src: None,
            off: Some(Either::Right(10)),
            imm: None,
            span: 0..8,
        };
        let result = validate_jump_register(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register, source register, and offset",
                    Opcode::JeqReg
                )
            );
        }
    }

    #[test]
    fn test_validate_jump_register_missing_offset() {
        let inst = Instruction {
            opcode: Opcode::JeqReg,
            dst: Some(Register { n: 0 }),
            src: Some(Register { n: 1 }),
            off: None,
            imm: None,
            span: 0..8,
        };
        let result = validate_jump_register(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register, source register, and offset",
                    Opcode::JeqReg
                )
            );
        }
    }

    #[test]
    fn test_validate_jump_register_has_imm() {
        let inst = Instruction {
            opcode: Opcode::JeqReg,
            dst: Some(Register { n: 0 }),
            src: Some(Register { n: 1 }),
            off: Some(Either::Right(10)),
            imm: Some(Either::Right(Number::Int(42))),
            span: 0..8,
        };
        let result = validate_jump_register(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires destination register, source register, and offset",
                    Opcode::JeqReg
                )
            );
        }
    }

    #[test]
    fn test_validate_call_immediate_valid() {
        let valid_inst = Instruction {
            opcode: Opcode::Call,
            dst: None,
            src: None,
            off: None,
            imm: Some(Either::Right(Number::Int(100))),
            span: 0..8,
        };
        assert!(validate_call_immediate(&valid_inst).is_ok());
    }

    #[test]
    fn test_validate_call_immediate_missing_imm() {
        let inst = Instruction {
            opcode: Opcode::Call,
            dst: None,
            src: None,
            off: None,
            imm: None,
            span: 0..8,
        };
        let result = validate_call_immediate(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!("{} instruction requires immediate value only", Opcode::Call)
            );
        }
    }

    #[test]
    fn test_validate_call_immediate_has_dst() {
        let inst = Instruction {
            opcode: Opcode::Call,
            dst: Some(Register { n: 0 }),
            src: None,
            off: None,
            imm: Some(Either::Right(Number::Int(100))),
            span: 0..8,
        };
        let result = validate_call_immediate(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!("{} instruction requires immediate value only", Opcode::Call)
            );
        }
    }

    #[test]
    fn test_validate_call_immediate_has_src() {
        let inst = Instruction {
            opcode: Opcode::Call,
            dst: None,
            src: Some(Register { n: 1 }),
            off: None,
            imm: Some(Either::Right(Number::Int(100))),
            span: 0..8,
        };
        let result = validate_call_immediate(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!("{} instruction requires immediate value only", Opcode::Call)
            );
        }
    }

    #[test]
    fn test_validate_call_immediate_has_offset() {
        let inst = Instruction {
            opcode: Opcode::Call,
            dst: None,
            src: None,
            off: Some(Either::Right(10)),
            imm: Some(Either::Right(Number::Int(100))),
            span: 0..8,
        };
        let result = validate_call_immediate(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!("{} instruction requires immediate value only", Opcode::Call)
            );
        }
    }

    #[test]
    fn test_validate_call_register_valid() {
        let valid_inst = Instruction {
            opcode: Opcode::Callx,
            dst: None,
            src: Some(Register { n: 1 }),
            off: None,
            imm: None,
            span: 0..8,
        };
        assert!(validate_call_register(&valid_inst).is_ok());
    }

    #[test]
    fn test_validate_call_register_missing_src() {
        let inst = Instruction {
            opcode: Opcode::Callx,
            dst: None,
            src: None,
            off: None,
            imm: None,
            span: 0..8,
        };
        let result = validate_call_register(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires source register only",
                    Opcode::Callx
                )
            );
        }
    }

    #[test]
    fn test_validate_call_register_has_dst() {
        let inst = Instruction {
            opcode: Opcode::Callx,
            dst: Some(Register { n: 0 }),
            src: Some(Register { n: 1 }),
            off: None,
            imm: None,
            span: 0..8,
        };
        let result = validate_call_register(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires source register only",
                    Opcode::Callx
                )
            );
        }
    }

    #[test]
    fn test_validate_call_register_has_offset() {
        let inst = Instruction {
            opcode: Opcode::Callx,
            dst: None,
            src: Some(Register { n: 1 }),
            off: Some(Either::Right(10)),
            imm: None,
            span: 0..8,
        };
        let result = validate_call_register(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires source register only",
                    Opcode::Callx
                )
            );
        }
    }

    #[test]
    fn test_validate_call_register_has_imm() {
        let inst = Instruction {
            opcode: Opcode::Callx,
            dst: None,
            src: Some(Register { n: 1 }),
            off: None,
            imm: Some(Either::Right(Number::Int(100))),
            span: 0..8,
        };
        let result = validate_call_register(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!(
                    "{} instruction requires source register only",
                    Opcode::Callx
                )
            );
        }
    }

    #[test]
    fn test_validate_exit_valid() {
        let valid_inst = Instruction {
            opcode: Opcode::Exit,
            dst: None,
            src: None,
            off: None,
            imm: None,
            span: 0..8,
        };
        assert!(validate_exit(&valid_inst).is_ok());
    }

    #[test]
    fn test_validate_exit_has_dst() {
        let inst = Instruction {
            opcode: Opcode::Exit,
            dst: Some(Register { n: 0 }),
            src: None,
            off: None,
            imm: None,
            span: 0..8,
        };
        let result = validate_exit(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!("{} instruction requires no operands", Opcode::Exit)
            );
        }
    }

    #[test]
    fn test_validate_exit_has_src() {
        let inst = Instruction {
            opcode: Opcode::Exit,
            dst: None,
            src: Some(Register { n: 1 }),
            off: None,
            imm: None,
            span: 0..8,
        };
        let result = validate_exit(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!("{} instruction requires no operands", Opcode::Exit)
            );
        }
    }

    #[test]
    fn test_validate_exit_has_offset() {
        let inst = Instruction {
            opcode: Opcode::Exit,
            dst: None,
            src: None,
            off: Some(Either::Right(10)),
            imm: None,
            span: 0..8,
        };
        let result = validate_exit(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!("{} instruction requires no operands", Opcode::Exit)
            );
        }
    }

    #[test]
    fn test_validate_exit_has_imm() {
        let inst = Instruction {
            opcode: Opcode::Exit,
            dst: None,
            src: None,
            off: None,
            imm: Some(Either::Right(Number::Int(0))),
            span: 0..8,
        };
        let result = validate_exit(&inst);
        assert!(result.is_err());
        if let Err(SBPFError::BytecodeError { error, .. }) = result {
            assert_eq!(
                error,
                format!("{} instruction requires no operands", Opcode::Exit)
            );
        }
    }
}
