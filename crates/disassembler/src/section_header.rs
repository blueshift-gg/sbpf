use std::{fmt::Debug, fmt::Display};

use object::Endianness;
use object::read::elf::ElfFile64;
use serde::{Deserialize, Serialize};

use crate::errors::EZBpfError;
use crate::section_header_entry::SectionHeaderEntry;

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
    pub fn from_elf_file(
        elf_file: &ElfFile64<Endianness>,
    ) -> Result<(Vec<Self>, Vec<SectionHeaderEntry>), EZBpfError> {
        let endian = elf_file.endian();
        let section_headers_data: Vec<_> = elf_file.elf_section_table().iter().collect();

        let mut section_headers = Vec::new();
        for sh in section_headers_data.iter() {
            let sh_name = sh.sh_name.get(endian);
            let sh_type = SectionHeaderType::try_from(sh.sh_type.get(endian))?;
            let sh_flags = sh.sh_flags.get(endian);
            let sh_addr = sh.sh_addr.get(endian);
            let sh_offset = sh.sh_offset.get(endian);
            let sh_size = sh.sh_size.get(endian);
            let sh_link = sh.sh_link.get(endian);
            let sh_info = sh.sh_info.get(endian);
            let sh_addralign = sh.sh_addralign.get(endian);
            let sh_entsize = sh.sh_entsize.get(endian);

            section_headers.push(SectionHeader {
                sh_name,
                sh_type,
                sh_flags,
                sh_addr,
                sh_offset,
                sh_size,
                sh_link,
                sh_info,
                sh_addralign,
                sh_entsize,
            });
        }

        let elf_header = elf_file.elf_header();
        let e_shstrndx = elf_header.e_shstrndx.get(endian);
        let shstrndx = &section_headers[e_shstrndx as usize];
        let shstrndx_value = elf_file.data()
            [shstrndx.sh_offset as usize..shstrndx.sh_offset as usize + shstrndx.sh_size as usize]
            .to_vec();

        let mut indices: Vec<u32> = section_headers.iter().map(|h| h.sh_name).collect();
        indices.push(shstrndx.sh_size as u32);
        indices.sort_unstable();

        let section_header_entries = section_headers
            .iter()
            .map(|s| {
                let current_offset = s.sh_name as usize;
                let next_index = indices.binary_search(&s.sh_name).unwrap() + 1;
                let next_offset =
                    *indices.get(next_index).ok_or(EZBpfError::InvalidString)? as usize;

                let label = String::from_utf8(shstrndx_value[current_offset..next_offset].to_vec())
                    .unwrap_or("default".to_string());

                let data = elf_file.data()
                    [s.sh_offset as usize..s.sh_offset as usize + s.sh_size as usize]
                    .to_vec();

                SectionHeaderEntry::new(label, s.sh_offset as usize, data)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok((section_headers, section_header_entries))
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

#[cfg(test)]
mod tests {
    use hex_literal::hex;

    use crate::program::Program;

    #[test]
    fn test_section_headers() {
        let program = Program::from_bytes(&hex!("7F454C460201010000000000000000000300F700010000002001000000000000400000000000000028020000000000000000000040003800030040000600050001000000050000002001000000000000200100000000000020010000000000003000000000000000300000000000000000100000000000000100000004000000C001000000000000C001000000000000C0010000000000003C000000000000003C000000000000000010000000000000020000000600000050010000000000005001000000000000500100000000000070000000000000007000000000000000080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007912A000000000007911182900000000B7000000010000002D21010000000000B70000000000000095000000000000001E0000000000000004000000000000000600000000000000C0010000000000000B0000000000000018000000000000000500000000000000F0010000000000000A000000000000000C00000000000000160000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000120001002001000000000000300000000000000000656E747279706F696E7400002E74657874002E64796E737472002E64796E73796D002E64796E616D6963002E73687374727461620000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000010000000600000000000000200100000000000020010000000000003000000000000000000000000000000008000000000000000000000000000000170000000600000003000000000000005001000000000000500100000000000070000000000000000400000000000000080000000000000010000000000000000F0000000B0000000200000000000000C001000000000000C001000000000000300000000000000004000000010000000800000000000000180000000000000007000000030000000200000000000000F001000000000000F0010000000000000C00000000000000000000000000000001000000000000000000000000000000200000000300000000000000000000000000000000000000FC010000000000002A00000000000000000000000000000001000000000000000000000000000000")).unwrap();

        // Verify we have the expected number of section headers.
        assert_eq!(program.section_headers.len(), 6);
        assert_eq!(program.section_header_entries.len(), 6);

        // Verify section header entries have proper data.
        for entry in &program.section_header_entries {
            assert!(!entry.label.is_empty());
            if entry.label == ".text\0" {
                assert!(!entry.data.is_empty());
                assert!(!entry.ixs.is_empty());
            }
        }
    }
}
