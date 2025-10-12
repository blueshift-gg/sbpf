use std::fmt::Debug;

use serde::{Deserialize, Serialize};

use crate::{errors::EZBpfError, instructions::Ix};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionHeaderEntry {
    pub label: String,
    pub offset: usize,
    pub data: Vec<u8>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ixs: Vec<Ix>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub utf8: String,
}

impl SectionHeaderEntry {
    pub fn new(label: String, offset: usize, data: Vec<u8>) -> Result<Self, EZBpfError> {
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

    pub fn to_ixs(&self) -> Result<Vec<Ix>, EZBpfError> {
        if self.data.len() % 8 != 0 {
            return Err(EZBpfError::InvalidDataLength);
        }
        let mut ixs: Vec<Ix> = vec![];
        let mut pos = 0;

        while pos < self.data.len() {
            let remaining = &self.data[pos..];
            if remaining.len() < 8 {
                break;
            }

            let ix = Ix::from_bytes(remaining)?;
            if ix.op == crate::opcodes::OpCode::Lddw {
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
    use crate::{instructions::Ix, opcodes::OpCode, section_header_entry::SectionHeaderEntry};

    #[test]
    fn serialize_e2e() {
        let data = vec![
            0x18, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x95, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let h = SectionHeaderEntry::new(".text\0".to_string(), 128, data.clone()).unwrap();

        let ixs = vec![
            Ix {
                op: OpCode::Lddw,
                dst: 1,
                src: 0,
                off: 0,
                imm: 0,
            },
            Ix {
                op: OpCode::Exit,
                dst: 0,
                src: 0,
                off: 0,
                imm: 0,
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
