use crate::dynsym::RelocationType;
use crate::lexer::{ImmediateValue, Token};
use crate::syscall::SYSCALLS;
use crate::errors::CompileError;
use sbpf_common::opcode::Opcode;

use std::ops::Range;

#[derive(Debug, Clone)]
pub struct Instruction {
    pub opcode: Opcode,
    pub operands: Vec<Token>,
    pub span: Range<usize>,
}

impl Instruction {
    //
    pub fn get_size(&self) -> u64 {
        match self.opcode {
            Opcode::Lddw => 16,
            _ => 8,
        }
    }
    //
    pub fn needs_relocation(&self) -> bool {
        match self.opcode {
            Opcode::Call | Opcode::Lddw => {
                matches!(&self.operands.last(), Some(Token::Identifier(_, _)))
            }
            _ => false,
        }
    }

    //
    pub fn is_jump(&self) -> bool {
        matches!(
            self.opcode,
            Opcode::Ja
                | Opcode::JeqImm
                | Opcode::JgtImm
                | Opcode::JgeImm
                | Opcode::JltImm
                | Opcode::JleImm
                | Opcode::JsetImm
                | Opcode::JneImm
                | Opcode::JsgtImm
                | Opcode::JsgeImm
                | Opcode::JsltImm
                | Opcode::JsleImm
                | Opcode::JeqReg
                | Opcode::JgtReg
                | Opcode::JgeReg
                | Opcode::JltReg
                | Opcode::JleReg
                | Opcode::JsetReg
                | Opcode::JneReg
                | Opcode::JsgtReg
                | Opcode::JsgeReg
                | Opcode::JsltReg
                | Opcode::JsleReg
        )
    }
    //
    pub fn get_relocation_info(&self) -> (RelocationType, String) {
        match self.opcode {
            Opcode::Lddw => match &self.operands[1] {
                Token::Identifier(name, _) => (RelocationType::RSbf64Relative, name.clone()),
                _ => panic!("Expected label operand"),
            },
            _ => {
                if let Token::Identifier(name, _) = &self.operands[0] {
                    (RelocationType::RSbfSyscall, name.clone())
                } else {
                    panic!("Expected label operand")
                }
            }
        }
    }
    //
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CompileError> {
        let mut operands = Vec::new();
        let span = 0..bytes.len();

        let opcode = Opcode::from_u8(bytes[0]).unwrap();
        let reg = bytes[1];
        let src = reg >> 4;
        let dst = reg & 0x0f;
        let off = i16::from_le_bytes([bytes[2], bytes[3]]);
        let imm = match opcode {
            Opcode::Lddw => {
                let imm_low = i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
                let imm_high = i32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);

                ((imm_high as i64) << 32) | (imm_low as u32 as i64)
            }
            _ => i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as i64,
        };

