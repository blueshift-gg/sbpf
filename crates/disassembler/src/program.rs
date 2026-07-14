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
    sbpf_common::{
        errors::SBPFError, inst_param::Number, instruction::Instruction, opcode::Opcode,
    },
    serde::{Deserialize, Serialize},
    std::collections::{BTreeSet, HashMap},
};

/// Outcome of an error-tolerant operation, the value `T` plus every error found while producing it.
#[derive(Debug)]
#[must_use]
pub struct Parsed<T> {
    pub value: T,
    pub errors: Vec<DisassemblerError>,
}

impl<T> Parsed<T> {
    /// Collapse to strict semantics where any error becomes failure.
    /// used in places where the errors are unrecoverable
    pub fn into_strict(self) -> Result<T, Vec<DisassemblerError>> {
        if self.errors.is_empty() {
            Ok(self.value)
        } else {
            Err(self.errors)
        }
    }
}

#[derive(Debug)]
pub struct Disassembly {
    pub instructions: Vec<Either<Instruction, DisassemblerError>>,
    pub rodata: Option<RodataSection>,
    pub entrypoint: Option<usize>,
}

pub type DisassembleResult = Result<Parsed<Disassembly>, Vec<DisassemblerError>>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Program {
    pub elf_header: ELFHeader,
    pub program_headers: Vec<ProgramHeader>,
    pub section_headers: Vec<SectionHeader>,
    pub section_header_entries: Vec<SectionHeaderEntry>,
    pub relocations: Vec<Relocation>,
}

impl Program {
    pub fn from_bytes(b: &[u8]) -> Result<Self, Vec<DisassemblerError>> {
        let mut errors = Vec::new();

        let elf_file = match ElfFile64::<Endianness>::parse(b) {
            Ok(elf_file) => elf_file,
            Err(source) => {
                errors.push(DisassemblerError::InvalidElfFile {
                    first_bytes: b.get(..16).unwrap_or(b).to_vec(),
                    source,
                });
                // Nothing to parse headers from.
                return Err(errors);
            }
        };

        // Parse elf header.
        let elf_header = ELFHeader::from_elf_file(&elf_file)?;

        // Parse program headers.
        let program_headers = ProgramHeader::from_elf_file(&elf_file)?;

        // Parse section headers and section header entries.
        let (section_headers, section_header_entries) = SectionHeader::from_elf_file(&elf_file)?;

        // Parse relocations.
        let relocations = Relocation::from_elf_file(&elf_file)?;

        // v3 binaries omit the section header table; reconstruct the .text and
        // .rodata section views from the program (segment) headers so the rest
        // of the disassembler can locate them by name.
        let (section_headers, section_header_entries) = if section_header_entries.is_empty() {
            Self::synthesize_sections_from_segments(b, &program_headers).map_err(|e| vec![e])?
        } else {
            (section_headers, section_header_entries)
        };

        Ok(Self {
            elf_header,
            program_headers,
            section_headers,
            section_header_entries,
            relocations,
        })
    }

