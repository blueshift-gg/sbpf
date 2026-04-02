use {
    crate::{
        errors::SBPFError,
        inst_handler::{OPCODE_TO_HANDLER, OPCODE_TO_TYPE},
        inst_param::{Number, Register},
        opcode::{Opcode, OperationType},
        syscalls::REGISTERED_SYSCALLS,
    },
    core::ops::Range,
    either::Either,
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AsmFormat {
    #[default]
    Default,
    Llvm,
}

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

    /// Checks if the instruction is a syscall.
    /// This should be used only when the call label hasn't been resolved to -1.
    pub fn is_syscall(&self) -> bool {
        if self.opcode == Opcode::Call
            && let Some(Either::Left(identifier)) = &self.imm
        {
            return REGISTERED_SYSCALLS.contains(&identifier.as_str());
        }
        false
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
        let src_val = self.src.as_ref().map(|r| r.n).unwrap_or(0);
        let off_val = match &self.off {
            Some(Either::Left(ident)) => {
                unreachable!("Identifier '{}' should have been resolved earlier", ident)
            }
            Some(Either::Right(off)) => *off,
            None => 0,
        };
        let imm_val = match &self.imm {
            Some(Either::Left(ident)) => {
                unreachable!("Identifier '{}' should have been resolved earlier", ident)
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

    pub fn to_asm(&self, format: AsmFormat) -> Result<String, SBPFError> {
        match format {
            AsmFormat::Default => self.to_default_asm(),
            AsmFormat::Llvm => self.to_llvm_asm(),
        }
    }

    fn to_default_asm(&self) -> Result<String, SBPFError> {
        if let Some(handler) = OPCODE_TO_HANDLER.get(&self.opcode) {
            match (handler.validate)(self) {
                Ok(()) => {
                    let mut asm = if self.opcode == Opcode::Le || self.opcode == Opcode::Be {
                        self.op_imm_bits()?
                    } else {
                        format!("{}", self.opcode)
                    };
                    let mut param = vec![];

                    fn fmt_mem_off(r: &Register, off: &Either<String, i16>) -> String {
                        format!("[r{}{}]", r.n, fmt_off(off))
                    }

                    if self.get_opcode_type() == OperationType::LoadMemory {
                        param.push(format!("r{}", self.dst.as_ref().unwrap().n));
                        param.push(fmt_mem_off(
                            self.src.as_ref().unwrap(),
                            self.off.as_ref().unwrap(),
                        ));
                    } else if self.get_opcode_type() == OperationType::StoreImmediate {
                        param.push(fmt_mem_off(
                            self.dst.as_ref().unwrap(),
                            self.off.as_ref().unwrap(),
                        ));
                        param.push(fmt_imm(self.imm.as_ref().unwrap()));
                    } else if self.get_opcode_type() == OperationType::StoreRegister {
                        param.push(fmt_mem_off(
                            self.dst.as_ref().unwrap(),
                            self.off.as_ref().unwrap(),
                        ));
                        param.push(format!("r{}", self.src.as_ref().unwrap().n));
                    } else {
                        if let Some(dst) = &self.dst {
                            param.push(format!("r{}", dst.n));
                        }
                        if let Some(src) = &self.src
                            && self.opcode != Opcode::Call
                        {
                            param.push(format!("r{}", src.n));
                        }
                        if let Some(imm) = &self.imm
                            && self.opcode != Opcode::Le
                            && self.opcode != Opcode::Be
                        {
                            param.push(fmt_imm(imm));
                        }
                        if let Some(off) = &self.off {
                            param.push(fmt_off(off));
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

    fn to_llvm_asm(&self) -> Result<String, SBPFError> {
        let op_type = self.get_opcode_type();

        fn fmt_mem_off(off: &Either<String, i16>) -> String {
            match off {
                Either::Left(label) => label.clone(),
                Either::Right(v) if *v < 0 => format!("- 0x{:x}", -(*v as i32)),
                Either::Right(v) => format!("+ 0x{:x}", v),
            }
        }

        match op_type {
            OperationType::BinaryImmediate | OperationType::BinaryRegister => {
                if self.opcode == Opcode::Le || self.opcode == Opcode::Be {
                    let bits = self.op_imm_bits()?;
                    let dst = self.dst.as_ref().unwrap().n;
                    return Ok(format!("r{} = {} r{}", dst, bits, dst));
                }
                let op = self
                    .opcode
                    .to_operator()
                    .ok_or_else(|| SBPFError::BytecodeError {
                        error: format!("unsupported opcode in LLVM format: {}", self.opcode),
                        span: self.span.clone(),
                        custom_label: None,
                    })?;
                let prefix = if self.opcode.is_32bit() { "w" } else { "r" };
                let dst = self.dst.as_ref().unwrap().n;
                let rhs = if op_type == OperationType::BinaryRegister {
                    format!("{}{}", prefix, self.src.as_ref().unwrap().n)
                } else {
                    fmt_imm(self.imm.as_ref().unwrap())
                };
                Ok(format!("{}{} {} {}", prefix, dst, op, rhs))
            }
            OperationType::Unary => {
                let prefix = if self.opcode == Opcode::Neg32 {
                    "w"
                } else {
                    "r"
                };
                let dst = self.dst.as_ref().unwrap().n;
                Ok(format!("{}{} = -{}{}", prefix, dst, prefix, dst))
            }
            OperationType::LoadImmediate => {
                let dst = self.dst.as_ref().unwrap().n;
                let imm = fmt_imm(self.imm.as_ref().unwrap());
                Ok(format!("r{} = {} ll", dst, imm))
            }
            OperationType::LoadMemory => {
                let size = self.opcode.to_size().unwrap();
                let dst_prefix = if self.opcode == Opcode::Ldxdw {
                    "r"
                } else {
                    "w"
                };
                let dst = self.dst.as_ref().unwrap().n;
                let src = self.src.as_ref().unwrap().n;
                let off = fmt_mem_off(self.off.as_ref().unwrap());
                Ok(format!(
                    "{}{} = *({} *)(r{} {})",
                    dst_prefix, dst, size, src, off
                ))
            }
            OperationType::StoreImmediate => {
                let size = self.opcode.to_size().unwrap();
                let dst = self.dst.as_ref().unwrap().n;
                let off = fmt_mem_off(self.off.as_ref().unwrap());
                let imm = fmt_imm(self.imm.as_ref().unwrap());
                Ok(format!("*({} *)(r{} {}) = {}", size, dst, off, imm))
            }
            OperationType::StoreRegister => {
                let size = self.opcode.to_size().unwrap();
                let dst = self.dst.as_ref().unwrap().n;
                let off = fmt_mem_off(self.off.as_ref().unwrap());
                let src_prefix = if self.opcode == Opcode::Stxdw {
                    "r"
                } else {
                    "w"
                };
                let src = self.src.as_ref().unwrap().n;
                Ok(format!(
                    "*({} *)(r{} {}) = {}{}",
                    size, dst, off, src_prefix, src
                ))
            }
            OperationType::Jump => {
                let off = fmt_off(self.off.as_ref().unwrap());
                Ok(format!("goto {}", off))
            }
            OperationType::JumpImmediate => {
                let dst = self.dst.as_ref().unwrap().n;
                let op = self.opcode.to_operator().unwrap();
                let imm = fmt_imm(self.imm.as_ref().unwrap());
                let off = fmt_off(self.off.as_ref().unwrap());
                Ok(format!("if r{} {} {} goto {}", dst, op, imm, off))
            }
            OperationType::JumpRegister => {
                let dst = self.dst.as_ref().unwrap().n;
                let op = self.opcode.to_operator().unwrap();
                let src = self.src.as_ref().unwrap().n;
                let off = fmt_off(self.off.as_ref().unwrap());
                Ok(format!("if r{} {} r{} goto {}", dst, op, src, off))
            }
            OperationType::CallImmediate | OperationType::CallRegister | OperationType::Exit => {
                self.to_default_asm()
            }
        }
    }
}

fn fmt_off(off: &Either<String, i16>) -> String {
    match off {
        Either::Left(label) => label.clone(),
        Either::Right(v) if *v < 0 => format!("-0x{:x}", -(*v as i32)),
        Either::Right(v) => format!("+0x{:x}", v),
    }
}

fn fmt_imm(imm: &Either<String, Number>) -> String {
    match imm {
        Either::Left(label) => label.clone(),
        Either::Right(Number::Int(v)) | Either::Right(Number::Addr(v)) => {
            if *v < 0 {
                format!("-0x{:x}", -v)
            } else {
                format!("0x{:x}", v)
            }
        }
    }
}

#[cfg(test)]
mod test {
    use {
        crate::{
            inst_param::{Number, Register},
            instruction::{AsmFormat, Instruction},
            opcode::Opcode,
        },
        either::Either,
        hex_literal::hex,
        syscall_map::murmur3_32,
    };

    #[test]
    fn serialize_e2e() {
        let b = hex!("9700000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm(AsmFormat::Default).unwrap(), "mod64 r0, 0x0");
        assert_eq!(i.to_asm(AsmFormat::Llvm).unwrap(), "r0 %= 0x0");
    }

    #[test]
    fn serialize_e2e_lddw() {
        let b = hex!("18010000000000000000000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm(AsmFormat::Default).unwrap(), "lddw r1, 0x0");
        assert_eq!(i.to_asm(AsmFormat::Llvm).unwrap(), "r1 = 0x0 ll");
    }

    #[test]
    fn serialize_e2e_add64_imm() {
        let b = hex!("0701000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm(AsmFormat::Default).unwrap(), "add64 r1, 0x0");
        assert_eq!(i.to_asm(AsmFormat::Llvm).unwrap(), "r1 += 0x0");
    }

    #[test]
    fn serialize_e2e_add64_reg() {
        let b = hex!("0f12000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm(AsmFormat::Default).unwrap(), "add64 r2, r1");
        assert_eq!(i.to_asm(AsmFormat::Llvm).unwrap(), "r2 += r1");
    }

    #[test]
    fn serialize_e2e_ja() {
        let b = hex!("05000a0000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm(AsmFormat::Default).unwrap(), "ja +0xa");
        assert_eq!(i.to_asm(AsmFormat::Llvm).unwrap(), "goto +0xa");
    }

    #[test]
    fn serialize_e2e_jeq_imm() {
        let b = hex!("15030a0001000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm(AsmFormat::Default).unwrap(), "jeq r3, 0x1, +0xa");
        assert_eq!(i.to_asm(AsmFormat::Llvm).unwrap(), "if r3 == 0x1 goto +0xa");
    }

    #[test]
    fn serialize_e2e_jeq_reg() {
        let b = hex!("1d210a0000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm(AsmFormat::Default).unwrap(), "jeq r1, r2, +0xa");
        assert_eq!(i.to_asm(AsmFormat::Llvm).unwrap(), "if r1 == r2 goto +0xa");
    }

    #[test]
    fn serialize_e2e_ldxw() {
        let b = hex!("6112000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm(AsmFormat::Default).unwrap(), "ldxw r2, [r1+0x0]");
        assert_eq!(
            i.to_asm(AsmFormat::Llvm).unwrap(),
            "w2 = *(u32 *)(r1 + 0x0)"
        );
    }

    #[test]
    fn serialize_e2e_stxw() {
        let b = hex!("6312000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm(AsmFormat::Default).unwrap(), "stxw [r2+0x0], r1");
        assert_eq!(
            i.to_asm(AsmFormat::Llvm).unwrap(),
            "*(u32 *)(r2 + 0x0) = w1"
        );
    }

    #[test]
    fn serialize_e2e_stb() {
        let b = hex!("7200000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.opcode, Opcode::Stb);
        assert!(i.src.is_none());
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm(AsmFormat::Default).unwrap(), "stb [r0+0x0], 0x0");
        assert_eq!(
            i.to_asm(AsmFormat::Llvm).unwrap(),
            "*(u8 *)(r0 + 0x0) = 0x0"
        );
    }

    #[test]
    fn serialize_e2e_sth() {
        let b = hex!("6a01040034120000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.opcode, Opcode::Sth);
        assert!(i.src.is_none());
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(
            i.to_asm(AsmFormat::Default).unwrap(),
            "sth [r1+0x4], 0x1234"
        );
        assert_eq!(
            i.to_asm(AsmFormat::Llvm).unwrap(),
            "*(u16 *)(r1 + 0x4) = 0x1234"
        );
    }

    #[test]
    fn serialize_e2e_stw() {
        let b = hex!("6201080064000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.opcode, Opcode::Stw);
        assert!(i.src.is_none());
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm(AsmFormat::Default).unwrap(), "stw [r1+0x8], 0x64");
        assert_eq!(
            i.to_asm(AsmFormat::Llvm).unwrap(),
            "*(u32 *)(r1 + 0x8) = 0x64"
        );
    }

    #[test]
    fn serialize_e2e_stdw() {
        let b = hex!("7a021000efbeadde");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.opcode, Opcode::Stdw);
        assert!(i.src.is_none());
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(
            i.to_asm(AsmFormat::Default).unwrap(),
            "stdw [r2+0x10], -0x21524111"
        );
        assert_eq!(
            i.to_asm(AsmFormat::Llvm).unwrap(),
            "*(u64 *)(r2 + 0x10) = -0x21524111"
        );
    }

    #[test]
    fn serialize_e2e_le16() {
        let b = hex!("d401000010000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.opcode, Opcode::Le);
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm(AsmFormat::Default).unwrap(), "le16 r1");
        assert_eq!(i.to_asm(AsmFormat::Llvm).unwrap(), "r1 = le16 r1");
    }

    #[test]
    fn serialize_e2e_le32() {
        let b = hex!("d401000020000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.opcode, Opcode::Le);
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm(AsmFormat::Default).unwrap(), "le32 r1");
        assert_eq!(i.to_asm(AsmFormat::Llvm).unwrap(), "r1 = le32 r1");
    }

    #[test]
    fn serialize_e2e_le64() {
        let b = hex!("d403000040000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.opcode, Opcode::Le);
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm(AsmFormat::Default).unwrap(), "le64 r3");
        assert_eq!(i.to_asm(AsmFormat::Llvm).unwrap(), "r3 = le64 r3");
    }

    #[test]
    fn serialize_e2e_be16() {
        let b = hex!("dc01000010000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.opcode, Opcode::Be);
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm(AsmFormat::Default).unwrap(), "be16 r1");
        assert_eq!(i.to_asm(AsmFormat::Llvm).unwrap(), "r1 = be16 r1");
    }

    #[test]
    fn serialize_e2e_be32() {
        let b = hex!("dc02000020000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.opcode, Opcode::Be);
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm(AsmFormat::Default).unwrap(), "be32 r2");
        assert_eq!(i.to_asm(AsmFormat::Llvm).unwrap(), "r2 = be32 r2");
    }

    #[test]
    fn serialize_e2e_be64() {
        let b = hex!("dc03000040000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.opcode, Opcode::Be);
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm(AsmFormat::Default).unwrap(), "be64 r3");
        assert_eq!(i.to_asm(AsmFormat::Llvm).unwrap(), "r3 = be64 r3");
    }

    #[test]
    fn serialize_e2e_neg64() {
        let b = hex!("8700000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm(AsmFormat::Default).unwrap(), "neg64 r0");
        assert_eq!(i.to_asm(AsmFormat::Llvm).unwrap(), "r0 = -r0");
    }

    #[test]
    fn serialize_e2e_exit() {
        let b = hex!("9500000000000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm(AsmFormat::Default).unwrap(), "exit");
        assert_eq!(i.to_asm(AsmFormat::Llvm).unwrap(), "exit");
    }

    #[test]
    fn serialize_e2e_jset_imm() {
        let b = hex!("45030a0010000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm(AsmFormat::Default).unwrap(), "jset r3, 0x10, +0xa");
        assert_eq!(i.to_asm(AsmFormat::Llvm).unwrap(), "if r3 & 0x10 goto +0xa");
    }

    #[test]
    fn serialize_e2e_sub32_imm() {
        let b = hex!("1401000042000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm(AsmFormat::Default).unwrap(), "sub32 r1, 0x42");
        assert_eq!(i.to_asm(AsmFormat::Llvm).unwrap(), "w1 -= 0x42");
    }

    #[test]
    fn serialize_e2e_mov32_imm() {
        let b = hex!("b400000001000000");
        let i = Instruction::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes().unwrap(), &b);
        assert_eq!(i.to_asm(AsmFormat::Default).unwrap(), "mov32 r0, 0x1");
        assert_eq!(i.to_asm(AsmFormat::Llvm).unwrap(), "w0 = 0x1");
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
    #[should_panic(expected = "should have been resolved earlier")]
    fn test_to_bytes_call_with_identifier() {
        let inst = Instruction {
            opcode: Opcode::Call,
            dst: None,
            src: None,
            off: None,
            imm: Some(Either::Left("function".to_string())),
            span: 0..8,
        };
        // This should panic because "function" does not exist
        let _ = inst.to_bytes().unwrap();
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

    #[test]
    fn test_is_syscall() {
        let test_cases = vec![
            // Syscalls
            ("sol_log_", true),
            ("sol_invoke_signed_c", true),
            ("abort", true),
            ("sol_sha256", true),
            ("sol_memcpy_", true),
            // Non-syscalls
            ("my_fn", false),
            ("helper_function", false),
            ("entrypoint", false),
            ("random", false),
        ];

        for (name, expected) in test_cases {
            let inst = Instruction {
                opcode: Opcode::Call,
                dst: None,
                src: Some(Register { n: 1 }),
                off: None,
                imm: Some(Either::Left(name.to_string())),
                span: 0..8,
            };
            assert_eq!(inst.is_syscall(), expected);
        }
    }

    #[test]
    fn test_to_bytes_syscall_dynamic() {
        let inst = Instruction {
            opcode: Opcode::Call,
            dst: None,
            src: Some(Register { n: 1 }),
            off: None,
            imm: Some(Either::Right(Number::Int(-1))),
            span: 0..8,
        };
        let bytes = inst.to_bytes().unwrap();
        assert_eq!(bytes[0], 0x85);
        assert_eq!(bytes[1], 0x10);

        // imm should be -1 (FF FF FF FF)
        assert_eq!(&bytes[4..8], &[0xFF, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn test_to_bytes_syscall_static() {
        let syscall_hash = murmur3_32("sol_log_");
        let inst = Instruction {
            opcode: Opcode::Call,
            dst: None,
            src: Some(Register { n: 0 }),
            off: None,
            imm: Some(Either::Right(Number::Int(syscall_hash as i64))),
            span: 0..8,
        };
        let bytes = inst.to_bytes().unwrap();
        assert_eq!(bytes[0], 0x85);
        assert_eq!(bytes[1], 0x00);

        // imm should be the murmur3_32 hash
        let actual_imm = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        assert_eq!(actual_imm, syscall_hash);
    }
}
