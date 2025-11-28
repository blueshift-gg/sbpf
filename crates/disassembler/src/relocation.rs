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