    /// Reconstruct `.text` and `.rodata` section views from loadable program
    /// segments. Used for v3 binaries, which carry no section header table:
    /// the executable segment becomes `.text` and the read-only,
    /// non-executable segment becomes `.rodata`. The synthesized section
    /// headers mirror what the assembler used to emit (`sh_addr == sh_offset ==
    /// file offset`) so downstream offset resolution is unchanged.
    fn synthesize_sections_from_segments(
        data: &[u8],
        program_headers: &[ProgramHeader],
    ) -> Result<(Vec<SectionHeader>, Vec<SectionHeaderEntry>), DisassemblerError> {
        use crate::{
            program_header::{PF_X, ProgramType},
            section_header::SectionHeaderType,
        };

        let segment_bytes = |offset: u64, size: u64| -> Vec<u8> {
            let start = offset as usize;
            let end = start.saturating_add(size as usize).min(data.len());
            if start >= data.len() {
                Vec::new()
            } else {
                data[start..end].to_vec()
            }
        };

        let make_header = |offset: u64, size: u64, executable: bool| SectionHeader {
            sh_name: 0,
            sh_type: SectionHeaderType::SHT_PROGBITS,
            sh_flags: if executable { 0x6 } else { 0x2 }, // SHF_ALLOC (+ SHF_EXECINSTR)
            sh_addr: offset,
            sh_offset: offset,
            sh_size: size,
            sh_link: 0,
            sh_info: 0,
            sh_addralign: if executable { 8 } else { 1 },
            sh_entsize: 0,
        };

        let mut headers = Vec::new();
        let mut entries = Vec::new();

        let is_load = |ph: &&ProgramHeader| matches!(ph.p_type, ProgramType::PT_LOAD);
        let is_exec = |ph: &ProgramHeader| ph.p_flags.0 & PF_X as u32 == PF_X as u32;

        // .rodata: read-only, non-executable loadable segment (if present).
        if let Some(ph) = program_headers
            .iter()
            .filter(is_load)
            .find(|ph| !is_exec(ph))
        {
            headers.push(make_header(ph.p_offset, ph.p_filesz, false));
            entries.push(SectionHeaderEntry::new(
                ".rodata\0".to_string(),
                ph.p_offset as usize,
                segment_bytes(ph.p_offset, ph.p_filesz),
            )?);
        }

        // .text: executable loadable segment.
        if let Some(ph) = program_headers
            .iter()
            .filter(is_load)
            .find(|ph| is_exec(ph))
        {
            headers.push(make_header(ph.p_offset, ph.p_filesz, true));
            entries.push(SectionHeaderEntry::new(
                ".text\0".to_string(),
                ph.p_offset as usize,
                segment_bytes(ph.p_offset, ph.p_filesz),
            )?);
        }

        Ok((headers, entries))
    }

    pub fn to_ixs(self) -> DisassembleResult {
        self.into_ixs_inner(true)
    }

    pub fn to_ixs_raw(self) -> DisassembleResult {
        self.into_ixs_inner(false)
    }

