use crate::errors::SBPFError;
use crate::opcode::Opcode;
use crate::syscall::SYSCALLS;

use core::fmt;
use core::ops::Range;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Register {
    pub n: u8,
}

impl fmt::Display for Register {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "r{}", self.n)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Number {
    Int(i64),
    Addr(i64),
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Number::Int(i) => write!(f, "{}", i),
            Number::Addr(a) => write!(f, "{}", a),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Instruction {
    pub opcode: Opcode,
    pub dst: Option<Register>,
    pub src: Option<Register>,
    pub off: Option<i16>,
    pub imm: Option<Number>,
    pub span: Range<usize>,
}

impl Instruction {
    pub fn get_size(&self) -> u64 {
        match self.opcode {
            Opcode::Lddw => 16,
            _ => 8,
        }
    }

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

    pub fn off_str(&self) -> String {
        match self.off {
            Some(off) => {
                if off.is_negative() {
                    off.to_string()
                } else {
                    format!("+{}", off)
                }
            }
            None => "0".to_string(),
        }
    }

    pub fn dst_off(&self) -> String {
        match &self.dst {
            Some(dst) => format!("[r{}{}]", dst.n, self.off_str()),
            None => format!("[r0{}]", self.off_str()),
        }
    }

    pub fn src_off(&self) -> String {
        match &self.src {
            Some(src) => format!("[r{}{}]", src.n, self.off_str()),
            None => format!("[r0{}]", self.off_str()),
        }
    }

    pub fn op_imm_bits(&self) -> Result<String, SBPFError> {
        match &self.imm {
            Some(Number::Int(imm)) => match *imm {
                16 => Ok(format!("{}16", self.opcode)),
                32 => Ok(format!("{}32", self.opcode)),
                64 => Ok(format!("{}64", self.opcode)),
                _ => Err(SBPFError::BytecodeError {
                    error: format!(
                        "Invalid immediate value: {:?} for opcode: {:?}",
                        self.imm, self.opcode
                    ),
                    span: self.span.clone(),
                    custom_label: None,
                }),
            },
            _ => Err(SBPFError::BytecodeError {
                error: format!("Expected immediate value for opcode: {:?}", self.opcode),
                span: self.span.clone(),
                custom_label: None,
            }),
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SBPFError> {
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

        let mut out_dst: Option<Register> = None;
        let mut out_src: Option<Register> = None;
        let mut out_off: Option<i16> = None;
        let mut out_imm: Option<Number> = None;

        match opcode {
            Opcode::Lddw => {
                if src != 0 || off != 0 {
                    return Err(SBPFError::BytecodeError {
                        error: format!(
                            "Lddw instruction expects src and off to be 0, but got src: {}, off: {}",
                            src, off
                        ),
                        span: span.clone(),
                        custom_label: None,
                    });
                }
                out_dst = Some(Register { n: dst });
                out_imm = Some(Number::Int(imm));
            }

            Opcode::Call => {
                if SYSCALLS.get(&(imm as u32)).is_some() {
                    if reg != 0 || off != 0 {
                        return Err(SBPFError::BytecodeError {
                            error: format!(
                                "Call instruction with syscall expects reg and off to be 0, but got reg: {}, off: {}",
                                reg, off
                            ),
                            span: span.clone(),
                            custom_label: None,
                        });
                    }
                    out_imm = Some(Number::Int(imm));
                } else {
                    if reg != 16 || off != 0 {
                        return Err(SBPFError::BytecodeError {
                            error: format!(
                                "Call instruction with immediate expects reg to be 16 and off to be 0, but got reg: {}, off: {}",
                                reg, off
                            ),
                            span: span.clone(),
                            custom_label: None,
                        });
                    }
                    out_imm = Some(Number::Int(imm));
                }
            }

            Opcode::Callx => {
                if src != 0 || off != 0 || imm != 0 {
                    return Err(SBPFError::BytecodeError {
                        error: format!(
                            "Callx instruction expects src, off, and imm to be 0, but got src: {}, off: {}, imm: {}",
                            src, off, imm
                        ),
                        span: span.clone(),
                        custom_label: None,
                    });
                }
                // callx destination register is encoded in the dst field
                out_dst = Some(Register { n: dst });
            }

            Opcode::Ja => {
                if reg != 0 || imm != 0 {
                    return Err(SBPFError::BytecodeError {
                        error: format!(
                            "Ja instruction expects reg and imm to be 0, but got reg: {}, imm: {}",
                            reg, imm
                        ),
                        span: span.clone(),
                        custom_label: None,
                    });
                }
                out_off = Some(off);
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
                    return Err(SBPFError::BytecodeError {
                        error: format!(
                            "Jump instruction with immediate expects src to be 0, but got src: {}",
                            src
                        ),
                        span: span.clone(),
                        custom_label: None,
                    });
                }
                out_dst = Some(Register { n: dst });
                out_imm = Some(Number::Int(imm));
                out_off = Some(off);
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
                    return Err(SBPFError::BytecodeError {
                        error: format!(
                            "Jump instruction with register expects imm to be 0, but got imm: {}",
                            imm
                        ),
                        span: span.clone(),
                        custom_label: None,
                    });
                }
                out_dst = Some(Register { n: dst });
                out_src = Some(Register { n: src });
                out_off = Some(off);
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
                    return Err(SBPFError::BytecodeError {
                        error: format!(
                            "Arithmetic instruction with immediate expects src and off to be 0, but got src: {}, off: {}",
                            src, off
                        ),
                        span: span.clone(),
                        custom_label: None,
                    });
                }
                out_dst = Some(Register { n: dst });
                out_imm = Some(Number::Int(imm));
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
                    return Err(SBPFError::BytecodeError {
                        error: format!(
                            "Arithmetic instruction with register expects off and imm to be 0, but got off: {}, imm: {}",
                            off, imm
                        ),
                        span: span.clone(),
                        custom_label: None,
                    });
                }
                out_dst = Some(Register { n: dst });
                out_src = Some(Register { n: src });
            }

