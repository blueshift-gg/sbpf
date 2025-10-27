use crate::errors::SBPFError;
use crate::inst_param::{Number, Register};
// use crate::instruction::{Instruction, Register};
use crate::opcode::Opcode;
use crate::syscalls::SYSCALLS;

// TODO: passing span for error reporting (not sure if it's necessary)

#[inline]
fn parse_bytes(bytes: &[u8]) -> Result<(Opcode, u8, u8, i16, i32), SBPFError> {
    let opcode = Opcode::from_u8(bytes[0]).ok_or(SBPFError::BytecodeError {
        error: format!("Invalid opcode: {:?}", bytes[0]),
        span: 0..bytes.len(),
        custom_label: None,
    })?;
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
        opcode: opcode,
        dst: Some(Register { n: dst }),
        src: None,
        off: None,
        imm: Some(Number::Int(imm.into())),
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
        opcode: opcode,
        dst: Some(Register { n: dst }),
        src: Some(Register { n: src }),
        off: Some(off),
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
        opcode: opcode,
        dst: Some(Register { n: dst }),
        src: None,
        off: Some(off),
        imm: Some(Number::Int(imm.into())),
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
        opcode: opcode,
        dst: Some(Register { n: dst }),
        src: Some(Register { n: src }),
        off: Some(off),
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
        opcode: opcode,
        dst: Some(Register { n: dst }),
        src: None,
        off: None,
        imm: Some(Number::Int(imm.into())),
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
        opcode: opcode,
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
        opcode: opcode,
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
        opcode: opcode,
        dst: None,
        src: None,
        off: Some(off),
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
        opcode: opcode,
        dst: Some(Register { n: dst }),
        src: None,
        off: Some(off),
        imm: Some(Number::Int(imm.into())),
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
        opcode: opcode,
        dst: Some(Register { n: dst }),
        src: Some(Register { n: src }),
        off: Some(off),
        imm: None,
        span: 0..8,
    })
}

pub fn decode_call_immediate(bytes: &[u8]) -> Result<Instruction, SBPFError> {
    assert!(bytes.len() >= 8);
    let (opcode, dst, src, off, imm) = parse_bytes(bytes)?;
    if SYSCALLS.get(&(imm as u32)).is_some() {
        if dst != 0 || src != 0 || off != 0 {
            return Err(SBPFError::BytecodeError {
                error: format!(
                    "{} instruction has dst: {}, src: {}, off: {} supposed to be zeros",
                    opcode, dst, src, off
                ),
                span: 0..8,
                custom_label: None,
            });
        }
    } else {
        if dst != 0 || src != 1 || off != 0 {
            return Err(SBPFError::BytecodeError {
                error: format!(
                    "{} instruction has dst: {}, src: {}, off: {} 
                        supposed to be sixteen and zero",
                    opcode, dst, src, off
                ),
                span: 0..8,
                custom_label: None,
            });
        }
    }
    Ok(Instruction {
        opcode: opcode,
        dst: None,
        src: None,
        off: None,
        imm: Some(Number::Int(imm.into())),
        span: 0..8,
    })
}

pub fn decode_call_register(bytes: &[u8]) -> Result<Instruction, SBPFError> {
    assert!(bytes.len() >= 8);
    let (opcode, dst, src, off, imm) = parse_bytes(bytes)?;
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
        opcode: opcode,
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
        opcode: opcode,
        dst: None,
        src: None,
        off: None,
        imm: None,
        span: 0..8,
    })
}