    fn into_ixs_inner(self, resolve_offsets: bool) -> DisassembleResult {
        // Find and populate instructions for the .text section
        let text_section = self
            .section_header_entries
            .iter()
            .find(|e| e.label.eq(".text\0"))
            .ok_or_else(|| {
                vec![DisassemblerError::MissingTextSection {
                    sections: self
                        .section_header_entries
                        .iter()
                        .map(|e| e.label.trim_end_matches('\0').to_string())
                        .collect(),
                }]
            })?;

        let mut errors = Vec::new();

        let text_section_offset = text_section.offset as u64;

        // Build syscall map
        let syscall_map = self.build_syscall_map(text_section_offset);

        let data = &text_section.data;
        if !data.len().is_multiple_of(8) {
            errors.push(DisassemblerError::InvalidDataLength(data.len()));
        }

        let is_sbpf_v2 =
            self.elf_header.e_flags == 0x02 && self.elf_header.e_machine == E_MACHINE_SBPF;
        let is_sbpf_v3 = self.elf_header.e_flags == 0x03 && self.elf_header.e_machine == E_MACHINE;

        // Get rodata info
        let rodata_info = self.get_rodata_info();
        let (rodata_base, rodata_end) = rodata_info
            .as_ref()
            .map(|(d, addr)| (*addr, *addr + d.len() as u64))
            .unwrap_or((0, 0));

        // Parse instructions and build slot mappings
        let mut ixs: Vec<Either<Instruction, DisassemblerError>> = Vec::new();
        let mut idx_to_slot: Vec<usize> = Vec::new();
        let mut pos: usize = 0;
        let mut slot: usize = 0;

        while pos < data.len() {
            let remaining = &data[pos..];
            if remaining.len() < 8 {
                break;
            }

            // lddw (0x18) is the only 16-byte instruction; sbpf-common's
            // decoder asserts on shorter input, so report the truncation
            // here instead of panicking.
            let decoded = if remaining.len() < 16 && remaining[0] == 0x18 {
                Err(SBPFError::BytecodeError {
                    error: format!("lddw needs 16 bytes but only {} remain", remaining.len()),
                    span: 0..8,
                    custom_label: None,
                })
            } else if is_sbpf_v2 {
                // ugly v2 shit we need to fix goes here:
                Instruction::from_bytes_sbpf_v2(remaining)
            } else if is_sbpf_v3 {
                Instruction::from_bytes_sbpf_v3(remaining)
            } else {
                Instruction::from_bytes(remaining)
            };

            let mut ix = match decoded {
                Ok(ix) => ix,
                // A word that fails to decode doesn't affect the rest of the
                // stream, instead we record the error and keep it inline in the stream,
                // where it occupies a slot to keep jump/call targets truthful.
                Err(e) => {
                    // Decode spans are relative to the instruction slice;
                    // rebase them to the instruction's offset within .text.
                    let e = match e {
                        SBPFError::BytecodeError { error, span, .. } => {
                            DisassemblerError::BytecodeError {
                                error,
                                span: span.start + pos..span.end + pos,
                            }
                        }
                    };
                    errors.push(e.clone());
                    idx_to_slot.push(slot);
                    ixs.push(Either::Right(e));
                    pos += 8;
                    slot += 1;
                    continue;
                }
            };

            // Handle syscall relocation
            if ix.opcode == Opcode::Call
                && let Some(Either::Right(Number::Int(-1))) = ix.imm
                && let Some(syscall_name) = syscall_map.get(&(pos as u64))
            {
                ix.imm = Some(Either::Left(syscall_name.clone()));
            }

            idx_to_slot.push(slot);

            if ix.opcode == Opcode::Lddw {
                pos += 16;
                slot += 2;
            } else {
                pos += 8;
                slot += 1;
            }

            ixs.push(Either::Left(ix));
        }

        let mut slot_to_idx = vec![0usize; slot];
        for (idx, &slot) in idx_to_slot.iter().enumerate() {
            slot_to_idx[slot] = idx;
        }

        let text_sh_addr = self
            .section_headers
            .iter()
            .find(|h| {
                self.section_header_entries
                    .iter()
                    .any(|e| e.label.eq(".text\0") && e.offset == h.sh_offset as usize)
            })
            .map(|h| h.sh_addr)
            .unwrap_or(0);
        let text_end_addr = text_sh_addr + text_section.data.len() as u64;

        let mut rodata_refs = BTreeSet::new();

        if resolve_offsets {
            // Resolve jump/call labels and collect rodata references

            for (idx, ix) in ixs.iter_mut().enumerate() {
                let Either::Left(ix) = ix else { continue };
                let is_lddw = ix.opcode == Opcode::Lddw;

                // Resolve jump targets
                if ix.is_jump()
                    && let Some(Either::Right(off)) = &ix.off
                {
                    let current_slot = idx_to_slot[idx] as i64;
                    let target_slot = current_slot + 1 + (*off as i64);
                    if target_slot >= 0
                        && let Some(&target_idx) = slot_to_idx.get(target_slot as usize)
                    {
                        let new_off = target_idx as i64 - (idx as i64 + 1);
                        ix.off = Some(Either::Right(new_off as i16));
                    }
                }

                // Resolve internal call targets
                if ix.opcode == Opcode::Call
                    && let Some(Either::Right(Number::Int(imm))) = &ix.imm
                {
                    let current_slot = idx_to_slot[idx] as i64;
                    let target_slot = current_slot + 1 + *imm;
                    if target_slot >= 0
                        && let Some(&target_idx) = slot_to_idx.get(target_slot as usize)
                    {
                        let new_rel = target_idx as i64 - (idx as i64 + 1);
                        ix.imm = Some(Either::Right(Number::Int(new_rel)));
                    }
                }

                // Collect rodata references
                if is_lddw && let Some(Either::Right(Number::Int(imm))) = &ix.imm {
                    let addr = *imm as u64;
                    if rodata_info.is_some() && addr >= rodata_base && addr < rodata_end {
                        rodata_refs.insert(addr);
                    } else if addr >= text_sh_addr && addr < text_end_addr {
                        // Convert text address to instruction index for callx.
                        let byte_offset = addr - text_sh_addr;
                        let target_slot = (byte_offset / 8) as usize;
                        if target_slot < slot_to_idx.len() {
                            let ix_idx = slot_to_idx[target_slot];
                            ix.imm = Some(Either::Right(Number::Int(ix_idx as i64)));
                        }
                    }
                }
            }
        }

        // Parse rodata section
        let rodata = if let Some((data, base_addr)) = rodata_info {
            let mut section = RodataSection::parse(data, base_addr, &rodata_refs);
            let (data_relocs, text_relocs) = self.classify_relocations(
                &section.data,
                base_addr,
                text_section_offset,
                text_section.data.len() as u64,
                text_sh_addr,
                &slot_to_idx,
            );
            section.data_relocations = data_relocs;
            section.text_relocations = text_relocs;
            Some(section)
        } else {
            None
        };

        // Calculate entrypoint instruction index from byte offset.
        let entrypoint_idx = self.get_entrypoint_offset().map(|byte_offset| {
            let entrypoint_slot = (byte_offset / 8) as usize;
            if entrypoint_slot < slot_to_idx.len() {
                slot_to_idx[entrypoint_slot]
            } else {
                0
            }
        });

        Ok(Parsed {
            value: Disassembly {
                instructions: ixs,
                rodata,
                entrypoint: entrypoint_idx,
            },
            errors,
        })
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

        // Check for .data.rel.ro section and combine if present.
        if let Some(data_rel_ro_entry) = self
            .section_header_entries
            .iter()
            .find(|e| e.label.starts_with(".data.rel.ro"))
        {
            let data_rel_ro_header = self
                .section_headers
                .iter()
                .find(|h| h.sh_offset as usize == data_rel_ro_entry.offset);

            if let Some(drr_header) = data_rel_ro_header {
                let drr_end = drr_header.sh_addr + drr_header.sh_size;
                let total_size = (drr_end - vaddr) as usize;

                // Allocate combined buffer.
                let mut combined = vec![0u8; total_size];

                // Copy .rodata at offset 0.
                let rodata_len = rodata_entry.data.len().min(total_size);
                combined[..rodata_len].copy_from_slice(&rodata_entry.data[..rodata_len]);

                // Copy .data.rel.ro at its offset relative to rodata base.
                let drr_offset = (drr_header.sh_addr - vaddr) as usize;
                let drr_len = data_rel_ro_entry.data.len().min(total_size - drr_offset);
                combined[drr_offset..drr_offset + drr_len]
                    .copy_from_slice(&data_rel_ro_entry.data[..drr_len]);

                return Some((combined, vaddr));
            }
        }

        Some((rodata_entry.data.clone(), vaddr))
    }

