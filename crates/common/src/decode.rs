use {
    crate::{
        errors::SBPFError,
        inst_param::{Number, Register},
        instruction::Instruction,
        opcode::Opcode,
        syscalls::SYSCALLS,
    },
    either::Either,
};

// TODO: passing span for error reporting (not sure if it's necessary)

#[inline]
fn parse_bytes(bytes: &[u8]) -> Result<(Opcode, u8, u8, i16, i32), SBPFError> {
    let opcode: Opcode = bytes[0].try_into()?;
    let reg = bytes[1];
    let dst = reg & 0x0f;
    let src = reg >> 4;
    let off = i16::from_le_bytes([bytes[2], bytes[3]]);
    let imm = i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    Ok((opcode, dst, src, off, imm))
}

pub fn decode_load_immediate(bytes: &[u8]) -> Result<Instruction, SBPFError> {
    assert!(bytes.len() >= 16);
    let (opcode, dst, src, off, imm_low) = parse_bytes(bytes)?;
    if src != 0 || off != 0 {
        return Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction has src: {}, off: {} supposed to be zero",
                opcode, src, off
            ),
            span: 0..16,
            custom_label: None,
        });
    }
    let imm_high = i32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    let imm = ((imm_high as i64) << 32) | (imm_low as u32 as i64);
    Ok(Instruction {
        opcode,
        dst: Some(Register { n: dst }),
        src: None,
        off: None,
        imm: Some(Either::Right(Number::Int(imm))),
        span: 0..16,
    })
}

pub fn decode_load_memory(bytes: &[u8]) -> Result<Instruction, SBPFError> {
    assert!(bytes.len() >= 8);
    let (opcode, dst, src, off, imm) = parse_bytes(bytes)?;
    if imm != 0 {
        return Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction has imm: {} supposed to be zero",
                opcode, imm
            ),
            span: 0..8,
            custom_label: None,
        });
    }
    Ok(Instruction {
        opcode,
        dst: Some(Register { n: dst }),
        src: Some(Register { n: src }),
        off: Some(Either::Right(off)),
        imm: None,
        span: 0..8,
    })
}

pub fn decode_store_immediate(bytes: &[u8]) -> Result<Instruction, SBPFError> {
    assert!(bytes.len() >= 8);
    let (opcode, dst, src, off, imm) = parse_bytes(bytes)?;
    if src != 0 {
        return Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction has src: {} supposed to be zero",
                opcode, src
            ),
            span: 0..8,
            custom_label: None,
        });
    }
    Ok(Instruction {
        opcode,
        dst: Some(Register { n: dst }),
        src: None,
        off: Some(Either::Right(off)),
        imm: Some(Either::Right(Number::Int(imm.into()))),
        span: 0..8,
    })
}

pub fn decode_store_register(bytes: &[u8]) -> Result<Instruction, SBPFError> {
    assert!(bytes.len() >= 8);
    let (opcode, dst, src, off, imm) = parse_bytes(bytes)?;
    if imm != 0 {
        return Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction has imm: {} supposed to be zero",
                opcode, imm
            ),
            span: 0..8,
            custom_label: None,
        });
    }
    Ok(Instruction {
        opcode,
        dst: Some(Register { n: dst }),
        src: Some(Register { n: src }),
        off: Some(Either::Right(off)),
        imm: None,
        span: 0..8,
    })
}

pub fn decode_binary_immediate(bytes: &[u8]) -> Result<Instruction, SBPFError> {
    assert!(bytes.len() >= 8);
    let (opcode, dst, src, off, imm) = parse_bytes(bytes)?;
    if src != 0 || off != 0 {
        return Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction has src: {}, off: {} supposed to be zeros",
                opcode, src, off
            ),
            span: 0..8,
            custom_label: None,
        });
    }
    Ok(Instruction {
        opcode,
        dst: Some(Register { n: dst }),
        src: None,
        off: None,
        imm: Some(Either::Right(Number::Int(imm.into()))),
        span: 0..8,
    })
}

