use {
    crate::errors::DisassemblerError,
    sbpf_common::{
        instruction::Instruction,
        opcode::Opcode,
        platform::BPFPlatform,
    },
    serde::{Deserialize, Serialize},
    std::fmt::Debug,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionHeaderEntry {
    pub label: String,
    pub offset: usize,
    pub data: Vec<u8>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ixs: Vec<Instruction>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub utf8: String,
}

impl SectionHeaderEntry {
    pub fn new<Platform: BPFPlatform>(label: String, offset: usize, data: Vec<u8>) -> Result<Self, DisassemblerError> {
        let mut h = SectionHeaderEntry {
            label,
            offset,
            data,
            ixs: vec![],
            utf8: String::new(),
        };

        if h.label.contains(".text\0") {
            h.ixs = h.to_ixs::<Platform>()?;
        }

        if let Ok(utf8) = String::from_utf8(h.data.clone()) {
            h.utf8 = utf8;
        }
        Ok(h)
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn to_ixs<Platform: BPFPlatform>(&self) -> Result<Vec<Instruction>, DisassemblerError> {
        if !self.data.len().is_multiple_of(8) {
            return Err(DisassemblerError::InvalidDataLength);
        }
        let mut ixs: Vec<Instruction> = vec![];
        let mut pos = 0;

        while pos < self.data.len() {
            let remaining = &self.data[pos..];
            if remaining.len() < 8 {
                break;
            }

            let ix = Instruction::from_bytes::<Platform>(remaining)?;
            if ix.opcode == Opcode::Lddw {
                pos += 16;
            } else {
                pos += 8;
            }

            ixs.push(ix);
        }

        Ok(ixs)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.data.clone()
    }
}

#[cfg(test)]
mod test {
    use {
        crate::section_header_entry::SectionHeaderEntry,
        either::Either,
        sbpf_common::{
            inst_param::{Number, Register},
            instruction::Instruction,
            opcode::Opcode,
            platform::SbpfV0,
        },
    };

    #[test]
    fn serialize_e2e() {
        let data = vec![
            0x18, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x95, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let h = SectionHeaderEntry::new::<SbpfV0>(".text\0".to_string(), 128, data.clone()).unwrap();

        let ixs = vec![
            Instruction {
                opcode: Opcode::Lddw,
                dst: Some(Register { n: 1 }),
                src: None,
                off: None,
                imm: Some(Either::Right(Number::Int(0))),
                span: 0..16,
            },
            Instruction {
                opcode: Opcode::Exit,
                dst: None,
                src: None,
                off: None,
                imm: None,
                span: 0..8,
            },
        ];
        assert_eq!(ixs, h.to_ixs::<SbpfV0>().unwrap());

        assert_eq!(
            data,
            h.to_ixs::<SbpfV0>()
                .expect("Invalid IX")
                .into_iter()
                .flat_map(|i| i.to_bytes::<SbpfV0>().unwrap())
                .collect::<Vec<u8>>()
        )
    }
}
