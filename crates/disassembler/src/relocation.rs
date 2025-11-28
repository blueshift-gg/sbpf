use {
    crate::errors::DisassemblerError,
    object::{Endianness, Object, ObjectSection, read::elf::ElfFile64},
    serde::{Deserialize, Serialize},
};

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum RelocationType {
    R_BPF_NONE = 0x00,        // No relocation
    R_BPF_64_64 = 0x01,       // Relocation of a ld_imm64 instruction
    R_BPF_64_RELATIVE = 0x08, // Relocation of a ldxdw instruction
    R_BPF_64_32 = 0x0a,       // Relocation of a call instruction
}

impl TryFrom<u32> for RelocationType {
    type Error = DisassemblerError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Ok(match value {
            0x00 => Self::R_BPF_NONE,
            0x01 => Self::R_BPF_64_64,
            0x08 => Self::R_BPF_64_RELATIVE,
            0x0a => Self::R_BPF_64_32,
            _ => return Err(DisassemblerError::InvalidDataLength),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relocation {
    pub offset: u64,
    pub rel_type: RelocationType,
    pub symbol_index: u32,
    pub symbol_name: Option<String>,
}

impl Relocation {
    /// Parse relocation entries from the provided ELF file
    pub fn from_elf_file(elf_file: &ElfFile64<Endianness>) -> Result<Vec<Self>, DisassemblerError> {
        // Find .rel.dyn section
        let rel_dyn_section = match elf_file.section_by_name(".rel.dyn") {
            Some(s) => s,
            None => return Ok(Vec::new()),
        };

        let rel_dyn_data = rel_dyn_section
            .data()
            .map_err(|_| DisassemblerError::InvalidDataLength)?;

        // Extract .dynsym and .dynstr data for symbol resolution.
        let dynsym_data = elf_file
            .section_by_name(".dynsym")
            .and_then(|s| s.data().ok());
        let dynstr_data = elf_file
            .section_by_name(".dynstr")
            .and_then(|s| s.data().ok());

        let mut relocations = Vec::new();

        // Parse relocation entries
        for chunk in rel_dyn_data.chunks_exact(16) {
            let offset = u64::from_le_bytes(chunk[0..8].try_into().unwrap());
            let rel_type_val = u32::from_le_bytes(chunk[8..12].try_into().unwrap());
            let rel_type = RelocationType::try_from(rel_type_val)
                .map_err(|_| DisassemblerError::InvalidDataLength)?;
            let symbol_index = u32::from_le_bytes(chunk[12..16].try_into().unwrap());

            // Resolve symbol name if this is a syscall relocation
            let symbol_name = if rel_type == RelocationType::R_BPF_64_32 {
                match (&dynsym_data, &dynstr_data) {
                    (Some(dynsym), Some(dynstr)) => {
                        resolve_symbol_name(dynsym, dynstr, symbol_index as usize).ok()
                    }
                    _ => None,
                }
            } else {
                None
            };

            relocations.push(Relocation {
                offset,
                rel_type,
                symbol_index,
                symbol_name,
            });
        }

        Ok(relocations)
    }

    /// Return this relocation's offset relative to the provided base offset
    pub fn relative_offset(&self, base_offset: u64) -> u64 {
        self.offset.saturating_sub(base_offset)
    }

    /// Check if this is a syscall relocation
    pub fn is_syscall(&self) -> bool {
        self.rel_type == RelocationType::R_BPF_64_32
    }
}

/// Resolve symbol name for the provided index using .dynsym and .dynstr data
fn resolve_symbol_name(
    dynsym_data: &[u8],
    dynstr_data: &[u8],
    symbol_index: usize,
) -> Result<String, DisassemblerError> {
    const DYNSYM_ENTRY_SIZE: usize = 24;

    // Calculate offset into .dynsym for this symbol.
    let symbol_entry_offset = symbol_index * DYNSYM_ENTRY_SIZE;
    if symbol_entry_offset + 4 > dynsym_data.len() {
        return Err(DisassemblerError::InvalidDataLength);
    }

    let dynstr_offset = u32::from_le_bytes(
        dynsym_data[symbol_entry_offset..symbol_entry_offset + 4]
            .try_into()
            .unwrap(),
    ) as usize;
    if dynstr_offset >= dynstr_data.len() {
        return Err(DisassemblerError::InvalidDynstrOffset);
    }

    // Read symbol name from .dynstr data.
    let end = dynstr_data[dynstr_offset..]
        .iter()
        .position(|&b| b == 0)
        .ok_or(DisassemblerError::InvalidDynstrOffset)?;

    String::from_utf8(dynstr_data[dynstr_offset..dynstr_offset + end].to_vec())
        .map_err(|_| DisassemblerError::InvalidUtf8InDynstr)
}

#[cfg(test)]
mod tests {
    use {super::*, hex_literal::hex, object::read::elf::ElfFile64};

    // Test program:
    // .globl entrypoint
    // entrypoint:
    //   lddw r1, 0x1
    //   lddw r2, 0x2
    //   call sol_log_64_
    //   call sol_log_compute_units_
    //   exit
    const TEST_PROGRAM: &[u8] = &hex!(
        "7F454C460201010000000000000000000300F70001000000E8000000000000004000000000000000A002000000000000000000004000380003004000070006000100000005000000E800000000000000E800000000000000E8000000000000003800000000000000380000000000000000100000000000000100000004000000C001000000000000C001000000000000C001000000000000B000000000000000B00000000000000000100000000000000200000006000000200100000000000020010000000000002001000000000000A000000000000000A0000000000000000800000000000000180100000100000000000000000000001802000002000000000000000000000085100000FFFFFFFF85100000FFFFFFFF95000000000000001E0000000000000004000000000000001100000000000000500200000000000012000000000000002000000000000000130000000000000010000000000000000600000000000000C0010000000000000B000000000000001800000000000000050000000000000020020000000000000A00000000000000300000000000000016000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000100000010000100E80000000000000000000000000000000C000000100000000000000000000000000000000000000018000000100000000000000000000000000000000000000000656E747279706F696E7400736F6C5F6C6F675F36345F00736F6C5F6C6F675F636F6D707574655F756E6974735F000008010000000000000A0000000200000010010000000000000A00000003000000002E74657874002E64796E616D6963002E64796E73796D002E64796E737472002E72656C2E64796E002E7300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000010000000600000000000000E800000000000000E80000000000000038000000000000000000000000000000040000000000000000000000000000000700000006000000030000000000000020010000000000002001000000000000A000000000000000040000000000000008000000000000001000000000000000100000000B0000000200000000000000C001000000000000C0010000000000006000000000000000040000000100000008000000000000001800000000000000180000000300000002000000000000002002000000000000200200000000000030000000000000000000000000000000010000000000000000000000000000002000000009000000020000000000000050020000000000005002000000000000200000000000000003000000000000000800000000000000100000000000000029000000030000000000000000000000000000000000000070020000000000002C00000000000000000000000000000001000000000000000000000000000000"
    );

    #[test]
    fn test_relocation_parsing() {
        let elf_file = ElfFile64::<Endianness>::parse(TEST_PROGRAM).expect("Failed to parse ELF");
        let relocations =
            Relocation::from_elf_file(&elf_file).expect("Failed to parse relocations");

        // Should have 2 relocations.
        assert_eq!(relocations.len(), 2, "Expected 2 relocations");

        // Both should be syscall relocations.
        assert!(relocations[0].is_syscall());
        assert!(relocations[1].is_syscall());

        // Verify symbol names are resolved.
        assert_eq!(relocations[0].symbol_name.as_deref(), Some("sol_log_64_"));
        assert_eq!(
            relocations[1].symbol_name.as_deref(),
            Some("sol_log_compute_units_")
        );

        // Verify symbol indices.
        // 0 -> null
        // 1 -> entrypoint
        // 2 -> sol_log_64_
        // 3 -> sol_log_compute_units_
        assert_eq!(relocations[0].symbol_index, 2);
        assert_eq!(relocations[1].symbol_index, 3);
    }

    #[test]
    fn test_relocation_relative_offset() {
        let elf_file = ElfFile64::<Endianness>::parse(TEST_PROGRAM).expect("Failed to parse ELF");
        let relocations =
            Relocation::from_elf_file(&elf_file).expect("Failed to parse relocations");

        // Get .text section base address from the ELF.
        let text_section = elf_file
            .section_by_name(".text")
            .expect("Failed to find .text section");
        let text_section_offset = text_section.address();

        // Test relative_offset calculation.
        let rel0_offset = relocations[0].relative_offset(text_section_offset);
        let rel1_offset = relocations[1].relative_offset(text_section_offset);

        // Verify relative offsets.
        // lddw r1, 0x1 -> 0x00
        // lddw r2, 0x2 -> 0x10
        // call sol_log_64_ -> 0x20
        // call sol_log_compute_units_ -> 0x28
        assert_eq!(rel0_offset, 0x20);
        assert_eq!(rel1_offset, 0x28);
    }

    #[test]
    fn test_no_relocations() {
        // Simple program with no relocations
        let test_program = &hex!(
            "7F454C460201010000000000000000000300F700010000002001000000000000400000000000000028020000000000000000000040003800030040000600050001000000050000002001000000000000200100000000000020010000000000003000000000000000300000000000000000100000000000000100000004000000C001000000000000C001000000000000C0010000000000003C000000000000003C000000000000000010000000000000020000000600000050010000000000005001000000000000500100000000000070000000000000007000000000000000080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007912A000000000007911182900000000B7000000010000002D21010000000000B70000000000000095000000000000001E0000000000000004000000000000000600000000000000C0010000000000000B0000000000000018000000000000000500000000000000F0010000000000000A000000000000000C00000000000000160000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000120001002001000000000000300000000000000000656E747279706F696E7400002E74657874002E64796E737472002E64796E73796D002E64796E616D6963002E73687374727461620000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000010000000600000000000000200100000000000020010000000000003000000000000000000000000000000008000000000000000000000000000000170000000600000003000000000000005001000000000000500100000000000070000000000000000400000000000000080000000000000010000000000000000F0000000B0000000200000000000000C001000000000000C001000000000000300000000000000004000000010000000800000000000000180000000000000007000000030000000200000000000000F001000000000000F0010000000000000C00000000000000000000000000000001000000000000000000000000000000200000000300000000000000000000000000000000000000FC010000000000002A00000000000000000000000000000001000000000000000000000000000000"
        );

        let elf_file = ElfFile64::<Endianness>::parse(test_program).expect("Failed to parse ELF");
        let relocations =
            Relocation::from_elf_file(&elf_file).expect("Failed to parse relocations");

        assert!(relocations.is_empty());
    }
}