        match opcode {
            Opcode::Lddw => {
                if src != 0 || off != 0 {
                    return Err(CompileError::BytecodeError {
                        error: format!("Lddw instruction expects src and off to be 0, but got src: {}, off: {}", src, off),
                        span: span.clone(),
                        custom_label: None,
                    });
                }
                operands.push(Token::Register(dst, 1..2));
                operands.push(Token::ImmediateValue(ImmediateValue::Int(imm), 4..12));
            }

            Opcode::Call => {
                if let Some(name) = SYSCALLS.get(&(imm as u32)) {
                    if reg != 0 || off != 0 {
                        return Err(CompileError::BytecodeError {
                            error: format!("Call instruction with syscall expects reg and off to be 0, but got reg: {}, off: {}", reg, off),
                            span: span.clone(),
                            custom_label: None,
                        });
                    }
                    operands.push(Token::Identifier(name.to_string(), 4..8));
                } else {
                    if reg != 16 || off != 0 {
                        return Err(CompileError::BytecodeError {
                            error: format!("Call instruction with immediate expects reg to be 16 and off to be 0, but got reg: {}, off: {}", reg, off),
                            span: span.clone(),
                            custom_label: None,
                        });
                    }
                    operands.push(Token::ImmediateValue(ImmediateValue::Int(imm), 4..8));
                }
            }

            Opcode::Callx => {
                if src != 0 || off != 0 || imm != 0 {
                    return Err(CompileError::BytecodeError {
                        error: format!("Callx instruction expects src, off, and imm to be 0, but got src: {}, off: {}, imm: {}", src, off, imm),
                        span: span.clone(),
                        custom_label: None,
                    });
                }
                // callx destination register is encoded in the dst field
                operands.push(Token::Register(dst, 1..2));
            }

            Opcode::Ja => {
                if reg != 0 || imm != 0 {
                    return Err(CompileError::BytecodeError {
                        error: format!("Ja instruction expects reg and imm to be 0, but got reg: {}, imm: {}", reg, imm),
                        span: span.clone(),
                        custom_label: None,
                    });
                }
                operands.push(Token::ImmediateValue(ImmediateValue::Int(off as i64), 2..4));
            }

            Opcode::JeqImm
            | Opcode::JgtImm
            | Opcode::JgeImm
            | Opcode::JltImm
            | Opcode::JleImm
            | Opcode::JsetImm
            | Opcode::JneImm
            | Opcode::JsgtImm
            | Opcode::JsgeImm
            | Opcode::JsltImm
            | Opcode::JsleImm => {
                if src != 0 {
                    return Err(CompileError::BytecodeError {
                        error: format!("Jump instruction with immediate expects src to be 0, but got src: {}", src),
                        span: span.clone(),
                        custom_label: None,
                    });
                }
                operands.push(Token::Register(dst, 1..2));
                operands.push(Token::ImmediateValue(ImmediateValue::Int(imm), 4..8));
                operands.push(Token::ImmediateValue(ImmediateValue::Int(off as i64), 2..4));
            }

            Opcode::JeqReg
            | Opcode::JgtReg
            | Opcode::JgeReg
            | Opcode::JltReg
            | Opcode::JleReg
            | Opcode::JsetReg
            | Opcode::JneReg
            | Opcode::JsgtReg
            | Opcode::JsgeReg
            | Opcode::JsltReg
            | Opcode::JsleReg => {
                if imm != 0 {
                    return Err(CompileError::BytecodeError {
                        error: format!("Jump instruction with register expects imm to be 0, but got imm: {}", imm),
                        span: span.clone(),
                        custom_label: None,
                    });
                }
                operands.push(Token::Register(dst, 1..2));
                operands.push(Token::Register(src, 1..2));
                operands.push(Token::ImmediateValue(ImmediateValue::Int(off as i64), 2..4));
            }

            // Arithmetic instructions with immediate values
            Opcode::Add32Imm
            | Opcode::Sub32Imm
            | Opcode::Mul32Imm
            | Opcode::Div32Imm
            | Opcode::Or32Imm
            | Opcode::And32Imm
            | Opcode::Lsh32Imm
            | Opcode::Rsh32Imm
            | Opcode::Mod32Imm
            | Opcode::Xor32Imm
            | Opcode::Mov32Imm
            | Opcode::Arsh32Imm
            | Opcode::Lmul32Imm
            | Opcode::Udiv32Imm
            | Opcode::Urem32Imm
            | Opcode::Sdiv32Imm
            | Opcode::Srem32Imm
            | Opcode::Add64Imm
            | Opcode::Sub64Imm
            | Opcode::Mul64Imm
            | Opcode::Div64Imm
            | Opcode::Or64Imm
            | Opcode::And64Imm
            | Opcode::Lsh64Imm
            | Opcode::Rsh64Imm
            | Opcode::Mod64Imm
            | Opcode::Xor64Imm
            | Opcode::Mov64Imm
            | Opcode::Arsh64Imm
            | Opcode::Hor64Imm
            | Opcode::Lmul64Imm
            | Opcode::Uhmul64Imm
            | Opcode::Udiv64Imm
            | Opcode::Urem64Imm
            | Opcode::Shmul64Imm
            | Opcode::Sdiv64Imm
            | Opcode::Srem64Imm
            | Opcode::Be
            | Opcode::Le => {
                if src != 0 || off != 0 {
                    return Err(CompileError::BytecodeError {
                        error: format!("Arithmetic instruction with immediate expects src and off to be 0, but got src: {}, off: {}", src, off),
                        span: span.clone(),
                        custom_label: None,
                    });
                }
                operands.push(Token::Register(dst, 1..2));
                operands.push(Token::ImmediateValue(ImmediateValue::Int(imm), 4..8));
            }

            // Arithmetic instructions with register operands
            Opcode::Add32Reg
            | Opcode::Sub32Reg
            | Opcode::Mul32Reg
            | Opcode::Div32Reg
            | Opcode::Or32Reg
            | Opcode::And32Reg
            | Opcode::Lsh32Reg
            | Opcode::Rsh32Reg
            | Opcode::Mod32Reg
            | Opcode::Xor32Reg
            | Opcode::Mov32Reg
            | Opcode::Arsh32Reg
            | Opcode::Lmul32Reg
            | Opcode::Udiv32Reg
            | Opcode::Urem32Reg
            | Opcode::Sdiv32Reg
            | Opcode::Srem32Reg
            | Opcode::Add64Reg
            | Opcode::Sub64Reg
            | Opcode::Mul64Reg
            | Opcode::Div64Reg
            | Opcode::Or64Reg
            | Opcode::And64Reg
            | Opcode::Lsh64Reg
            | Opcode::Rsh64Reg
            | Opcode::Mod64Reg
            | Opcode::Xor64Reg
            | Opcode::Mov64Reg
            | Opcode::Arsh64Reg
            | Opcode::Lmul64Reg
            | Opcode::Uhmul64Reg
            | Opcode::Udiv64Reg
            | Opcode::Urem64Reg
            | Opcode::Shmul64Reg
            | Opcode::Sdiv64Reg
            | Opcode::Srem64Reg => {
                if off != 0 || imm != 0 {
                    return Err(CompileError::BytecodeError {
                        error: format!("Arithmetic instruction with register expects off and imm to be 0, but got off: {}, imm: {}", off, imm),
                        span: span.clone(),
                        custom_label: None,
                    });
                }
                operands.push(Token::Register(dst, 1..2));
                operands.push(Token::Register(src, 1..2));
            }

            Opcode::Ldxw | Opcode::Ldxh | Opcode::Ldxb | Opcode::Ldxdw => {
                if imm != 0 {
                    return Err(CompileError::BytecodeError {
                        error: format!("Load instruction expects imm to be 0, but got imm: {}", imm),
                        span: span.clone(),
                        custom_label: None,
                    });
                }
                operands.push(Token::Register(dst, 1..2));
                operands.push(Token::Register(src, 1..2));
                operands.push(Token::ImmediateValue(ImmediateValue::Int(off as i64), 2..4));
            }

            Opcode::Stw | Opcode::Sth | Opcode::Stb | Opcode::Stdw => {
                if src != 0 {
                    return Err(CompileError::BytecodeError {
                        error: format!("Store instruction expects src to be 0, but got src: {}", src),
                        span: span.clone(),
                        custom_label: None,
                    });
                }
                operands.push(Token::Register(dst, 1..2));
                operands.push(Token::ImmediateValue(ImmediateValue::Int(off as i64), 2..4));
                operands.push(Token::ImmediateValue(ImmediateValue::Int(imm), 4..8));
            }

            Opcode::Stxb | Opcode::Stxh | Opcode::Stxw | Opcode::Stxdw => {
                if imm != 0 {
                    return Err(CompileError::BytecodeError {
                        error: format!("Store instruction with register expects imm to be 0, but got imm: {}", imm),
                        span: span.clone(),
                        custom_label: None,
                    });
                }
                operands.push(Token::Register(dst, 1..2));
                operands.push(Token::Register(src, 1..2));
                operands.push(Token::ImmediateValue(ImmediateValue::Int(off as i64), 2..4));
            }

            // Unary operations
            Opcode::Neg32 | Opcode::Neg64 | Opcode::Exit => {
                if src != 0 || off != 0 || imm != 0 {
                    return Err(CompileError::BytecodeError {
                        error: format!("Unary operation expects src, off, and imm to be 0, but got src: {}, off: {}, imm: {}", src, off, imm),
                        span: span.clone(),
                        custom_label: None,
                    });
                }
                operands.push(Token::Register(dst, 1..2));
            }

            _ => {
                return Err(CompileError::BytecodeError {
                    error: format!("Unsupported opcode: {:?}", opcode),
                    span: span.clone(),
                    custom_label: None,
                });
            }
        }

        Ok(Instruction {
            opcode,
            operands,
            span,
        })
    }
}