    /// Classify relocations into data and text relocations.
    fn classify_relocations(
        &self,
        rodata_data: &[u8],
        rodata_base: u64,
        text_offset: u64,
        text_len: u64,
        text_sh_addr: u64,
        slot_to_idx: &[usize],
    ) -> (Vec<usize>, Vec<(usize, usize)>) {
        let rodata_len = rodata_data.len();
        let text_end_addr = text_sh_addr + text_len;
        let mut data_relocs = Vec::new();
        let mut text_relocs = Vec::new();

        for r in &self.relocations {
            if r.rel_type != crate::relocation::RelocationType::R_BPF_64_RELATIVE {
                continue;
            }
            if r.offset >= text_offset && r.offset < text_offset + text_len {
                continue;
            }
            if r.offset < rodata_base || r.offset + 8 > rodata_base + rodata_len as u64 {
                continue;
            }
            let offset_in_blob = (r.offset - rodata_base) as usize;
            data_relocs.push(offset_in_blob);

            let imm_offset = offset_in_blob + 4;
            if imm_offset + 4 <= rodata_len {
                let ptr =
                    u32::from_le_bytes(rodata_data[imm_offset..imm_offset + 4].try_into().unwrap())
                        as u64;
                if ptr >= text_sh_addr && ptr < text_end_addr {
                    let target_slot = ((ptr - text_sh_addr) / 8) as usize;
                    if target_slot < slot_to_idx.len() {
                        text_relocs.push((offset_in_blob, slot_to_idx[target_slot]));
                    }
                }
            }
        }

        (data_relocs, text_relocs)
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
            elf_header::{E_MACHINE, E_MACHINE_SBPF, EI_OSABI_LINUX, ELFHeader},
            program::Program,
            section_header_entry::SectionHeaderEntry,
        },
        hex_literal::hex,
    };

    #[test]
    fn try_deserialize_program() {
        let mut bytes = hex!("7F454C460201010000000000000000000300F700010000002001000000000000400000000000000028020000000000000000000040003800030040000600050001000000050000002001000000000000200100000000000020010000000000003000000000000000300000000000000000100000000000000100000004000000C001000000000000C001000000000000C0010000000000003C000000000000003C000000000000000010000000000000020000000600000050010000000000005001000000000000500100000000000070000000000000007000000000000000080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007912A000000000007911182900000000B7000000010000002D21010000000000B70000000000000095000000000000001E0000000000000004000000000000000600000000000000C0010000000000000B0000000000000018000000000000000500000000000000F0010000000000000A000000000000000C00000000000000160000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000120001002001000000000000300000000000000000656E747279706F696E7400002E74657874002E64796E737472002E64796E73796D002E64796E616D6963002E73687374727461620000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000010000000600000000000000200100000000000020010000000000003000000000000000000000000000000008000000000000000000000000000000170000000600000003000000000000005001000000000000500100000000000070000000000000000400000000000000080000000000000010000000000000000F0000000B0000000200000000000000C001000000000000C001000000000000300000000000000004000000010000000800000000000000180000000000000007000000030000000200000000000000F001000000000000F0010000000000000C00000000000000000000000000000001000000000000000000000000000000200000000300000000000000000000000000000000000000FC010000000000002A00000000000000000000000000000001000000000000000000000000000000").to_vec();
        let program = Program::from_bytes(&bytes).unwrap();
        println!("{:?}", program.section_header_entries);

        bytes[7] = EI_OSABI_LINUX;
        let program = Program::from_bytes(&bytes).unwrap();
        assert_eq!(program.elf_header.ei_osabi, EI_OSABI_LINUX);

        // Corrupt e_machine (LE u16 at bytes 18..20): parsing fails and the
        // error carries the field name, the accepted values and the value
        // found.
        bytes[18] = 0x00;
        let errors = Program::from_bytes(&bytes).unwrap_err();
        match errors.as_slice() {
            [
                crate::errors::DisassemblerError::NonStandardElfHeader {
                    field,
                    expected,
                    found,
                },
            ] => {
                assert_eq!(*field, "machine");
                assert_eq!(*expected, vec![E_MACHINE as u64, E_MACHINE_SBPF as u64]);
                assert_eq!(*found, 0);
            }
            other => panic!("expected NonStandardElfHeader for machine, got {other:?}"),
        }
    }

    #[test]
    fn test_from_bytes_reports_all_header_errors() {
        let mut bytes = hex!("7F454C460201010000000000000000000300F700010000002001000000000000400000000000000028020000000000000000000040003800030040000600050001000000050000002001000000000000200100000000000020010000000000003000000000000000300000000000000000100000000000000100000004000000C001000000000000C001000000000000C0010000000000003C000000000000003C000000000000000010000000000000020000000600000050010000000000005001000000000000500100000000000070000000000000007000000000000000080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007912A000000000007911182900000000B7000000010000002D21010000000000B70000000000000095000000000000001E0000000000000004000000000000000600000000000000C0010000000000000B0000000000000018000000000000000500000000000000F0010000000000000A000000000000000C00000000000000160000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000120001002001000000000000300000000000000000656E747279706F696E7400002E74657874002E64796E737472002E64796E73796D002E64796E616D6963002E73687374727461620000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000010000000600000000000000200100000000000020010000000000003000000000000000000000000000000008000000000000000000000000000000170000000600000003000000000000005001000000000000500100000000000070000000000000000400000000000000080000000000000010000000000000000F0000000B0000000200000000000000C001000000000000C001000000000000300000000000000004000000010000000800000000000000180000000000000007000000030000000200000000000000F001000000000000F0010000000000000C00000000000000000000000000000001000000000000000000000000000000200000000300000000000000000000000000000000000000FC010000000000002A00000000000000000000000000000001000000000000000000000000000000").to_vec();

        // Corrupt two independent header fields: os abi (byte 7) and
        // e_version (LE u32 at bytes 20..24). One Err reports both, not
        // just the first.
        bytes[7] = 0x05;
        bytes[20] = 0x02;

        let errors = Program::from_bytes(&bytes).unwrap_err();
        let fields: Vec<&str> = errors
            .iter()
            .map(|e| match e {
                crate::errors::DisassemblerError::NonStandardElfHeader { field, .. } => *field,
                other => panic!("expected NonStandardElfHeader, got {other:?}"),
            })
            .collect();
        assert_eq!(fields, vec!["os abi", "version"]);
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

        let parsed = program.to_ixs().unwrap();
        assert!(parsed.value.instructions.is_empty());
        assert!(matches!(
            parsed.errors.as_slice(),
            [crate::errors::DisassemblerError::InvalidDataLength(3)]
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

        let parsed = program.to_ixs().unwrap();
        assert!(parsed.errors.is_empty());
        let ixs = parsed.value.instructions;
        assert_eq!(ixs.len(), 2); // lddw + exit
        assert_eq!(
            ixs[0].as_ref().unwrap_left().opcode,
            sbpf_common::opcode::Opcode::Lddw
        );
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

        let parsed = program.to_ixs().unwrap();
        assert!(parsed.errors.is_empty());
        let ixs = parsed.value.instructions;
        assert_eq!(ixs.len(), 1);
        assert_eq!(
            ixs[0].as_ref().unwrap_left().opcode,
            sbpf_common::opcode::Opcode::Ldxw
        );
    }

    #[test]
    fn test_to_ixs_sbpf_v3() {
        let v3_bytes = vec![0x46, 0x01, 0x00, 0x00, 0x7f, 0x00, 0x00, 0x00];

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
                e_machine: E_MACHINE,
                e_version: 0,
                e_entry: 0,
                e_phoff: 0,
                e_shoff: 0,
                e_flags: 0x03, // SBPF v3 flag
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
                SectionHeaderEntry::new(".text\0".to_string(), 0, v3_bytes).unwrap(),
            ],
            relocations: vec![],
        };

        let parsed = program.to_ixs().unwrap();
        assert!(parsed.errors.is_empty());
        let ixs = parsed.value.instructions;
        assert_eq!(ixs.len(), 1);
        assert_eq!(
            ixs[0].as_ref().unwrap_left().opcode,
            sbpf_common::opcode::Opcode::Jset32Imm
        );
    }

    #[test]
    fn test_to_ixs_skips_undecodable_instruction() {
        // .text: [8 bytes of garbage][exit]. The garbage word is reported
        // and kept inline in the instruction stream; decoding resumes at
        // the next 8-byte boundary.
        let mut text = vec![0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        text.extend_from_slice(&[0x95, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);

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
                SectionHeaderEntry::new(".text\0".to_string(), 0, text).unwrap(),
            ],
            relocations: vec![],
        };

        let parsed = program.to_ixs().unwrap();
        assert_eq!(parsed.value.instructions.len(), 2);
        match &parsed.value.instructions[0] {
            either::Either::Right(crate::errors::DisassemblerError::BytecodeError {
                span, ..
            }) => assert_eq!(span.start, 0),
            other => panic!("expected inline BytecodeError, got {other:?}"),
        }
        assert_eq!(
            parsed.value.instructions[1].as_ref().unwrap_left().opcode,
            sbpf_common::opcode::Opcode::Exit
        );
        match parsed.errors.as_slice() {
            [crate::errors::DisassemblerError::BytecodeError { span, .. }] => {
                assert_eq!(span.start, 0);
            }
            other => panic!("expected one BytecodeError, got {other:?}"),
        }
    }

    #[test]
    fn test_to_ixs_reports_truncated_lddw() {
        // .text: [exit][lddw first half with no second half]. The truncated
        // lddw is reported and skipped instead of panicking in the decoder.
        let mut text = vec![0x95, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        text.extend_from_slice(&[0x18, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00]);

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
                SectionHeaderEntry::new(".text\0".to_string(), 0, text).unwrap(),
            ],
            relocations: vec![],
        };

        let parsed = program.to_ixs().unwrap();
        assert_eq!(parsed.value.instructions.len(), 2);
        assert_eq!(
            parsed.value.instructions[0].as_ref().unwrap_left().opcode,
            sbpf_common::opcode::Opcode::Exit
        );
        assert!(parsed.value.instructions[1].is_right());
        match parsed.errors.as_slice() {
            [crate::errors::DisassemblerError::BytecodeError { error, span }] => {
                assert_eq!(*span, (8..16));
                assert_eq!(error, "lddw needs 16 bytes but only 8 remain");
            }
            other => panic!("expected one BytecodeError, got {other:?}"),
        }
    }

    #[test]
    fn test_to_ixs_decodes_words_before_trailing_bytes() {
        // .text: exit + 3 trailing bytes (11 total). The length is reported
        // but the full 8-byte word still decodes.
        let text = vec![
            0x95, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03,
        ];

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
                SectionHeaderEntry::new(".text\0".to_string(), 0, text).unwrap(),
            ],
            relocations: vec![],
        };

        let parsed = program.to_ixs().unwrap();
        assert_eq!(parsed.value.instructions.len(), 1);
        assert!(matches!(
            parsed.errors.as_slice(),
            [crate::errors::DisassemblerError::InvalidDataLength(11)]
        ));
    }
}