pub fn decode_binary_register(bytes: &[u8]) -> Result<Instruction, SBPFError> {
    assert!(bytes.len() >= 8);
    let (opcode, dst, src, off, imm) = parse_bytes(bytes)?;
    if off != 0 || imm != 0 {
        return Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction has off: {}, imm: {} supposed to be zeros",
                opcode, off, imm
            ),
            span: 0..8,
            custom_label: None,
        });
    }
    Ok(Instruction {
        opcode,
        dst: Some(Register { n: dst }),
        src: Some(Register { n: src }),
        off: None,
        imm: None,
        span: 0..8,
    })
}

pub fn decode_unary(bytes: &[u8]) -> Result<Instruction, SBPFError> {
    assert!(bytes.len() >= 8);
    let (opcode, dst, src, off, imm) = parse_bytes(bytes)?;
    if src != 0 || off != 0 || imm != 0 {
        return Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction has src: {}, off: {}, imm: {} supposed to be zeros",
                opcode, src, off, imm
            ),
            span: 0..8,
            custom_label: None,
        });
    }
    Ok(Instruction {
        opcode,
        dst: Some(Register { n: dst }),
        src: None,
        off: None,
        imm: None,
        span: 0..8,
    })
}

pub fn decode_jump(bytes: &[u8]) -> Result<Instruction, SBPFError> {
    assert!(bytes.len() >= 8);
    let (opcode, dst, src, off, imm) = parse_bytes(bytes)?;
    if dst != 0 || src != 0 || imm != 0 {
        return Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction has dst: {}, src: {}, imm: {} supposed to be zeros",
                opcode, dst, src, imm
            ),
            span: 0..8,
            custom_label: None,
        });
    }
    Ok(Instruction {
        opcode,
        dst: None,
        src: None,
        off: Some(Either::Right(off)),
        imm: None,
        span: 0..8,
    })
}

pub fn decode_jump_immediate(bytes: &[u8]) -> Result<Instruction, SBPFError> {
    assert!(bytes.len() >= 8);
    let (opcode, dst, src, off, imm) = parse_bytes(bytes)?;
    if src != 0 {
        return Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction has src: {} supposed to be zero",
                opcode, src
            ),
            span: 0..8,
            custom_label: None,
        });
    }
    Ok(Instruction {
        opcode,
        dst: Some(Register { n: dst }),
        src: None,
        off: Some(Either::Right(off)),
        imm: Some(Either::Right(Number::Int(imm.into()))),
        span: 0..8,
    })
}

pub fn decode_jump_register(bytes: &[u8]) -> Result<Instruction, SBPFError> {
    assert!(bytes.len() >= 8);
    let (opcode, dst, src, off, imm) = parse_bytes(bytes)?;
    if imm != 0 {
        return Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction has imm: {} supposed to be zero",
                opcode, imm
            ),
            span: 0..8,
            custom_label: None,
        });
    }
    Ok(Instruction {
        opcode,
        dst: Some(Register { n: dst }),
        src: Some(Register { n: src }),
        off: Some(Either::Right(off)),
        imm: None,
        span: 0..8,
    })
}

pub fn decode_call_immediate(bytes: &[u8]) -> Result<Instruction, SBPFError> {
    assert!(bytes.len() >= 8);
    let (opcode, dst, src, off, imm) = parse_bytes(bytes)?;

    if dst != 0 || off != 0 {
        return Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction has dst: {} and off: {}, expected to be zeros",
                opcode, dst, off
            ),
            span: 0..8,
            custom_label: None,
        });
    }

    let callimm = match src {
        0 => {
            // Static syscall - check if syscall is registered.
            if let Some(syscall) = SYSCALLS.get(imm as u32) {
                Either::Left(syscall.to_string())
            } else {
                Either::Right(Number::Int(imm.into()))
            }
        }
        1 => Either::Right(Number::Int(imm.into())),
        _ => {
            return Err(SBPFError::BytecodeError {
                error: format!("{} instruction has invalid src register: {}", opcode, src),
                span: 0..8,
                custom_label: None,
            });
        }
    };

    Ok(Instruction {
        opcode,
        dst: None,
        src: Some(Register { n: src }),
        off: None,
        imm: Some(callimm),
        span: 0..8,
    })
}

