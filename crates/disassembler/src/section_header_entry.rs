use {
    crate::errors::DisassemblerError,
    sbpf_common::instruction::Instruction,
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
    pub fn new(label: String, offset: usize, data: Vec<u8>) -> Result<Self, DisassemblerError> {
        let mut h = SectionHeaderEntry {
            label,
            offset,
            data,
            ixs: vec![],
            utf8: String::new(),
        };

        if let Ok(utf8) = String::from_utf8(h.data.clone()) {
            h.utf8 = utf8;
        }
        Ok(h)
    }

    pub fn offset(&self) -> usize {
        self.offset
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
        },
    };

    #[test]
    fn serialize_e2e() {
        let data = vec![
            0x18, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x95, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let h = SectionHeaderEntry::new(".text\0".to_string(), 128, data.clone()).unwrap();

        let ixs = [
            Instruction {
                opcode: Opcode::Lddw,
                dst: Some(Register { n: 1 }),
                src: None,
                off: None,
                imm: Some(Either::Right(Number::Int(0))),
                span: 0..16,
            }
            .to_bytes()
            .unwrap(),
            Instruction {
                opcode: Opcode::Exit,
                dst: None,
                src: None,
                off: None,
                imm: None,
                span: 0..8,
            }
            .to_bytes()
            .unwrap(),
        ]
        .concat();

        assert_eq!(ixs, h.to_bytes());
    }
}
