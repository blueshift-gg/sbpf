use {
    crate::{
        errors::SBPFError,
        inst_handler::{OPCODE_TO_HANDLER, OPCODE_TO_TYPE},
        inst_param::{Number, Register},
        opcode::{Opcode, OperationType},
    },
    core::ops::Range,
    either::Either,
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Instruction {
    pub opcode: Opcode,
    pub dst: Option<Register>,
    pub src: Option<Register>,
    pub off: Option<Either<String, i16>>,
    pub imm: Option<Either<String, Number>>,
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

    pub fn needs_relocation(&self) -> bool {
        match self.opcode {
            Opcode::Call | Opcode::Lddw => {
                matches!(&self.imm, Some(Either::Left(_identifier)))
            }
            _ => false,
        }
    }

    // only used for be/le
    pub fn op_imm_bits(&self) -> Result<String, SBPFError> {
        match &self.imm {
            Some(Either::Right(Number::Int(imm))) => match *imm {
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

    pub fn from_bytes_sbpf_v2(bytes: &[u8]) -> Result<Self, SBPFError> {
        // Preprocess the opcode byte for SBPF v2 (e_flags == 0x02)
        let mut processed_bytes = bytes.to_vec();

        match processed_bytes[0] {
            // New opcodes in v2 that map to existing instructions
            0x8C => processed_bytes[0] = 0x61, // v2: 0x8C -> ldxw dst, [src + off]
            0x8F => processed_bytes[0] = 0x63, // v2: 0x8F -> stxw [dst + off], src
            // Repurposed opcodes in v2
            0x2C => processed_bytes[0] = 0x71, // v2: mul32 dst, src -> ldxb dst, [src + off]
            0x3C => processed_bytes[0] = 0x69, // v2: div32 dst, src -> ldxh dst, [src + off]
            0x9C => processed_bytes[0] = 0x79, // v2: mod32 dst, src -> ldxdw dst, [src + off]
            0x27 => processed_bytes[0] = 0x72, // v2: mul64 dst, imm -> stb [dst + off], imm
            0x2F => processed_bytes[0] = 0x73, // v2: mul64 dst, src -> stxb [dst + off], src
            0x37 => processed_bytes[0] = 0x6A, // v2: div64 dst, imm -> sth [dst + off], imm
            0x3F => processed_bytes[0] = 0x6B, // v2: div64 dst, src -> stxh [dst + off], src
            0x87 => processed_bytes[0] = 0x62, // v2: neg64 dst -> stw [dst + off], imm
            0x97 => processed_bytes[0] = 0x7A, // v2: mod64 dst, imm -> stdw [dst + off], imm
            0x9F => processed_bytes[0] = 0x7B, // v2: mod64 dst, src -> stxdw [dst + off], src
            // Revert Lddw
            0x21 => {
                if let Some(lddw_2) = processed_bytes.get(8)
                    && lddw_2 == &0xf7
                {
                    processed_bytes[0] = 0x18;
                    processed_bytes[8..12].clone_from_slice(&[0u8; 4]);
                }
            }
            // Move callx target from src to dst
            0x8D => processed_bytes[1] >>= 4,
            // All other opcodes remain unchanged
            _ => (),
        }

        Self::from_bytes(&processed_bytes)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, SBPFError> {
        let dst_val = self.dst.as_ref().map(|r| r.n).unwrap_or(0);
        let src_val = if self.opcode == Opcode::Call {
            1
        } else {
            self.src.as_ref().map(|r| r.n).unwrap_or(0)
        };
        let off_val = match &self.off {
            Some(Either::Left(ident)) => {
                unreachable!("Identifier '{}' should have been resolved earlier", ident)
            }
            Some(Either::Right(off)) => *off,
            None => 0,
        };
        let imm_val = match &self.imm {
            Some(Either::Left(ident)) => {
                if self.opcode == Opcode::Call {
                    -1i64 // FF FF FF FF
                } else {
                    unreachable!("Identifier '{}' should have been resolved earlier", ident)
                }
            }
            Some(Either::Right(Number::Int(imm))) | Some(Either::Right(Number::Addr(imm))) => *imm,
            None => 0,
        };
        // fix callx encoding in sbpf
        let (dst_val, imm_val) = match self.opcode {
            Opcode::Callx => (0, dst_val as i64), // callx: dst register encoded in imm
            _ => (dst_val, imm_val),
        };

        let mut b = vec![self.opcode.into(), src_val << 4 | dst_val];
        b.extend_from_slice(&off_val.to_le_bytes());
        b.extend_from_slice(&(imm_val as i32).to_le_bytes());
        if self.opcode == Opcode::Lddw {
            b.extend_from_slice(&[0; 4]);
            b.extend_from_slice(&((imm_val >> 32) as i32).to_le_bytes());
        }
        Ok(b)
    }

    pub fn to_asm(&self) -> Result<String, SBPFError> {
        if let Some(handler) = OPCODE_TO_HANDLER.get(&self.opcode) {
            match (handler.validate)(self) {
                Ok(()) => {
                    let mut asm = format!("{}", self.opcode);
                    let mut param = vec![];

                    fn off_str(off: &Either<String, i16>) -> String {
                        match off {
                            Either::Left(ident) => ident.clone(),
                            Either::Right(offset) => {
                                if offset.is_negative() {
                                    offset.to_string()
                                } else {
                                    format!("+{}", offset)
                                }
                            }
                        }
                    }

                    fn mem_off(r: &Register, off: &Either<String, i16>) -> String {
                        format!("[r{}{}]", r.n, off_str(off))
                    }

                    if self.get_opcode_type() == OperationType::LoadMemory {
                        param.push(format!("r{}", self.dst.as_ref().unwrap().n));
                        param.push(mem_off(
                            self.src.as_ref().unwrap(),
                            self.off.as_ref().unwrap(),
                        ));
                    } else if self.get_opcode_type() == OperationType::StoreImmediate
                        || self.get_opcode_type() == OperationType::StoreRegister
                    {
                        param.push(mem_off(
                            self.dst.as_ref().unwrap(),
                            self.off.as_ref().unwrap(),
                        ));
                        param.push(format!("r{}", self.src.as_ref().unwrap().n));
                    } else {
                        if let Some(dst) = &self.dst {
                            param.push(format!("r{}", dst.n));
                        }
                        if let Some(src) = &self.src {
                            param.push(format!("r{}", src.n));
                        }
                        if let Some(imm) = &self.imm {
                            if self.opcode == Opcode::Le || self.opcode == Opcode::Be {
                                todo!("handle le/be")
                            } else {
                                param.push(format!("{}", imm));
                            }
                        }
                        if let Some(off) = &self.off {
                            param.push(off_str(off).to_string());
                        }
                    }
                    if !param.is_empty() {
                        asm.push(' ');
                        asm.push_str(&param.join(", "));
                    }
                    Ok(asm)
                }
                Err(e) => Err(e),
            }
        } else {
            Err(SBPFError::BytecodeError {
                error: format!("no validate handler for opcode {}", self.opcode),
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
            inst_param::{Number, Register},
            instruction::Instruction,
            opcode::Opcode,
        },
        either::Either,
        hex_literal::hex,
    };

    #[test]
    fn serialize_e2e() {
        let b = hex!("9700000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm().unwrap(), "mod64 r0, 0");
    }

    #[test]
    fn serialize_e2e_lddw() {
        let b = hex!("18010000000000000000000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm().unwrap(), "lddw r1, 0");
    }

    #[test]
    fn serialize_e2e_add64_imm() {
        let b = hex!("0701000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm().unwrap(), "add64 r1, 0");
    }

    #[test]
    fn serialize_e2e_add64_reg() {
        let b = hex!("0f12000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm().unwrap(), "add64 r2, r1");
    }

    #[test]
    fn serialize_e2e_ja() {
        let b = hex!("05000a0000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm().unwrap(), "ja +10");
    }

    #[test]
    fn serialize_e2e_jeq_imm() {
        let b = hex!("15030a0001000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm().unwrap(), "jeq r3, 1, +10");
    }

    #[test]
    fn serialize_e2e_jeq_reg() {
        let b = hex!("1d210a0000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm().unwrap(), "jeq r1, r2, +10");
    }

    #[test]
    fn serialize_e2e_ldxw() {
        let b = hex!("6112000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm().unwrap(), "ldxw r2, [r1+0]");
    }

    #[test]
    fn serialize_e2e_stxw() {
        let b = hex!("6312000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm().unwrap(), "stxw [r2+0], r1");
    }

    #[test]
    fn serialize_e2e_neg64() {
        let b = hex!("8700000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
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
    fn test_invalid_opcode() {
        let result = Instruction::from_bytes(&hex!("ff00000000000000"));
        assert!(result.is_err());
    }

    #[test]
    fn test_unsupported_opcode() {
        let add32 = Instruction::from_bytes(&hex!("1300000000000000"));
        assert!(add32.is_err());
    }

    #[test]
    fn test_op_imm_bits_16() {
        let inst = Instruction {
            opcode: Opcode::Le,
            dst: Some(Register { n: 1 }),
            src: None,
            off: None,
            imm: Some(Either::Right(Number::Int(16))),
            span: 0..8,
        };
        assert_eq!(inst.op_imm_bits().unwrap(), "le16");
    }

    #[test]
    fn test_op_imm_bits_32() {
        let inst = Instruction {
            opcode: Opcode::Le,
            dst: Some(Register { n: 1 }),
            src: None,
            off: None,
            imm: Some(Either::Right(Number::Int(32))),
            span: 0..8,
        };
        assert_eq!(inst.op_imm_bits().unwrap(), "le32");
    }

    #[test]
    fn test_op_imm_bits_64() {
        let inst = Instruction {
            opcode: Opcode::Be,
            dst: Some(Register { n: 1 }),
            src: None,
            off: None,
            imm: Some(Either::Right(Number::Int(64))),
            span: 0..8,
        };
        assert_eq!(inst.op_imm_bits().unwrap(), "be64");
    }

    #[test]
    fn test_op_imm_bits_invalid() {
        let inst = Instruction {
            opcode: Opcode::Le,
            dst: Some(Register { n: 1 }),
            src: None,
            off: None,
            imm: Some(Either::Right(Number::Int(8))),
            span: 0..8,
        };
        assert!(inst.op_imm_bits().is_err());
    }

    #[test]
    fn test_op_imm_bits_no_imm() {
        let inst = Instruction {
            opcode: Opcode::Le,
            dst: Some(Register { n: 1 }),
            src: None,
            off: None,
            imm: None,
            span: 0..8,
        };
        assert!(inst.op_imm_bits().is_err());
    }

    #[test]
    fn test_to_bytes_callx() {
        // callx r5 - dst register encoded in imm
        let inst = Instruction {
            opcode: Opcode::Callx,
            dst: Some(Register { n: 5 }),
            src: None,
            off: None,
            imm: None,
            span: 0..8,
        };
        let bytes = inst.to_bytes().unwrap();
        assert_eq!(bytes[0], 0x8d);
        assert_eq!(bytes[4], 5);
    }

    #[test]
    fn test_to_bytes_call_with_identifier() {
        let inst = Instruction {
            opcode: Opcode::Call,
            dst: None,
            src: Some(Register { n: 1 }),
            off: None,
            imm: Some(Either::Left("function".to_string())),
            span: 0..8,
        };
        let bytes = inst.to_bytes().unwrap();
        // Should encode -1 for unresolved identifier
        assert_eq!(
            i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            -1
        );
    }

    #[test]
    fn test_to_asm_with_imm_addr() {
        // Test Number::Addr variant in to_bytes
        let inst = Instruction {
            opcode: Opcode::Add64Imm,
            dst: Some(Register { n: 1 }),
            src: None,
            off: None,
            imm: Some(Either::Right(Number::Addr(100))),
            span: 0..8,
        };
        let bytes = inst.to_bytes().unwrap();
        assert_eq!(bytes[0], 0x07); // add64 imm opcode
        assert_eq!(
            i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            100
        );
    }

    #[test]
    fn test_from_bytes_sbpf_v2() {
        // Test all v2 opcode mappings and repurposed opcodes
        let test_cases = vec![
            // New opcodes in v2
            (hex!("8c12000000000000"), Opcode::Ldxw, "v2: 0x8C -> ldxw"),
            (hex!("8f12000000000000"), Opcode::Stxw, "v2: 0x8F -> stxw"),
            // Repurposed opcodes in v2
            (
                hex!("2c12000000000000"),
                Opcode::Ldxb,
                "v2: 0x2C (mul32 reg) -> ldxb",
            ),
            (
                hex!("3c12000000000000"),
                Opcode::Ldxh,
                "v2: 0x3C (div32 reg) -> ldxh",
            ),
            (
                hex!("9c12000000000000"),
                Opcode::Ldxdw,
                "v2: 0x9C (mod32 reg) -> ldxdw",
            ),
            (
                hex!("2701040064000000"),
                Opcode::Stb,
                "v2: 0x27 (mul64 imm) -> stb",
            ),
            (
                hex!("2f12040000000000"),
                Opcode::Stxb,
                "v2: 0x2F (mul64 reg) -> stxb",
            ),
            (
                hex!("3701040064000000"),
                Opcode::Sth,
                "v2: 0x37 (div64 imm) -> sth",
            ),
            (
                hex!("3f12040000000000"),
                Opcode::Stxh,
                "v2: 0x3F (div64 reg) -> stxh",
            ),
            (
                hex!("8701040064000000"),
                Opcode::Stw,
                "v2: 0x87 (neg64) -> stw",
            ),
            (
                hex!("9701040064000000"),
                Opcode::Stdw,
                "v2: 0x97 (mod64 imm) -> stdw",
            ),
            (
                hex!("9f12040000000000"),
                Opcode::Stxdw,
                "v2: 0x9F (mod64 reg) -> stxdw",
            ),
        ];

        for (bytes, expected_opcode, description) in test_cases {
            let inst = Instruction::from_bytes_sbpf_v2(&bytes).unwrap();
            assert_eq!(inst.opcode, expected_opcode, "{}", description);
        }

        // Test callx
        let callx_bytes = hex!("8d50000000000000");
        let callx_inst = Instruction::from_bytes_sbpf_v2(&callx_bytes).unwrap();
        assert_eq!(callx_inst.opcode, Opcode::Callx);
        assert_eq!(callx_inst.dst.unwrap().n, 5);

        // Test lddw
        let mut lddw_bytes = vec![0x21, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        lddw_bytes.extend_from_slice(&[0xf7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        let lddw_inst = Instruction::from_bytes_sbpf_v2(&lddw_bytes).unwrap();
        assert_eq!(lddw_inst.opcode, Opcode::Lddw);
    }
}