pub fn decode_call_register(bytes: &[u8]) -> Result<Instruction, SBPFError> {
    assert!(bytes.len() >= 8);
    let (opcode, dst, src, off, imm) = parse_bytes(bytes)?;
    // Handle SBPF Callx normalization
    let (dst, imm) = if dst == 0 && imm != 0 {
        (imm as u8, 0)
    } else {
        (dst, 0)
    };

    // TODO: sbpf encodes dst_reg in immediate
    if src != 0 || off != 0 || imm != 0 {
        return Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction has src: {}, off: {}, imm: {} supposed to be zeros",
                opcode, src, off, imm
            ),
            span: 0..8,
            custom_label: None,
        });
    }
    Ok(Instruction {
        opcode,
        dst: Some(Register { n: dst }),
        src: None,
        off: None,
        imm: None,
        span: 0..8,
    })
}

pub fn decode_exit(bytes: &[u8]) -> Result<Instruction, SBPFError> {
    assert!(bytes.len() >= 8);
    let (opcode, dst, src, off, imm) = parse_bytes(bytes)?;
    if dst != 0 || src != 0 || off != 0 || imm != 0 {
        return Err(SBPFError::BytecodeError {
            error: format!(
                "{} instruction dst: {}, src: {}, off: {}, imm: {} supposed to be zero",
                opcode, dst, src, off, imm
            ),
            span: 0..8,
            custom_label: None,
        });
    }
    Ok(Instruction {
        opcode,
        dst: None,
        src: None,
        off: None,
        imm: None,
        span: 0..8,
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{syscalls::REGISTERED_SYSCALLS, syscalls_map::murmur3_32},
    };

    #[test]
    fn test_decode_load_immediate_valid() {
        // lddw r1, 0x123456789abcdef0
        let mut bytes = vec![0x18, 0x01, 0x00, 0x00, 0xf0, 0xde, 0xbc, 0x9a];
        bytes.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x78, 0x56, 0x34, 0x12]);

        let result = decode_load_immediate(&bytes).unwrap();
        assert_eq!(result.opcode, Opcode::Lddw);
        assert_eq!(result.dst.unwrap().n, 1);
        assert!(result.src.is_none());
        assert!(result.off.is_none());
        assert_eq!(result.span, 0..16);
    }

    #[test]
    fn test_decode_load_immediate_error_nonzero_src() {
        let mut bytes = vec![0x18, 0x11, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        bytes.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);

        let result = decode_load_immediate(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_load_immediate_error_nonzero_off() {
        let mut bytes = vec![0x18, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00];
        bytes.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);

        let result = decode_load_immediate(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_load_memory_valid() {
        // ldxw r2, [r3+10]
        let bytes = vec![0x61, 0x32, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00];

        let result = decode_load_memory(&bytes).unwrap();
        assert_eq!(result.opcode, Opcode::Ldxw);
        assert_eq!(result.dst.unwrap().n, 2);
        assert_eq!(result.src.unwrap().n, 3);
        assert_eq!(result.span, 0..8);
    }

    #[test]
    fn test_decode_load_memory_error_nonzero_imm() {
        let bytes = vec![0x61, 0x32, 0x0a, 0x00, 0x01, 0x00, 0x00, 0x00];

        let result = decode_load_memory(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_store_immediate_valid() {
        // stw [r1+4], 100
        let bytes = vec![0x62, 0x01, 0x04, 0x00, 0x64, 0x00, 0x00, 0x00];

        let result = decode_store_immediate(&bytes).unwrap();
        assert_eq!(result.opcode, Opcode::Stw);
        assert_eq!(result.dst.unwrap().n, 1);
        assert!(result.src.is_none());
        assert_eq!(result.span, 0..8);
    }

    #[test]
    fn test_decode_store_immediate_error_nonzero_src() {
        let bytes = vec![0x62, 0x11, 0x04, 0x00, 0x64, 0x00, 0x00, 0x00];

        let result = decode_store_immediate(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_store_register_valid() {
        // stxw [r1+4], r2
        let bytes = vec![0x63, 0x21, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00];

        let result = decode_store_register(&bytes).unwrap();
        assert_eq!(result.opcode, Opcode::Stxw);
        assert_eq!(result.dst.unwrap().n, 1);
        assert_eq!(result.src.unwrap().n, 2);
        assert_eq!(result.span, 0..8);
    }

    #[test]
    fn test_decode_store_register_error_nonzero_imm() {
        let bytes = vec![0x63, 0x21, 0x04, 0x00, 0x01, 0x00, 0x00, 0x00];

        let result = decode_store_register(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_binary_immediate_valid() {
        // add32 r1, 100
        let bytes = vec![0x04, 0x01, 0x00, 0x00, 0x64, 0x00, 0x00, 0x00];

        let result = decode_binary_immediate(&bytes).unwrap();
        assert_eq!(result.opcode, Opcode::Add32Imm);
        assert_eq!(result.dst.unwrap().n, 1);
        assert!(result.src.is_none());
        assert!(result.off.is_none());
        assert_eq!(result.span, 0..8);
    }

    #[test]
    fn test_decode_binary_immediate_error_nonzero_src() {
        let bytes = vec![0x04, 0x11, 0x00, 0x00, 0x64, 0x00, 0x00, 0x00];

        let result = decode_binary_immediate(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_binary_immediate_error_nonzero_off() {
        let bytes = vec![0x04, 0x01, 0x01, 0x00, 0x64, 0x00, 0x00, 0x00];

        let result = decode_binary_immediate(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_binary_register_valid() {
        // add32 r1, r2
        let bytes = vec![0x0c, 0x21, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

        let result = decode_binary_register(&bytes).unwrap();
        assert_eq!(result.opcode, Opcode::Add32Reg);
        assert_eq!(result.dst.unwrap().n, 1);
        assert_eq!(result.src.unwrap().n, 2);
        assert!(result.off.is_none());
        assert!(result.imm.is_none());
        assert_eq!(result.span, 0..8);
    }

    #[test]
    fn test_decode_binary_register_error_nonzero_off() {
        let bytes = vec![0x0c, 0x21, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00];

        let result = decode_binary_register(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_binary_register_error_nonzero_imm() {
        let bytes = vec![0x0c, 0x21, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];

        let result = decode_binary_register(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_unary_valid() {
        // neg64 r1
        let bytes = vec![0x87, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

        let result = decode_unary(&bytes).unwrap();
        assert_eq!(result.opcode, Opcode::Neg64);
        assert_eq!(result.dst.unwrap().n, 1);
        assert!(result.src.is_none());
        assert!(result.off.is_none());
        assert!(result.imm.is_none());
        assert_eq!(result.span, 0..8);
    }

    #[test]
    fn test_decode_unary_error_nonzero_src() {
        let bytes = vec![0x87, 0x11, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

        let result = decode_unary(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_unary_error_nonzero_off() {
        let bytes = vec![0x87, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00];

        let result = decode_unary(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_unary_error_nonzero_imm() {
        let bytes = vec![0x87, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];

        let result = decode_unary(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_jump_valid() {
        // ja +10
        let bytes = vec![0x05, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00];

        let result = decode_jump(&bytes).unwrap();
        assert_eq!(result.opcode, Opcode::Ja);
        assert!(result.dst.is_none());
        assert!(result.src.is_none());
        assert!(result.imm.is_none());
        assert_eq!(result.span, 0..8);
    }

    #[test]
    fn test_decode_jump_error_nonzero_dst() {
        let bytes = vec![0x05, 0x01, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00];

        let result = decode_jump(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_jump_error_nonzero_src() {
        let bytes = vec![0x05, 0x10, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00];

        let result = decode_jump(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_jump_error_nonzero_imm() {
        let bytes = vec![0x05, 0x00, 0x0a, 0x00, 0x01, 0x00, 0x00, 0x00];

        let result = decode_jump(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_jump_immediate_valid() {
        // jeq r1, 100, +10
        let bytes = vec![0x15, 0x01, 0x0a, 0x00, 0x64, 0x00, 0x00, 0x00];

        let result = decode_jump_immediate(&bytes).unwrap();
        assert_eq!(result.opcode, Opcode::JeqImm);
        assert_eq!(result.dst.unwrap().n, 1);
        assert!(result.src.is_none());
        assert_eq!(result.span, 0..8);
    }

    #[test]
    fn test_decode_jump_immediate_error_nonzero_src() {
        let bytes = vec![0x15, 0x11, 0x0a, 0x00, 0x64, 0x00, 0x00, 0x00];

        let result = decode_jump_immediate(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_jump_register_valid() {
        // jeq r1, r2, +10
        let bytes = vec![0x1d, 0x21, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00];

        let result = decode_jump_register(&bytes).unwrap();
        assert_eq!(result.opcode, Opcode::JeqReg);
        assert_eq!(result.dst.unwrap().n, 1);
        assert_eq!(result.src.unwrap().n, 2);
        assert_eq!(result.span, 0..8);
    }

    #[test]
    fn test_decode_jump_register_error_nonzero_imm() {
        let bytes = vec![0x1d, 0x21, 0x0a, 0x00, 0x01, 0x00, 0x00, 0x00];

        let result = decode_jump_register(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_call_immediate_valid_regular() {
        // call 100 (non-syscall) - src must be 1
        let bytes = vec![0x85, 0x10, 0x00, 0x00, 0x64, 0x00, 0x00, 0x00];

        let result = decode_call_immediate(&bytes).unwrap();
        assert_eq!(result.opcode, Opcode::Call);
        assert!(result.dst.is_none());
        assert_eq!(result.src.unwrap().n, 1);
        assert_eq!(result.span, 0..8);
    }

    #[test]
    fn test_decode_call_immediate_error_invalid_regular() {
        // Invalid: dst != 0 for non-syscall
        let bytes = vec![0x85, 0x01, 0x00, 0x00, 0x64, 0x00, 0x00, 0x00];

        let result = decode_call_immediate(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_call_immediate_valid_syscall() {
        let syscall_name = REGISTERED_SYSCALLS[0];
        let syscall_hash = murmur3_32(syscall_name);
        let hash_bytes = syscall_hash.to_le_bytes();

        // Build call instruction with syscall hash and all zeros for dst, src, off
        let bytes = vec![
            0x85,
            0x00,
            0x00,
            0x00,
            hash_bytes[0],
            hash_bytes[1],
            hash_bytes[2],
            hash_bytes[3],
        ];

        let result = decode_call_immediate(&bytes).unwrap();
        assert_eq!(result.opcode, Opcode::Call);
        assert!(result.dst.is_none());
        assert_eq!(result.src.unwrap().n, 0);
    }

    #[test]
    fn test_decode_call_immediate_syscall_error_nonzero_dst() {
        let syscall_hash = murmur3_32(REGISTERED_SYSCALLS[0]);
        let hash_bytes = syscall_hash.to_le_bytes();

        let bytes = vec![
            0x85,
            0x01,
            0x00,
            0x00,
            hash_bytes[0],
            hash_bytes[1],
            hash_bytes[2],
            hash_bytes[3],
        ];

        let result = decode_call_immediate(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_call_immediate_syscall_nonzero_src() {
        let syscall_hash = murmur3_32(REGISTERED_SYSCALLS[0]);
        let hash_bytes = syscall_hash.to_le_bytes();

        let bytes = vec![
            0x85,
            0x10,
            0x00,
            0x00,
            hash_bytes[0],
            hash_bytes[1],
            hash_bytes[2],
            hash_bytes[3],
        ];

        let result = decode_call_immediate(&bytes).unwrap();
        assert_eq!(result.src.unwrap().n, 1);
        assert!(matches!(result.imm, Some(Either::Right(_))));
    }

    #[test]
    fn test_decode_call_immediate_syscall_error_nonzero_off() {
        let syscall_hash = murmur3_32(REGISTERED_SYSCALLS[0]);
        let hash_bytes = syscall_hash.to_le_bytes();

        let bytes = vec![
            0x85,
            0x00,
            0x01,
            0x00,
            hash_bytes[0],
            hash_bytes[1],
            hash_bytes[2],
            hash_bytes[3],
        ];
        let result = decode_call_immediate(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_call_register_valid() {
        // callx r1
        let bytes = vec![0x8d, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

        let result = decode_call_register(&bytes).unwrap();
        assert_eq!(result.opcode, Opcode::Callx);
        assert_eq!(result.dst.unwrap().n, 1);
        assert!(result.src.is_none());
        assert!(result.off.is_none());
        assert!(result.imm.is_none());
        assert_eq!(result.span, 0..8);
    }

    #[test]
    fn test_decode_call_register_normalized() {
        // callx with dst in imm (sBPF normalization)
        let bytes = vec![0x8d, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00];
        let result = decode_call_register(&bytes).unwrap();
        assert_eq!(result.dst.unwrap().n, 5);
    }

    #[test]
    fn test_decode_call_register_error_nonzero_src() {
        let bytes = vec![0x8d, 0x11, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

        let result = decode_call_register(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_call_register_error_nonzero_off() {
        let bytes = vec![0x8d, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00];

        let result = decode_call_register(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_exit_valid() {
        // exit
        let bytes = vec![0x95, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

        let result = decode_exit(&bytes).unwrap();
        assert_eq!(result.opcode, Opcode::Exit);
        assert!(result.dst.is_none());
        assert!(result.src.is_none());
        assert!(result.off.is_none());
        assert!(result.imm.is_none());
        assert_eq!(result.span, 0..8);
    }

    #[test]
    fn test_decode_exit_error_nonzero_dst() {
        let bytes = vec![0x95, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

        let result = decode_exit(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_exit_error_nonzero_src() {
        let bytes = vec![0x95, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

        let result = decode_exit(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_exit_error_nonzero_off() {
        let bytes = vec![0x95, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00];

        let result = decode_exit(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_exit_error_nonzero_imm() {
        let bytes = vec![0x95, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];

        let result = decode_exit(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_all_load_memory_opcodes() {
        let opcodes = vec![
            (0x71, Opcode::Ldxb),
            (0x69, Opcode::Ldxh),
            (0x61, Opcode::Ldxw),
            (0x79, Opcode::Ldxdw),
        ];

        for (byte, expected_opcode) in opcodes {
            let bytes = vec![byte, 0x21, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00];
            let result = decode_load_memory(&bytes).unwrap();
            assert_eq!(result.opcode, expected_opcode);
        }
    }

    #[test]
    fn test_all_store_immediate_opcodes() {
        let opcodes = vec![
            (0x72, Opcode::Stb),
            (0x6a, Opcode::Sth),
            (0x62, Opcode::Stw),
            (0x7a, Opcode::Stdw),
        ];

        for (byte, expected_opcode) in opcodes {
            let bytes = vec![byte, 0x01, 0x04, 0x00, 0x64, 0x00, 0x00, 0x00];
            let result = decode_store_immediate(&bytes).unwrap();
            assert_eq!(result.opcode, expected_opcode);
        }
    }

    #[test]
    fn test_all_store_register_opcodes() {
        let opcodes = vec![
            (0x73, Opcode::Stxb),
            (0x6b, Opcode::Stxh),
            (0x63, Opcode::Stxw),
            (0x7b, Opcode::Stxdw),
        ];

        for (byte, expected_opcode) in opcodes {
            let bytes = vec![byte, 0x21, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00];
            let result = decode_store_register(&bytes).unwrap();
            assert_eq!(result.opcode, expected_opcode);
        }
    }

    #[test]
    fn test_decode_alu64_operations() {
        let ops = vec![
            (0x07, Opcode::Add64Imm),
            (0x0f, Opcode::Add64Reg),
            (0x17, Opcode::Sub64Imm),
            (0x1f, Opcode::Sub64Reg),
        ];

        for (byte, expected_opcode) in ops {
            let mut bytes = vec![byte, 0x01, 0x00, 0x00, 0x64, 0x00, 0x00, 0x00];

            if byte & 0x08 == 0 {
                // Immediate
                let result = decode_binary_immediate(&bytes).unwrap();
                assert_eq!(result.opcode, expected_opcode);
            } else {
                // Register
                bytes[4] = 0x00; // imm must be 0 for reg ops
                bytes[1] = 0x21; // add src
                let result = decode_binary_register(&bytes).unwrap();
                assert_eq!(result.opcode, expected_opcode);
            }
        }
    }

    #[test]
    fn test_decode_neg32() {
        let bytes = vec![0x84, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

        let result = decode_unary(&bytes).unwrap();
        assert_eq!(result.opcode, Opcode::Neg32);
        assert_eq!(result.dst.unwrap().n, 1);
    }

    #[test]
    fn test_decode_various_jump_immediates() {
        let jumps = vec![
            (0x15, Opcode::JeqImm),
            (0x25, Opcode::JgtImm),
            (0x35, Opcode::JgeImm),
        ];

        for (byte, expected_opcode) in jumps {
            let bytes = vec![byte, 0x01, 0x0a, 0x00, 0x64, 0x00, 0x00, 0x00];
            let result = decode_jump_immediate(&bytes).unwrap();
            assert_eq!(result.opcode, expected_opcode);
        }
    }

    #[test]
    fn test_decode_various_jump_registers() {
        let jumps = vec![
            (0x1d, Opcode::JeqReg),
            (0x2d, Opcode::JgtReg),
            (0x3d, Opcode::JgeReg),
        ];

        for (byte, expected_opcode) in jumps {
            let bytes = vec![byte, 0x21, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00];
            let result = decode_jump_register(&bytes).unwrap();
            assert_eq!(result.opcode, expected_opcode);
        }
    }

    #[test]
    fn test_decode_static_syscall_unknown_hash() {
        let unknown_hash: u32 = 0x12345678;
        let hash_bytes = unknown_hash.to_le_bytes();

        let bytes = vec![
            0x85,
            0x00, // src=0 (static syscall)
            0x00,
            0x00,
            hash_bytes[0],
            hash_bytes[1],
            hash_bytes[2],
            hash_bytes[3],
        ];

        let result = decode_call_immediate(&bytes).unwrap();
        assert_eq!(result.opcode, Opcode::Call);
        assert!(result.dst.is_none());
        assert_eq!(result.src.unwrap().n, 0);

        // unknown hash should be kept as numeric immediate
        match result.imm {
            Some(Either::Right(Number::Int(imm))) => {
                assert_eq!(imm as u32, unknown_hash);
            }
            _ => panic!("Expected numeric immediate for unknown syscall hash"),
        }
    }

    #[test]
    fn test_decode_call_invalid_src_register() {
        let bytes = vec![0x85, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]; // src=2
        let result = decode_call_immediate(&bytes);
        assert!(result.is_err());
    }
}
