use {
    crate::{
        elf_header::{E_MACHINE, E_MACHINE_SBPF, ELFHeader},
        errors::DisassemblerError,
        program_header::ProgramHeader,
        relocation::Relocation,
        rodata::RodataSection,
        section_header::SectionHeader,
        section_header_entry::SectionHeaderEntry,
    },
    either::Either,
    object::{Endianness, read::elf::ElfFile64},
    sbpf_common::{inst_param::Number, instruction::Instruction, opcode::Opcode},
    serde::{Deserialize, Serialize},
    std::collections::{BTreeSet, HashMap},
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

    pub fn to_ixs(self) -> Result<(Vec<Instruction>, Option<RodataSection>), DisassemblerError> {
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

        let is_sbpf_v2 =
            self.elf_header.e_flags == 0x02 && self.elf_header.e_machine == E_MACHINE_SBPF;

        // Get rodata info
        let rodata_info = self.get_rodata_info();
        let (rodata_base, rodata_end) = rodata_info
            .as_ref()
            .map(|(d, addr)| (*addr, *addr + d.len() as u64))
            .unwrap_or((0, 0));

        // Parse instructions and build slot/position mappings
        let mut ixs: Vec<Instruction> = Vec::new();
        let mut slot_to_position: Vec<u64> = Vec::new();
        let mut idx_to_slot: Vec<usize> = Vec::new();
        let mut pos: usize = 0;
        let mut slot: usize = 0;

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

            slot_to_position.push(pos as u64);
            idx_to_slot.push(slot);

            if ix.opcode == Opcode::Lddw {
                slot_to_position.push(pos as u64 + 8);
                pos += 16;
                slot += 2;
            } else {
                pos += 8;
                slot += 1;
            }

            ixs.push(ix);
        }

        // Resolve jump/call labels and collect rodata references
        let mut rodata_refs = BTreeSet::new();

        for (idx, ix) in ixs.iter_mut().enumerate() {
            let is_lddw = ix.opcode == Opcode::Lddw;

            // Resolve jump targets
            if ix.is_jump()
                && let Some(Either::Right(off)) = &ix.off
            {
                let current_slot = idx_to_slot[idx];
                let target_slot = (current_slot as i64 + 1 + (*off as i64)) as usize;
                if let Some(&target_pos) = slot_to_position.get(target_slot) {
                    ix.off = Some(Either::Left(format!("jmp_{:04x}", target_pos)));
                }
            }

            // Resolve internal call targets
            if ix.opcode == Opcode::Call
                && let Some(Either::Right(Number::Int(imm))) = &ix.imm
            {
                let current_slot = idx_to_slot[idx] as i64;
                let target_slot = current_slot + 1 + *imm;
                if target_slot >= 0
                    && let Some(&target_pos) = slot_to_position.get(target_slot as usize)
                {
                    ix.imm = Some(Either::Left(format!("fn_{:04x}", target_pos)));
                }
            }

            // Collect rodata references
            if is_lddw
                && rodata_info.is_some()
                && let Some(Either::Right(Number::Int(imm))) = &ix.imm
            {
                let addr = *imm as u64;
                if addr >= rodata_base && addr < rodata_end {
                    rodata_refs.insert(addr);
                }
            }
        }

        // Parse rodata and replace addresses with labels
        let rodata = if let Some((data, base_addr)) = rodata_info {
            let rodata = RodataSection::parse(data, base_addr, &rodata_refs);

            for ix in &mut ixs {
                if ix.opcode == Opcode::Lddw
                    && let Some(Either::Right(Number::Int(imm))) = &ix.imm
                {
                    let addr = *imm as u64;
                    if let Some(label) = rodata.get_label(addr) {
                        ix.imm = Some(Either::Left(label.to_string()));
                    }
                }
            }

            Some(rodata)
        } else {
            None
        };

        Ok((ixs, rodata))
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

    /// Get the raw rodata bytes and the virtual address where it's loaded in memory
    fn get_rodata_info(&self) -> Option<(Vec<u8>, u64)> {
        let rodata_entry = self
            .section_header_entries
            .iter()
            .find(|e| e.label.starts_with(".rodata"))?;

        // v3: use program header p_vaddr
        // v0: use section header sh_addr
        let vaddr = if self.is_v3() {
            self.program_headers
                .iter()
                .find(|ph| {
                    let rodata_offset = rodata_entry.offset as u64;
                    rodata_offset >= ph.p_offset && rodata_offset < ph.p_offset + ph.p_filesz
                })
                .map(|ph| ph.p_vaddr)
                .unwrap_or(0)
        } else {
            let rodata_header = self
                .section_headers
                .iter()
                .find(|h| h.sh_offset as usize == rodata_entry.offset)?;
            rodata_header.sh_addr
        };

        Some((rodata_entry.data.clone(), vaddr))
    }

    /// Get the entrypoint offset
    pub fn get_entrypoint_offset(&self) -> Option<u64> {
        let e_entry = self.elf_header.e_entry;

        if self.is_v3() {
            const V3_BYTECODE_VADDR: u64 = 1 << 32;
            if e_entry >= V3_BYTECODE_VADDR {
                Some(e_entry - V3_BYTECODE_VADDR)
            } else {
                None
            }
        } else {
            let text_header = self.section_headers.iter().find(|h| {
                self.section_header_entries
                    .iter()
                    .any(|e| e.label.eq(".text\0") && e.offset == h.sh_offset as usize)
            })?;
            let text_sh_addr = text_header.sh_addr;

            if e_entry >= text_sh_addr {
                Some(e_entry - text_sh_addr)
            } else {
                None
            }
        }
    }

    fn is_v3(&self) -> bool {
        self.elf_header.e_flags == 0x03 && self.elf_header.e_machine == E_MACHINE
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
            relocations: vec![],
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
            relocations: vec![],
        };

        let (ixs, _) = program.to_ixs().unwrap();
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
            relocations: vec![],
        };

        let (ixs, _) = program.to_ixs().unwrap();
        assert_eq!(ixs.len(), 1);
        assert_eq!(ixs[0].opcode, sbpf_common::opcode::Opcode::Ldxw);
    }
}
