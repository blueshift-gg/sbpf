use std::{fmt::Debug, io::Cursor};

use serde::{Deserialize, Serialize};

use crate::{cursor::ELFCursor, errors::EZBpfError};

// Program Segment Flags
pub const PF_X: u8 = 0x01;
pub const PF_W: u8 = 0x02;
pub const PF_R: u8 = 0x04;

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(u32)]
pub enum ProgramType {
    PT_NULL = 0x00,    // Program header table entry unused.
    PT_LOAD = 0x01,    // Loadable segment.
    PT_DYNAMIC = 0x02, // Dynamic linking information.
    PT_INTERP = 0x03,  // Interpreter information.
    PT_NOTE = 0x04,    // Auxiliary information.
    PT_SHLIB = 0x05,   // Reserved.
    PT_PHDR = 0x06,    // Segment containing program header table itself.
    PT_TLS = 0x07,     // Thread-Local Storage template.
}

impl TryFrom<u32> for ProgramType {
    type Error = EZBpfError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::PT_NULL,
            1 => Self::PT_LOAD,
            2 => Self::PT_DYNAMIC,
            3 => Self::PT_INTERP,
            4 => Self::PT_NOTE,
            5 => Self::PT_SHLIB,
            6 => Self::PT_PHDR,
            7 => Self::PT_TLS,
            _ => return Err(EZBpfError::InvalidProgramType),
        })
    }
}

impl From<ProgramType> for u32 {
    fn from(val: ProgramType) -> Self {
        match val {
            ProgramType::PT_NULL => 0,
            ProgramType::PT_LOAD => 1,
            ProgramType::PT_DYNAMIC => 2,
            ProgramType::PT_INTERP => 3,
            ProgramType::PT_NOTE => 4,
            ProgramType::PT_SHLIB => 5,
            ProgramType::PT_PHDR => 6,
            ProgramType::PT_TLS => 7,
        }
    }
}

impl From<ProgramType> for &str {
    fn from(val: ProgramType) -> Self {
        match val {
            ProgramType::PT_NULL => "PT_NULL",
            ProgramType::PT_LOAD => "PT_LOAD",
            ProgramType::PT_DYNAMIC => "PT_DYNAMIC",
            ProgramType::PT_INTERP => "PT_INTERP",
            ProgramType::PT_NOTE => "PT_NOTE",
            ProgramType::PT_SHLIB => "PT_SHLIB",
            ProgramType::PT_PHDR => "PT_PHDR",
            ProgramType::PT_TLS => "PT_TLS",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramFlags(pub u32);

impl From<u32> for ProgramFlags {
    fn from(value: u32) -> Self {
        Self(value & 7)
    }
}

impl std::fmt::Display for ProgramFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let x = match self.0 & PF_X as u32 == PF_X as u32 {
            true => "X",
            false => "*",
        };

        let r = match self.0 & PF_R as u32 == PF_R as u32 {
            true => "R",
            false => "*",
        };

        let w = match self.0 & PF_W as u32 == PF_W as u32 {
            true => "W",
            false => "*",
        };
        f.write_str(&format!("{}/{}/{}", r, w, x))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramHeader {
    pub p_type: ProgramType, // An offset to a string in the .shstrtab section that represents the name of this section.
    pub p_flags: ProgramFlags, // Identifies the type of this header.
    pub p_offset: u64,       // Offset of the segment in the file image.
    pub p_vaddr: u64,        // Virtual address of the segment in memory.
    pub p_paddr: u64, // On systems where physical address is relevant, reserved for segment's physical address.
    pub p_filesz: u64, // Size in bytes of the section in the file image. May be 0.
    pub p_memsz: u64, // Size in bytes of the segment in memory. May be 0.
    pub p_align: u64, // 0 and 1 specify no alignment. Otherwise should be a positive, integral power of 2, with p_vaddr equating p_offset modulus p_align.
}

impl ProgramHeader {
    pub fn from_bytes(b: &[u8]) -> Result<Self, EZBpfError> {
        let mut c = Cursor::new(b);
        c.read_program_header()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = (self.p_type.clone() as u32).to_le_bytes().to_vec();
        b.extend_from_slice(&self.p_flags.0.to_le_bytes());
        b.extend_from_slice(&self.p_offset.to_le_bytes());
        b.extend_from_slice(&self.p_vaddr.to_le_bytes());
        b.extend_from_slice(&self.p_paddr.to_le_bytes());
        b.extend_from_slice(&self.p_filesz.to_le_bytes());
        b.extend_from_slice(&self.p_memsz.to_le_bytes());
        b.extend_from_slice(&self.p_align.to_le_bytes());
        b
    }
}

#[cfg(test)]
mod tests {
    use hex_literal::hex;

    use crate::program_header::ProgramHeader;

    #[test]
    fn serialize_e2e() {
        let b = hex!(
            "0100000005000000780000000000000078000000000000007800000000000000080000000000000008000000000000000010000000000000"
        );
        let h = ProgramHeader::from_bytes(&b).unwrap();
        assert_eq!(h.to_bytes(), &b)
    }
}
