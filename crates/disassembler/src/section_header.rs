use std::{fmt::Debug, fmt::Display, io::Cursor};

use serde::{Deserialize, Serialize};

use crate::{cursor::ELFCursor, errors::EZBpfError, instructions::Ix};

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(u32)]
pub enum SectionHeaderType {
    SHT_NULL = 0x00,          // Section header table entry unused
    SHT_PROGBITS = 0x01,      // Program data
    SHT_SYMTAB = 0x02,        // Symbol table
    SHT_STRTAB = 0x03,        // String table
    SHT_RELA = 0x04,          // Relocation entries with addends
    SHT_HASH = 0x05,          // Symbol hash table
    SHT_DYNAMIC = 0x06,       // Dynamic linking information
    SHT_NOTE = 0x07,          // Notes
    SHT_NOBITS = 0x08,        // Program space with no data (bss)
    SHT_REL = 0x09,           // Relocation entries, no addends
    SHT_SHLIB = 0x0A,         // Reserved
    SHT_DYNSYM = 0x0B,        // Dynamic linker symbol table
    SHT_INIT_ARRAY = 0x0E,    // Array of constructors
    SHT_FINI_ARRAY = 0x0F,    // Array of destructors
    SHT_PREINIT_ARRAY = 0x10, // Array of pre-constructors
    SHT_GROUP = 0x11,         // Section group
    SHT_SYMTAB_SHNDX = 0x12,  //	Extended section indices
    SHT_NUM = 0x13,           // Number of defined types.
}

impl TryFrom<u32> for SectionHeaderType {
    type Error = EZBpfError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Ok(match value {
            0x00 => Self::SHT_NULL,
            0x01 => Self::SHT_PROGBITS,
            0x02 => Self::SHT_SYMTAB,
            0x03 => Self::SHT_STRTAB,
            0x04 => Self::SHT_RELA,
            0x05 => Self::SHT_HASH,
            0x06 => Self::SHT_DYNAMIC,
            0x07 => Self::SHT_NOTE,
            0x08 => Self::SHT_NOBITS,
            0x09 => Self::SHT_REL,
            0x0A => Self::SHT_SHLIB,
            0x0B => Self::SHT_DYNSYM,
            0x0E => Self::SHT_INIT_ARRAY,
            0x0F => Self::SHT_FINI_ARRAY,
            0x10 => Self::SHT_PREINIT_ARRAY,
            0x11 => Self::SHT_GROUP,
            0x12 => Self::SHT_SYMTAB_SHNDX,
            0x13 => Self::SHT_NUM,
            _ => return Err(EZBpfError::InvalidSectionHeaderType),
        })
    }
}

impl Display for SectionHeaderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(Into::<&str>::into(self.clone()))
    }
}

impl From<SectionHeaderType> for &str {
    fn from(val: SectionHeaderType) -> Self {
        match val {
            SectionHeaderType::SHT_NULL => "SHT_NULL",
            SectionHeaderType::SHT_PROGBITS => "SHT_PROGBITS",
            SectionHeaderType::SHT_SYMTAB => "SHT_SYMTAB",
            SectionHeaderType::SHT_STRTAB => "SHT_STRTAB",
            SectionHeaderType::SHT_RELA => "SHT_RELA",
            SectionHeaderType::SHT_HASH => "SHT_HASH",
            SectionHeaderType::SHT_DYNAMIC => "SHT_DYNAMIC",
            SectionHeaderType::SHT_NOTE => "SHT_NOTE",
            SectionHeaderType::SHT_NOBITS => "SHT_NOBITS",
            SectionHeaderType::SHT_REL => "SHT_REL",
            SectionHeaderType::SHT_SHLIB => "SHT_SHLIB",
            SectionHeaderType::SHT_DYNSYM => "SHT_DYNSYM",
            SectionHeaderType::SHT_INIT_ARRAY => "SHT_INIT_ARRAY",
            SectionHeaderType::SHT_FINI_ARRAY => "SHT_FINI_ARRAY",
            SectionHeaderType::SHT_PREINIT_ARRAY => "SHT_PREINIT_ARRAY",
            SectionHeaderType::SHT_GROUP => "SHT_GROUP",
            SectionHeaderType::SHT_SYMTAB_SHNDX => "SHT_SYMTAB_SHNDX",
            SectionHeaderType::SHT_NUM => "SHT_NUM",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionHeader {
    pub sh_name: u32, // An offset to a string in the .shstrtab section that represents the name of this section.
    pub sh_type: SectionHeaderType, // Identifies the type of this header.
    pub sh_flags: u64, // Identifies the attributes of the section.
    pub sh_addr: u64, // Virtual address of the section in memory, for sections that are loaded.
    pub sh_offset: u64, // Offset of the section in the file image.
    pub sh_size: u64, // Size in bytes of the section in the file image. May be 0.
    pub sh_link: u32, // Contains the section index of an associated section. This field is used for several purposes, depending on the type of section.
    pub sh_info: u32, // Contains extra information about the section. This field is used for several purposes, depending on the type of section.
    pub sh_addralign: u64, // Contains the required alignment of the section. This field must be a power of two.
    pub sh_entsize: u64, // Contains the size, in bytes, of each entry, for sections that contain fixed-size entries. Otherwise, this field contains zero.
}

impl SectionHeader {
    pub fn from_bytes(b: &[u8]) -> Result<Self, EZBpfError> {
        let mut c = Cursor::new(b);
        c.read_section_header()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = self.sh_name.to_le_bytes().to_vec();
        b.extend_from_slice(&(self.sh_type.clone() as u32).to_le_bytes());
        b.extend_from_slice(&self.sh_flags.to_le_bytes());
        b.extend_from_slice(&self.sh_addr.to_le_bytes());
        b.extend_from_slice(&self.sh_offset.to_le_bytes());
        b.extend_from_slice(&self.sh_size.to_le_bytes());
        b.extend_from_slice(&self.sh_link.to_le_bytes());
        b.extend_from_slice(&self.sh_info.to_le_bytes());
        b.extend_from_slice(&self.sh_addralign.to_le_bytes());
        b.extend_from_slice(&self.sh_entsize.to_le_bytes());
        b
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionHeaderEntry {
    pub label: String,
    pub offset: usize,
    pub data: Vec<u8>,
}

impl SectionHeaderEntry {
    pub fn to_ixs(&self) -> Result<Vec<Ix>, EZBpfError> {
        if self.data.len() % 8 != 0 {
            return Err(EZBpfError::InvalidDataLength);
        }
        let mut ixs: Vec<Ix> = vec![];
        if self.data.len() >= 8 {
            let mut c = Cursor::new(self.data.as_slice());
            while let Ok(ix) = c.read_ix() {
                ixs.push(ix)
            }
        }
        Ok(ixs)
    }
}

#[cfg(test)]
mod test {
    use hex_literal::hex;

    use crate::section_header::SectionHeader;

    #[test]
    fn serialize_e2e() {
        let b = hex!(
            "07000000030000000000000000000000000000000000000080000000000000000A00000000000000000000000000000001000000000000000000000000000000"
        );
        let h = SectionHeader::from_bytes(&b).unwrap();
        assert_eq!(h.to_bytes(), &b)
    }
}