            Opcode::Ldxw | Opcode::Ldxh | Opcode::Ldxb | Opcode::Ldxdw => {
                if imm != 0 {
                    return Err(SBPFError::BytecodeError {
                        error: format!(
                            "Load instruction expects imm to be 0, but got imm: {}",
                            imm
                        ),
                        span: span.clone(),
                        custom_label: None,
                    });
                }
                out_dst = Some(Register { n: dst });
                out_src = Some(Register { n: src });
                out_off = Some(off);
            }

            Opcode::Stw | Opcode::Sth | Opcode::Stb | Opcode::Stdw => {
                if src != 0 {
                    return Err(SBPFError::BytecodeError {
                        error: format!(
                            "Store instruction expects src to be 0, but got src: {}",
                            src
                        ),
                        span: span.clone(),
                        custom_label: None,
                    });
                }
                out_dst = Some(Register { n: dst });
                out_off = Some(off);
                out_imm = Some(Number::Int(imm));
            }

            Opcode::Stxb | Opcode::Stxh | Opcode::Stxw | Opcode::Stxdw => {
                if imm != 0 {
                    return Err(SBPFError::BytecodeError {
                        error: format!(
                            "Store instruction with register expects imm to be 0, but got imm: {}",
                            imm
                        ),
                        span: span.clone(),
                        custom_label: None,
                    });
                }
                out_dst = Some(Register { n: dst });
                out_src = Some(Register { n: src });
                out_off = Some(off);
            }

            // Unary operations
            Opcode::Neg32 | Opcode::Neg64 | Opcode::Exit => {
                if src != 0 || off != 0 || imm != 0 {
                    return Err(SBPFError::BytecodeError {
                        error: format!(
                            "Unary operation expects src, off, and imm to be 0, but got src: {}, off: {}, imm: {}",
                            src, off, imm
                        ),
                        span: span.clone(),
                        custom_label: None,
                    });
                }
                out_dst = Some(Register { n: dst });
            }

            _ => {
                return Err(SBPFError::BytecodeError {
                    error: format!("Unsupported opcode: {:?}", opcode),
                    span: span.clone(),
                    custom_label: None,
                });
            }
        }

