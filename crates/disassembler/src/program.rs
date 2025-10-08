use std::io::Cursor;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{
    cursor::ELFCursor, elf_header::ELFHeader, errors::EZBpfError, program_header::ProgramHeader,
    section_header::SectionHeader, section_header_entry::SectionHeaderEntry,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Program {
    pub elf_header: ELFHeader,
    pub program_headers: Vec<ProgramHeader>,
    pub section_headers: Vec<SectionHeader>,
    pub section_header_entries: Vec<SectionHeaderEntry>,
}

impl Program {
    pub fn from_bytes(b: &[u8]) -> Result<Self, EZBpfError> {
        let mut c = Cursor::new(b);
        let elf_header = c.read_elf_header()?;

        c.set_position(elf_header.e_phoff);
        let program_headers = (0..elf_header.e_phnum)
            .map(|_| c.read_program_header())
            .collect::<Result<Vec<_>, _>>()?;
        c.set_position(elf_header.e_shoff);
        let section_headers = (0..elf_header.e_shnum)
            .map(|_| c.read_section_header())
            .collect::<Result<Vec<_>, _>>()?;

        let shstrndx = &section_headers[elf_header.e_shstrndx as usize];
        let shstrndx_value = b
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
                let data =
                    b[s.sh_offset as usize..s.sh_offset as usize + s.sh_size as usize].to_vec();

                SectionHeaderEntry::new(label, s.sh_offset as usize, data)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            elf_header,
            program_headers,
            section_headers,
            section_header_entries,
        })
    }
}

#[cfg(test)]
mod tests {
    use hex_literal::hex;

    use crate::program::Program;

    #[test]
    fn try_deserialize_program() {
        let program = Program::from_bytes(&hex!("7F454C460201010000000000000000000300F700010000002001000000000000400000000000000028020000000000000000000040003800030040000600050001000000050000002001000000000000200100000000000020010000000000003000000000000000300000000000000000100000000000000100000004000000C001000000000000C001000000000000C0010000000000003C000000000000003C000000000000000010000000000000020000000600000050010000000000005001000000000000500100000000000070000000000000007000000000000000080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007912A000000000007911182900000000B7000000010000002D21010000000000B70000000000000095000000000000001E0000000000000004000000000000000600000000000000C0010000000000000B0000000000000018000000000000000500000000000000F0010000000000000A000000000000000C00000000000000160000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000120001002001000000000000300000000000000000656E747279706F696E7400002E74657874002E64796E737472002E64796E73796D002E64796E616D6963002E73687374727461620000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000010000000600000000000000200100000000000020010000000000003000000000000000000000000000000008000000000000000000000000000000170000000600000003000000000000005001000000000000500100000000000070000000000000000400000000000000080000000000000010000000000000000F0000000B0000000200000000000000C001000000000000C001000000000000300000000000000004000000010000000800000000000000180000000000000007000000030000000200000000000000F001000000000000F0010000000000000C00000000000000000000000000000001000000000000000000000000000000200000000300000000000000000000000000000000000000FC010000000000002A00000000000000000000000000000001000000000000000000000000000000")).unwrap();
        println!("{:?}", program.section_header_entries);
    }
}
