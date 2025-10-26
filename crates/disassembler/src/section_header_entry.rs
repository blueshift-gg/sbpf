use std::fmt::Debug;

use serde::{Deserialize, Serialize};

use crate::errors::DisassemblerError;
use sbpf_common::instruction::Instruction;
use sbpf_common::opcode::Opcode;

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

        if h.label.contains(".text\0") {
            h.ixs = h.to_ixs()?;
        }

        if let Ok(utf8) = String::from_utf8(h.data.clone()) {
            h.utf8 = utf8;
        }
        Ok(h)
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn to_ixs(&self) -> Result<Vec<Instruction>, DisassemblerError> {
        if self.data.len() % 8 != 0 {
            return Err(DisassemblerError::InvalidDataLength);
        }
        let mut ixs: Vec<Instruction> = vec![];
        let mut pos = 0;

        while pos < self.data.len() {
            let remaining = &self.data[pos..];
            if remaining.len() < 8 {
                break;
            }

            let ix = Instruction::from_bytes(remaining)?;
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
    use crate::section_header_entry::SectionHeaderEntry;
    use sbpf_common::inst_param::{
        Number,
        Register
    };
    use sbpf_common::instruction::Instruction;
    use sbpf_common::opcode::Opcode;

    #[test]
    fn serialize_e2e() {
        let data = vec![
            0x18, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x95, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let h = SectionHeaderEntry::new(".text\0".to_string(), 128, data.clone()).unwrap();

        let ixs = vec![
            Instruction {
                opcode: Opcode::Lddw,
                dst: Some(Register { n: 1 }),
                src: None,
                off: None,
                imm: Some(Number::Int(0)),
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
        assert_eq!(ixs, h.to_ixs().unwrap());

        assert_eq!(
            data,
            h.to_ixs()
                .expect("Invalid IX")
                .into_iter()
                .flat_map(|i| i.to_bytes())
                .collect::<Vec<u8>>()
        )
    }
}