        Ok(Instruction {
            opcode,
            dst: out_dst,
            src: out_src,
            off: out_off,
            imm: out_imm,
            span,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let src_val = self.src.as_ref().map(|r| r.n).unwrap_or(0);
        let dst_val = self.dst.as_ref().map(|r| r.n).unwrap_or(0);
        let off_val = self.off.unwrap_or(0);
        let imm_val = match &self.imm {
            Some(Number::Int(imm)) | Some(Number::Addr(imm)) => *imm,
            None => 0,
        };

        let mut b = vec![self.opcode.to_bytecode(), src_val << 4 | dst_val];
        b.extend_from_slice(&off_val.to_le_bytes());
        b.extend_from_slice(&(imm_val as i32).to_le_bytes());
        if self.opcode == Opcode::Lddw {
            b.extend_from_slice(&[0; 4]);
            b.extend_from_slice(&((imm_val >> 32) as i32).to_le_bytes());
        }
        b
    }

    pub fn to_asm(&self) -> Result<String, SBPFError> {
        Ok(match self.opcode {
            // lddw - (load double word) takes up two instructions. The 64 bit value
            // is made up of two halves with the upper half being the immediate
            // of the lddw value and the lower half being the immediate of the
            // following instruction
            Opcode::Lddw => {
                match (&self.dst, &self.imm) {
                    (Some(dst), Some(imm)) => format!("{} r{}, {}", self.opcode, dst.n, imm),
                    _ => return Err(SBPFError::BytecodeError {
                        error: "Lddw instruction missing destination register or immediate value".to_string(),
                        span: self.span.clone(),
                        custom_label: None,
                    }),
                }
            },
            // ldx - (load x) store a 8/16/32/64 bit (byte/half/word/double word)
            // value in a register
            Opcode::Ldxb |
            Opcode::Ldxh |
            Opcode::Ldxw |
            Opcode::Ldxdw => {
                match &self.dst {
                    Some(dst) => format!("{} r{}, {}", self.opcode, dst.n, self.src_off()),
                    None => return Err(SBPFError::BytecodeError {
                        error: format!("{} instruction missing destination register", self.opcode),
                        span: self.span.clone(),
                        custom_label: None,
                    }),
                }
            },
            // stb - these instructions are deprecated
            Opcode::Stb |
            Opcode::Sth |
            Opcode::Stw |
            Opcode::Stdw => {
                match &self.imm {
                    Some(imm) => format!("{} {}, {}", self.opcode, self.dst_off(), imm),
                    None => return Err(SBPFError::BytecodeError {
                        error: format!("{} instruction missing immediate value", self.opcode),
                        span: self.span.clone(),
                        custom_label: None,
                    }),
                }
            },
            // stx - store a 8/16/32/64 bit value from a source register into the offset
            // of the destination register
            Opcode::Stxb |
            Opcode::Stxh |
            Opcode::Stxw |
            Opcode::Stxdw => {
                match &self.src {
                    Some(src) => format!("{} {}, r{}", self.opcode, self.dst_off(), src.n),
                    None => return Err(SBPFError::BytecodeError {
                        error: format!("{} instruction missing source register", self.opcode),
                        span: self.span.clone(),
                        custom_label: None,
                    }),
                }
            },
            // Math
            Opcode::Neg32 | // Deprecated in SBFv2
            Opcode::Neg64 => {
                match &self.dst {
                    Some(dst) => format!("{} r{}", self.opcode, dst.n),
                    None => return Err(SBPFError::BytecodeError {
                        error: format!("{} instruction missing destination register", self.opcode),
                        span: self.span.clone(),
                        custom_label: None,
                    }),
                }
            },
            // LE and BE OpCodes act a little differently to others. In assembly form, they are
            // notated as be16, be32 and b64. In byte form, the bit length of the operation is 
            // determined by the immedate value of its parent instruction, 0x10, 0x20 and 0x40
            // accordingly (the hex of 16/32/64)
            Opcode::Le |
            Opcode::Be => {
                match &self.dst {
                    Some(dst) => format!("{}{}", self.op_imm_bits()?, dst.n),
                    None => return Err(SBPFError::BytecodeError {
                        error: format!("{} instruction missing destination register", self.opcode),
                        span: self.span.clone(),
                        custom_label: None,
                    }),
                }
            }, // Docs for this seem wrong //DC01000010000000 DC01000020000000 DC01000040000000
            // Immedate
            Opcode::Add32Imm |
            Opcode::Sub32Imm |
            Opcode::Mul32Imm |
            Opcode::Div32Imm |
            Opcode::Or32Imm |
            Opcode::And32Imm |
            Opcode::Lsh32Imm |
            Opcode::Rsh32Imm |
            Opcode::Mod32Imm |
            Opcode::Xor32Imm |
            Opcode::Arsh32Imm |
            Opcode::Mov32Imm |
            Opcode::Lmul32Imm |
            Opcode::Udiv32Imm |
            Opcode::Urem32Imm |
            Opcode::Sdiv32Imm |
            Opcode::Srem32Imm |
            Opcode::Add64Imm |
            Opcode::Sub64Imm |
            Opcode::Mul64Imm |
            Opcode::Div64Imm |
            Opcode::Or64Imm |
            Opcode::And64Imm |
            Opcode::Lsh64Imm |
            Opcode::Rsh64Imm |
            Opcode::Mod64Imm |
            Opcode::Xor64Imm |
            Opcode::Mov64Imm |
            Opcode::Arsh64Imm |
            Opcode::Hor64Imm |
            Opcode::Lmul64Imm |
            Opcode::Uhmul64Imm |
            Opcode::Udiv64Imm |
            Opcode::Urem64Imm |
            Opcode::Shmul64Imm |
            Opcode::Sdiv64Imm |
            Opcode::Srem64Imm => {
                match (&self.dst, &self.imm) {
                    (Some(dst), Some(imm)) => format!("{} r{}, {}", self.opcode, dst.n, imm),
                    _ => return Err(SBPFError::BytecodeError {
                        error: format!("{} instruction missing destination register or immediate value", self.opcode),
                        span: self.span.clone(),
                        custom_label: None,
                    }),
                }
            },
            // Register
            Opcode::Add32Reg |
            Opcode::Sub32Reg |
            Opcode::Mul32Reg |
            Opcode::Div32Reg |
            Opcode::Or32Reg |
            Opcode::And32Reg |
            Opcode::Lsh32Reg |
            Opcode::Rsh32Reg |
            Opcode::Mod32Reg |
            Opcode::Xor32Reg |
            Opcode::Mov32Reg |
            Opcode::Arsh32Reg |
            Opcode::Lmul32Reg |
            Opcode::Udiv32Reg |
            Opcode::Urem32Reg |
            Opcode::Sdiv32Reg |
            Opcode::Srem32Reg |
            Opcode::Add64Reg |
            Opcode::Sub64Reg |
            Opcode::Mul64Reg |
            Opcode::Div64Reg |
            Opcode::Or64Reg |
            Opcode::And64Reg |
            Opcode::Lsh64Reg |
            Opcode::Rsh64Reg |
            Opcode::Mod64Reg |
            Opcode::Xor64Reg |
            Opcode::Mov64Reg |
            Opcode::Arsh64Reg |
            Opcode::Lmul64Reg |
            Opcode::Uhmul64Reg |
            Opcode::Udiv64Reg |
            Opcode::Urem64Reg |
            Opcode::Shmul64Reg |
            Opcode::Sdiv64Reg |
            Opcode::Srem64Reg => {
                match (&self.dst, &self.src) {
                    (Some(dst), Some(src)) => format!("{} r{}, r{}", self.opcode, dst.n, src.n),
                    _ => return Err(SBPFError::BytecodeError {
                        error: format!("{} instruction missing destination or source register", self.opcode),
                        span: self.span.clone(),
                        custom_label: None,
                    }),
                }
            },

            // Jumps
            Opcode::Ja => format!("{} {}", self.opcode, self.off_str()),

            // Immediates
            Opcode::JeqImm |
            Opcode::JgtImm |
            Opcode::JgeImm |
            Opcode::JltImm |
            Opcode::JleImm |
            Opcode::JsetImm |
            Opcode::JneImm |
            Opcode::JsgtImm |
            Opcode::JsgeImm |
            Opcode::JsltImm |
            Opcode::JsleImm => {
                match (&self.dst, &self.imm) {
                    (Some(dst), Some(imm)) => format!("{} r{}, {}, {}", self.opcode, dst.n, imm, self.off_str()),
                    _ => return Err(SBPFError::BytecodeError {
                        error: format!("{} instruction missing destination register or immediate value", self.opcode),
                        span: self.span.clone(),
                        custom_label: None,
                    }),
                }
            },
            // Registers
            Opcode::JeqReg |
            Opcode::JgtReg |
            Opcode::JgeReg |
            Opcode::JltReg |
            Opcode::JleReg |
            Opcode::JsetReg |
            Opcode::JneReg |
            Opcode::JsgtReg |
            Opcode::JsgeReg |
            Opcode::JsltReg |
            Opcode::JsleReg => {
                match (&self.dst, &self.src) {
                    (Some(dst), Some(src)) => format!("{} r{}, r{}, {}", self.opcode, dst.n, src.n, self.off_str()),
                    _ => return Err(SBPFError::BytecodeError {
                        error: format!("{} instruction missing destination or source register", self.opcode),
                        span: self.span.clone(),
                        custom_label: None,
                    }),
                }
            },


            // Calls
            Opcode::Call => {
                match &self.imm {
                    Some(imm) => format!("call {}", imm),
                    None => return Err(SBPFError::BytecodeError {
                        error: "Call instruction missing immediate value".to_string(),
                        span: self.span.clone(),
                        custom_label: None,
                    }),
                }
            },
            Opcode::Callx => {
                match &self.src {
                    Some(src) => format!("call r{}", src.n),
                    None => return Err(SBPFError::BytecodeError {
                        error: "Callx instruction missing source register".to_string(),
                        span: self.span.clone(),
                        custom_label: None,
                    }),
                }
            },
            Opcode::Exit => format!("{}", self.opcode),

            _ => return Err(SBPFError::BytecodeError {
                error: format!("Unsupported opcode: {:?}", self.opcode),
                span: self.span.clone(),
                custom_label: None,
            })
        })
    }
}

#[cfg(test)]
mod test {
    use hex_literal::hex;

    use crate::instruction::Instruction;

    #[test]
    fn serialize_e2e() {
        let b = hex!("9700000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes(), &b);
    }

    #[test]
    fn serialize_e2e_lddw() {
        let b = hex!("18010000000000000000000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes(), &b);
    }
}
