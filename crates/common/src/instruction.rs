use {
    crate::{
        errors::SBPFError,
        inst_handler::{OPCODE_TO_HANDLER, OPCODE_TO_TYPE},
        inst_param::{Number, Register},
        opcode::{Opcode, OperationType},
        platform::BpfPlatform,
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

    pub fn from_bytes<Platform: BpfPlatform>(bytes: &[u8]) -> Result<Self, SBPFError> {
        // Use platform-specific transformer to decode instruction
        let (opcode, dst, src, off, imm) = Platform::decode_instruction(bytes)?;

        if let Some(handler) = OPCODE_TO_HANDLER.get(&opcode) {
            // Reconstruct bytes with transformed values for the handler
            let mut transformed_bytes = vec![opcode.into(), src << 4 | dst];
            transformed_bytes.extend_from_slice(&off.to_le_bytes());
            transformed_bytes.extend_from_slice(&imm.to_le_bytes());

            // For lddw, we need the full 16 bytes (include second half from original)
            if opcode == Opcode::Lddw && bytes.len() >= 16 {
                transformed_bytes.extend_from_slice(&bytes[8..16]);
            }

            // Call the opcode-specific decoder with transformed bytes
            (handler.decode)(&transformed_bytes)
        } else {
            Err(SBPFError::BytecodeError {
                error: format!("no decode handler for opcode {}", opcode),
                span: 0..1,
                custom_label: Some("Invalid opcode".to_string()),
            })
        }
    }

    pub fn to_bytes<Platform: BpfPlatform>(&self) -> Result<Vec<u8>, SBPFError> {
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
        let imm_val_i64 = match &self.imm {
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

        // Apply platform-specific encoding transformation
        let (raw_opcode, dst_val, src_val, off_val, imm_val) =
            Platform::encode_instruction(self.opcode, dst_val, src_val, off_val, imm_val_i64 as i32);

        let mut b = vec![raw_opcode, src_val << 4 | dst_val];
        b.extend_from_slice(&off_val.to_le_bytes());
        b.extend_from_slice(&imm_val.to_le_bytes());
        if self.opcode == Opcode::Lddw {
            b.extend_from_slice(&[0; 4]);
            b.extend_from_slice(&((imm_val_i64 >> 32) as i32).to_le_bytes());
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
    use {crate::{instruction::Instruction, platform::SbpfV0}, hex_literal::hex};

    #[test]
    fn serialize_e2e() {
        let b = hex!("9700000000000000");
        let i = Instruction::from_bytes::<SbpfV0>(&b).unwrap();
        assert_eq!(i.to_bytes::<SbpfV0>().unwrap(), &b);
        assert_eq!(i.to_asm().unwrap(), "mod64 r0, 0");
    }

    #[test]
    fn serialize_e2e_lddw() {
        let b = hex!("18010000000000000000000000000000");
        let i = Instruction::from_bytes::<SbpfV0>(&b).unwrap();
        assert_eq!(i.to_bytes::<SbpfV0>().unwrap(), &b);
        assert_eq!(i.to_asm().unwrap(), "lddw r1, 0");
    }

    #[test]
    fn serialize_e2e_add64_imm() {
        let b = hex!("0701000000000000");
        let i = Instruction::from_bytes::<SbpfV0>(&b).unwrap();
        assert_eq!(i.to_bytes::<SbpfV0>().unwrap(), &b);
        assert_eq!(i.to_asm().unwrap(), "add64 r1, 0");
    }

    #[test]
    fn serialize_e2e_add64_reg() {
        let b = hex!("0f12000000000000");
        let i = Instruction::from_bytes::<SbpfV0>(&b).unwrap();
        assert_eq!(i.to_bytes::<SbpfV0>().unwrap(), &b);
        assert_eq!(i.to_asm().unwrap(), "add64 r2, r1");
    }

    #[test]
    fn serialize_e2e_ja() {
        let b = hex!("05000a0000000000");
        let i = Instruction::from_bytes::<SbpfV0>(&b).unwrap();
        assert_eq!(i.to_bytes::<SbpfV0>().unwrap(), &b);
        assert_eq!(i.to_asm().unwrap(), "ja +10");
    }

    #[test]
    fn serialize_e2e_jeq_imm() {
        let b = hex!("15030a0001000000");
        let i = Instruction::from_bytes::<SbpfV0>(&b).unwrap();
        assert_eq!(i.to_bytes::<SbpfV0>().unwrap(), &b);
        assert_eq!(i.to_asm().unwrap(), "jeq r3, 1, +10");
    }

    #[test]
    fn serialize_e2e_jeq_reg() {
        let b = hex!("1d210a0000000000");
        let i = Instruction::from_bytes::<SbpfV0>(&b).unwrap();
        assert_eq!(i.to_bytes::<SbpfV0>().unwrap(), &b);
        assert_eq!(i.to_asm().unwrap(), "jeq r1, r2, +10");
    }

    #[test]
    fn serialize_e2e_ldxw() {
        let b = hex!("6112000000000000");
        let i = Instruction::from_bytes::<SbpfV0>(&b).unwrap();
        assert_eq!(i.to_bytes::<SbpfV0>().unwrap(), &b);
        assert_eq!(i.to_asm().unwrap(), "ldxw r2, [r1+0]");
    }

    #[test]
    fn serialize_e2e_stxw() {
        let b = hex!("6312000000000000");
        let i = Instruction::from_bytes::<SbpfV0>(&b).unwrap();
        assert_eq!(i.to_bytes::<SbpfV0>().unwrap(), &b);
        assert_eq!(i.to_asm().unwrap(), "stxw [r2+0], r1");
    }

    #[test]
    fn serialize_e2e_neg64() {
        let b = hex!("8700000000000000");
        let i = Instruction::from_bytes::<SbpfV0>(&b).unwrap();
        assert_eq!(i.to_bytes::<SbpfV0>().unwrap(), &b);
        assert_eq!(i.to_asm().unwrap(), "neg64 r0");
    }

    #[test]
    fn test_instruction_size() {
        let exit = Instruction::from_bytes::<SbpfV0>(&hex!("9500000000000000")).unwrap();
        assert_eq!(exit.get_size(), 8);

        let lddw = Instruction::from_bytes::<SbpfV0>(&hex!("18010000000000000000000000000000")).unwrap();
        assert_eq!(lddw.get_size(), 16);
    }

    #[test]
    fn test_is_jump() {
        let ja = Instruction::from_bytes::<SbpfV0>(&hex!("0500000000000000")).unwrap();
        assert!(ja.is_jump());

        let jeq_imm = Instruction::from_bytes::<SbpfV0>(&hex!("1502000000000000")).unwrap();
        assert!(jeq_imm.is_jump());

        let jeq_reg = Instruction::from_bytes::<SbpfV0>(&hex!("1d12000000000000")).unwrap();
        assert!(jeq_reg.is_jump());

        let exit = Instruction::from_bytes::<SbpfV0>(&hex!("9500000000000000")).unwrap();
        assert!(!exit.is_jump());

        let add64 = Instruction::from_bytes::<SbpfV0>(&hex!("0701000000000000")).unwrap();
        assert!(!add64.is_jump());
    }

    #[test]
    fn test_invalid_opcode() {
        let result = Instruction::from_bytes::<SbpfV0>(&hex!("ff00000000000000"));
        assert!(result.is_err());
    }

    #[test]
    fn test_unsupported_opcode() {
        let add32 = Instruction::from_bytes::<SbpfV0>(&hex!("1300000000000000"));
        assert!(add32.is_err());
    }

    #[test]
    fn test_sbpfv2_new_opcode_mappings() {
        use crate::platform::SbpfV2;

        // 0x8C -> ldxw
        let b = hex!("8c12000000000000");
        let inst = Instruction::from_bytes::<SbpfV2>(&b).unwrap();
        assert_eq!(inst.opcode, crate::opcode::Opcode::Ldxw);
        assert_eq!(inst.to_asm().unwrap(), "ldxw r2, [r1+0]");

        // 0x8F -> stxw
        let b = hex!("8f12000000000000");
        let inst = Instruction::from_bytes::<SbpfV2>(&b).unwrap();
        assert_eq!(inst.opcode, crate::opcode::Opcode::Stxw);
        assert_eq!(inst.to_asm().unwrap(), "stxw [r2+0], r1");

        // 0xF7 -> hor64
        let b = hex!("f701000005000000");
        let inst = Instruction::from_bytes::<SbpfV2>(&b).unwrap();
        assert_eq!(inst.opcode, crate::opcode::Opcode::Hor64Imm);
        assert_eq!(inst.to_asm().unwrap(), "hor64 r1, 5");
    }

    #[test]
    fn test_sbpfv2_opcode_translations() {
        use crate::platform::SbpfV2;

        // mul32 reg (0x2c) -> ldxb
        let b = hex!("2c12050000000000");
        let inst = Instruction::from_bytes::<SbpfV2>(&b).unwrap();
        assert_eq!(inst.opcode, crate::opcode::Opcode::Ldxb);
        assert_eq!(inst.dst.as_ref().unwrap().n, 2);
        assert_eq!(inst.src.as_ref().unwrap().n, 1);

        // div32 reg (0x3c) -> ldxh
        let b = hex!("3c12080000000000");
        let inst = Instruction::from_bytes::<SbpfV2>(&b).unwrap();
        assert_eq!(inst.opcode, crate::opcode::Opcode::Ldxh);
        assert_eq!(inst.dst.as_ref().unwrap().n, 2);
        assert_eq!(inst.src.as_ref().unwrap().n, 1);

        // mod32 reg (0x9c) -> ldxdw
        let b = hex!("9c12100000000000");
        let inst = Instruction::from_bytes::<SbpfV2>(&b).unwrap();
        assert_eq!(inst.opcode, crate::opcode::Opcode::Ldxdw);
        assert_eq!(inst.dst.as_ref().unwrap().n, 2);
        assert_eq!(inst.src.as_ref().unwrap().n, 1);

        // mul64 imm (0x27) -> stb
        let b = hex!("2701050042000000");
        let inst = Instruction::from_bytes::<SbpfV2>(&b).unwrap();
        assert_eq!(inst.opcode, crate::opcode::Opcode::Stb);
        assert_eq!(inst.dst.as_ref().unwrap().n, 1);

        // mul64 reg (0x2f) -> stxb
        let b = hex!("2f12080000000000");
        let inst = Instruction::from_bytes::<SbpfV2>(&b).unwrap();
        assert_eq!(inst.opcode, crate::opcode::Opcode::Stxb);
        assert_eq!(inst.dst.as_ref().unwrap().n, 2);
        assert_eq!(inst.src.as_ref().unwrap().n, 1);
    }
}
