use {
    crate::{
      errors::SBPFError, 
      inst_param::{
        Number,
        Register,
      },
      opcode::Opcode, 
      syscall::SYSCALLS,
    },
    core::{fmt, ops::Range},
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Register {
    pub n: u8,
}
use crate::errors::SBPFError;

use crate::inst_param::{
    Register,
    Number
};

use crate::opcode::{Opcode, OperationType};
use crate::inst_handler::{OPCODE_TO_HANDLER, OPCODE_TO_TYPE};

use core::ops::Range;
use serde::{Deserialize, Serialize};

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
    
    fn get_opcode_type(&self) -> OperationType {
        *OPCODE_TO_TYPE.get(&self.opcode).unwrap()
    }
    
    pub fn is_jump(&self) -> bool {
        matches!(
            self.get_opcode_type(),
            OperationType::Jump 
                | OperationType::JumpImmediate 
                | OperationType::JumpRegister
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
    // only used for be/le
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
        let opcode = Opcode::from_u8(bytes[0]).unwrap();
        if let Some(handler) = OPCODE_TO_HANDLER.get(&opcode) {
            (handler.decode)(bytes)
        } else {
            Err(SBPFError::BytecodeError {
                error: format!("no decode handler for opcode {}", opcode),
                span: 0..1,
                custom_label: Some("Invalid opcode".to_string()),
            })
        }
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
    use {
        crate::{
            instruction::{Instruction, Register},
            opcode::Opcode,
        },
        hex_literal::hex,
    };

    #[test]
    fn serialize_e2e() {
        let b = hex!("9700000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes(), &b);
        assert_eq!(i.to_asm().unwrap(), "mod64 r0, 0");
    }

    #[test]
    fn serialize_e2e_lddw() {
        let b = hex!("18010000000000000000000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes(), &b);
        assert_eq!(i.to_asm().unwrap(), "lddw r1, 0");
    }

    #[test]
    fn serialize_e2e_add64_imm() {
        let b = hex!("0701000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes(), &b);
        assert_eq!(i.to_asm().unwrap(), "add64 r1, 0");
    }

    #[test]
    fn serialize_e2e_add64_reg() {
        let b = hex!("0f12000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes(), &b);
        assert_eq!(i.to_asm().unwrap(), "add64 r2, r1");
    }

    #[test]
    fn serialize_e2e_ja() {
        let b = hex!("05000a0000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes(), &b);
        assert_eq!(i.to_asm().unwrap(), "jeq +10");
    }

    #[test]
    fn serialize_e2e_jeq_imm() {
        let b = hex!("15030a0001000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes(), &b);
        assert_eq!(i.to_asm().unwrap(), "jeq r3, 1, +10");
    }

    #[test]
    fn serialize_e2e_jeq_reg() {
        let b = hex!("1d210a0000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes(), &b);
        assert_eq!(i.to_asm().unwrap(), "jeq r1, r2, +10");
    }

    #[test]
    fn serialize_e2e_ldxw() {
        let b = hex!("6112000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes(), &b);
        assert_eq!(i.to_asm().unwrap(), "ldxw r2, [r1+0]");
    }

    #[test]
    fn serialize_e2e_stxw() {
        let b = hex!("6312000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes(), &b);
        assert_eq!(i.to_asm().unwrap(), "stxw [r2+0], r1");
    }

    #[test]
    fn serialize_e2e_neg64() {
        let b = hex!("8700000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes(), &b);
        assert_eq!(i.to_asm().unwrap(), "neg64 r0");
    }

    #[test]
    fn test_instruction_size() {
        let exit = Instruction::from_bytes(&hex!("9500000000000000")).unwrap();
        assert_eq!(exit.get_size(), 8);

        let lddw = Instruction::from_bytes(&hex!("18010000000000000000000000000000")).unwrap();
        assert_eq!(lddw.get_size(), 16);
    }

    #[test]
    fn test_is_jump() {
        let ja = Instruction::from_bytes(&hex!("0500000000000000")).unwrap();
        assert!(ja.is_jump());

        let jeq_imm = Instruction::from_bytes(&hex!("1502000000000000")).unwrap();
        assert!(jeq_imm.is_jump());

        let jeq_reg = Instruction::from_bytes(&hex!("1d12000000000000")).unwrap();
        assert!(jeq_reg.is_jump());

        let exit = Instruction::from_bytes(&hex!("9500000000000000")).unwrap();
        assert!(!exit.is_jump());

        let add64 = Instruction::from_bytes(&hex!("0701000000000000")).unwrap();
        assert!(!add64.is_jump());
    }

    #[test]
    fn test_off_str() {
        let pos_off = Instruction {
            opcode: Opcode::Ja,
            dst: None,
            src: None,
            off: Some(10),
            imm: None,
            span: 0..8,
        };
        assert_eq!(pos_off.off_str(), "+10");

        let neg_off = Instruction {
            opcode: Opcode::Ja,
            dst: None,
            src: None,
            off: Some(-10),
            imm: None,
            span: 0..8,
        };
        assert_eq!(neg_off.off_str(), "-10");
    }

    #[test]
    fn test_dst_off() {
        let inst = Instruction {
            opcode: Opcode::Ldxw,
            dst: Some(Register { n: 1 }),
            src: Some(Register { n: 2 }),
            off: Some(10),
            imm: None,
            span: 0..8,
        };
        assert_eq!(inst.dst_off(), "[r1+10]");
    }

    #[test]
    fn test_src_off() {
        let inst = Instruction {
            opcode: Opcode::Ldxw,
            dst: Some(Register { n: 1 }),
            src: Some(Register { n: 2 }),
            off: Some(-5),
            imm: None,
            span: 0..8,
        };
        assert_eq!(inst.src_off(), "[r2-5]");
    }

    #[test]
    fn test_invalid_opcode() {
        let result = Instruction::from_bytes(&hex!("ff00000000000000"));
        assert!(result.is_err());
    }

    #[test]
    fn test_unsupported_opcode() {
        let add32 = Instruction::from_bytes(&hex!("1300000000000000"));
        assert!(add32.is_err());
    }
}
