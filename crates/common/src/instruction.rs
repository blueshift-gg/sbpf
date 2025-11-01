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
        let opcode: Opcode = bytes[0].try_into()?;
        if let Some(handler) = OPCODE_TO_HANDLER.get(&opcode) {
            let mut inst = (handler.decode)(bytes)?;

            // Apply platform-specific callx decoding to convert to standard BPF convention
            if inst.opcode == Opcode::Callx {
                let dst_val = inst.dst.as_ref().map(|r| r.n).unwrap_or(0);
                let imm_val = match &inst.imm {
                    Some(Either::Right(Number::Int(imm))) | Some(Either::Right(Number::Addr(imm))) => *imm as i32,
                    _ => 0,
                };
                let (new_dst, new_imm) = Platform::decode_callx(dst_val, imm_val);
                inst.dst = if new_dst != 0 { Some(Register { n: new_dst }) } else { None };
                inst.imm = if new_imm != 0 {
                    Some(Either::Right(Number::Int(new_imm as i64)))
                } else {
                    None
                };
            }

            Ok(inst)
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
        // Apply platform-specific callx encoding
        let (dst_val, imm_val) = if self.opcode == Opcode::Callx {
            let (dst, imm) = Platform::encode_callx(dst_val, imm_val as i32);
            (dst, imm as i64)
        } else {
            (dst_val, imm_val)
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
}
