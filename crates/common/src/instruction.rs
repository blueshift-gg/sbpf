use {
    crate::{
        errors::SBPFError,
        inst_handler::{OPCODE_TO_HANDLER, OPCODE_TO_TYPE},
        inst_param::{Number, Register},
        opcode::{Opcode, OperationType},
    },
    core::ops::Range,
    serde::{Deserialize, Serialize},
};

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
            OperationType::Jump | OperationType::JumpImmediate | OperationType::JumpRegister
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
        let opcode: Opcode = bytes[0].try_into()?;
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

        let mut b = vec![self.opcode.into(), src_val << 4 | dst_val];
        b.extend_from_slice(&off_val.to_le_bytes());
        b.extend_from_slice(&(imm_val as i32).to_le_bytes());
        if self.opcode == Opcode::Lddw {
            b.extend_from_slice(&[0; 4]);
            b.extend_from_slice(&((imm_val >> 32) as i32).to_le_bytes());
        }
        b
    }

    pub fn to_asm(&self) -> Result<String, SBPFError> {
        if let Some(handler) = OPCODE_TO_HANDLER.get(&self.opcode) {
            (handler.encode)(self)
        } else {
            Err(SBPFError::BytecodeError {
                error: format!("no encode handler for opcode {}", self.opcode),
                span: self.span.clone(),
                custom_label: None,
            })
        }
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
        assert_eq!(i.to_asm().unwrap(), "ja +10");
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
