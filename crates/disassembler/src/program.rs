use {
    crate::{
        elf_header::{E_MACHINE_SBPF, ELFHeader},
        errors::DisassemblerError,
        program_header::ProgramHeader,
        relocation::Relocation,
        section_header::SectionHeader,
        section_header_entry::SectionHeaderEntry,
    },
    either::Either,
    object::{Endianness, read::elf::ElfFile64},
    sbpf_common::{inst_param::Number, instruction::Instruction, opcode::Opcode},
    serde::{Deserialize, Serialize},
    std::collections::HashMap,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Program {
    pub elf_header: ELFHeader,
    pub program_headers: Vec<ProgramHeader>,
    pub section_headers: Vec<SectionHeader>,
    pub section_header_entries: Vec<SectionHeaderEntry>,
    pub relocations: Vec<Relocation>,
}

impl Program {
    pub fn from_bytes(b: &[u8]) -> Result<Self, DisassemblerError> {
        let elf_file = ElfFile64::<Endianness>::parse(b).map_err(|e| {
            eprintln!("ELF parse error: {}", e);
            DisassemblerError::NonStandardElfHeader
        })?;

        // Parse elf header.
        let elf_header = ELFHeader::from_elf_file(&elf_file)?;

        // Parse program headers.
        let program_headers = ProgramHeader::from_elf_file(&elf_file)?;

        // Parse section headers and section header entries.
        let (section_headers, section_header_entries) = SectionHeader::from_elf_file(&elf_file)?;

        // Parse relocations.
        let relocations = Relocation::from_elf_file(&elf_file)?;

        Ok(Self {
            elf_header,
            program_headers,
            section_headers,
            section_header_entries,
            relocations,
        })
    }

    pub fn to_ixs(self) -> Result<Vec<Instruction>, DisassemblerError> {
        // Find and populate instructions for the .text section
        let text_section = self
            .section_header_entries
            .iter()
            .find(|e| e.label.eq(".text\0"))
            .ok_or(DisassemblerError::MissingTextSection)?;
        let text_section_offset = text_section.offset as u64;

        // Build syscall map
        let syscall_map = self.build_syscall_map(text_section_offset);

        let data = &text_section.data;
        if !data.len().is_multiple_of(8) {
            return Err(DisassemblerError::InvalidDataLength);
        }
        let mut ixs: Vec<Instruction> = vec![];
        let mut pos = 0;

        let is_sbpf_v2 =
            self.elf_header.e_flags == 0x02 && self.elf_header.e_machine == E_MACHINE_SBPF;
        // Handle pre-processing

        while pos < data.len() {
            let remaining = &data[pos..];
            if remaining.len() < 8 {
                break;
            }

            // ugly v2 shit we need to fix goes here:
            let mut ix = if is_sbpf_v2 {
                Instruction::from_bytes_sbpf_v2(remaining)?
            } else {
                Instruction::from_bytes(remaining)?
            };

            // Handle syscall relocation
            if ix.opcode == Opcode::Call
                && let Some(Either::Right(Number::Int(-1))) = ix.imm
                && let Some(syscall_name) = syscall_map.get(&(pos as u64))
            {
                ix.imm = Some(Either::Left(syscall_name.clone()));
            }

            if ix.opcode == Opcode::Lddw {
                pos += 16;
            } else {
                pos += 8;
            }

            ixs.push(ix);
        }

        Ok(ixs)
    }

    /// Build a hashmap where:
    /// - key: relative position within .text section
    /// - value: syscall name (sol_log_64_, sol_log_, etc.)
    fn build_syscall_map(&self, text_section_offset: u64) -> HashMap<u64, String> {
        self.relocations
            .iter()
            .filter(|r| r.is_syscall())
            .filter_map(|r| {
                r.symbol_name.as_ref().map(|name| {
                    // Convert absolute offset to relative position within .text
                    let relative_pos = r.relative_offset(text_section_offset);
                    (relative_pos, name.clone())
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use {
        crate::{
            elf_header::{E_MACHINE_SBPF, ELFHeader},
            program::Program,
            section_header_entry::SectionHeaderEntry,
        },
        hex_literal::hex,
    };

    #[test]
    fn try_deserialize_program() {
        let program = Program::from_bytes(&hex!("7F454C460201010000000000000000000300F700010000002001000000000000400000000000000028020000000000000000000040003800030040000600050001000000050000002001000000000000200100000000000020010000000000003000000000000000300000000000000000100000000000000100000004000000C001000000000000C001000000000000C0010000000000003C000000000000003C000000000000000010000000000000020000000600000050010000000000005001000000000000500100000000000070000000000000007000000000000000080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007912A000000000007911182900000000B7000000010000002D21010000000000B70000000000000095000000000000001E0000000000000004000000000000000600000000000000C0010000000000000B0000000000000018000000000000000500000000000000F0010000000000000A000000000000000C00000000000000160000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000120001002001000000000000300000000000000000656E747279706F696E7400002E74657874002E64796E737472002E64796E73796D002E64796E616D6963002E73687374727461620000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000010000000600000000000000200100000000000020010000000000003000000000000000000000000000000008000000000000000000000000000000170000000600000003000000000000005001000000000000500100000000000070000000000000000400000000000000080000000000000010000000000000000F0000000B0000000200000000000000C001000000000000C001000000000000300000000000000004000000010000000800000000000000180000000000000007000000030000000200000000000000F001000000000000F0010000000000000C00000000000000000000000000000001000000000000000000000000000000200000000300000000000000000000000000000000000000FC010000000000002A00000000000000000000000000000001000000000000000000000000000000")).unwrap();
        println!("{:?}", program.section_header_entries);
    }

    #[test]
    fn test_to_ixs_invalid_data_length() {
        // Create program with .text section that has invalid length (not multiple of 8)
        let program = Program {
            elf_header: ELFHeader {
                ei_magic: [127, 69, 76, 70],
                ei_class: 2,
                ei_data: 1,
                ei_version: 1,
                ei_osabi: 0,
                ei_abiversion: 0,
                ei_pad: [0; 7],
                e_type: 0,
                e_machine: 0,
                e_version: 0,
                e_entry: 0,
                e_phoff: 0,
                e_shoff: 0,
                e_flags: 0,
                e_ehsize: 0,
                e_phentsize: 0,
                e_phnum: 0,
                e_shentsize: 0,
                e_shnum: 0,
                e_shstrndx: 0,
            },
            program_headers: vec![],
            section_headers: vec![],
            section_header_entries: vec![
                SectionHeaderEntry::new(".text\0".to_string(), 0, vec![0x95, 0x00, 0x00]).unwrap(), // Only 3 bytes
            ],
        };

        let result = program.to_ixs();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::errors::DisassemblerError::InvalidDataLength
        ));
    }

    #[test]
    fn test_to_ixs_with_lddw() {
        // Test with 16 bytes lddw instruction

        let mut lddw_bytes = vec![0x18, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        lddw_bytes.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        lddw_bytes.extend_from_slice(&[0x95, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]); // exit

        let program = Program {
            elf_header: ELFHeader {
                ei_magic: [127, 69, 76, 70],
                ei_class: 2,
                ei_data: 1,
                ei_version: 1,
                ei_osabi: 0,
                ei_abiversion: 0,
                ei_pad: [0; 7],
                e_type: 0,
                e_machine: E_MACHINE_SBPF,
                e_version: 0,
                e_entry: 0,
                e_phoff: 0,
                e_shoff: 0,
                e_flags: 0,
                e_ehsize: 0,
                e_phentsize: 0,
                e_phnum: 0,
                e_shentsize: 0,
                e_shnum: 0,
                e_shstrndx: 0,
            },
            program_headers: vec![],
            section_headers: vec![],
            section_header_entries: vec![
                SectionHeaderEntry::new(".text\0".to_string(), 0, lddw_bytes).unwrap(),
            ],
        };

        let ixs = program.to_ixs().unwrap();
        assert_eq!(ixs.len(), 2); // lddw + exit
        assert_eq!(ixs[0].opcode, sbpf_common::opcode::Opcode::Lddw);
    }

    #[test]
    fn test_to_ixs_sbpf_v2() {
        // Use a v2 opcode (0x8C -> ldxw in v2)
        let v2_bytes = vec![0x8c, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

        let program = Program {
            elf_header: ELFHeader {
                ei_magic: [127, 69, 76, 70],
                ei_class: 2,
                ei_data: 1,
                ei_version: 1,
                ei_osabi: 0,
                ei_abiversion: 0,
                ei_pad: [0; 7],
                e_type: 0,
                e_machine: E_MACHINE_SBPF,
                e_version: 0,
                e_entry: 0,
                e_phoff: 0,
                e_shoff: 0,
                e_flags: 0x02, // SBPF v2 flag
                e_ehsize: 0,
                e_phentsize: 0,
                e_phnum: 0,
                e_shentsize: 0,
                e_shnum: 0,
                e_shstrndx: 0,
            },
            program_headers: vec![],
            section_headers: vec![],
            section_header_entries: vec![
                SectionHeaderEntry::new(".text\0".to_string(), 0, v2_bytes).unwrap(),
            ],
        };

        let ixs = program.to_ixs().unwrap();
        assert_eq!(ixs.len(), 1);
        assert_eq!(ixs[0].opcode, sbpf_common::opcode::Opcode::Ldxw);
    }
}
