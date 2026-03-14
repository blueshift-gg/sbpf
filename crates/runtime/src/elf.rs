use {
    crate::errors::{RuntimeError, RuntimeResult},
    either::Either,
    sbpf_common::{inst_param::Number, instruction::Instruction, opcode::Opcode},
    sbpf_disassembler::{program::Program, rodata::RodataSection},
    sbpf_vm::memory::Memory,
};

/// Parse an ELF binary and return instructions, rodata, and entrypoint.
pub fn load_elf(elf_bytes: &[u8]) -> RuntimeResult<(Vec<Instruction>, Vec<u8>, usize)> {
    let program = Program::from_bytes(elf_bytes)
        .map_err(|e| RuntimeError::ElfParseError(format!("{:?}", e)))?;
    let (mut instructions, rodata_section, entrypoint_idx) = program
        .to_ixs()
        .map_err(|e| RuntimeError::ElfParseError(format!("{:?}", e)))?;
    let entrypoint = entrypoint_idx.unwrap_or(0);

    let mut rodata = rodata_section
        .as_ref()
        .map(|s| s.data.clone())
        .unwrap_or_default();

    if let Some(ref section) = rodata_section {
        apply_relocations(&mut instructions, &mut rodata, section);
    }

    Ok((instructions, rodata, entrypoint))
}

/// Apply all relocations.
fn apply_relocations(instructions: &mut [Instruction], rodata: &mut [u8], section: &RodataSection) {
    let elf_base = section.base_address;
    let elf_end = elf_base + section.data.len() as u64;

    // 1. Relocate lddw immediates that reference rodata addresses.
    for ix in instructions.iter_mut() {
        if ix.opcode == Opcode::Lddw
            && let Some(Either::Right(Number::Int(imm))) = &ix.imm
        {
            let addr = *imm as u64;
            if addr >= elf_base && addr < elf_end {
                ix.imm = Some(Either::Right(Number::Int(
                    (Memory::RODATA_START + addr - elf_base) as i64,
                )));
            }
        }
    }

    // 2. Apply data relocations.
    for &offset in &section.data_relocations {
        if offset + 8 <= rodata.len() {
            let ptr = u64::from_le_bytes(rodata[offset..offset + 8].try_into().unwrap());
            if ptr >= elf_base && ptr < elf_end {
                let relocated = Memory::RODATA_START + (ptr - elf_base);
                rodata[offset..offset + 8].copy_from_slice(&relocated.to_le_bytes());
            }
        }
    }

    // 3. Apply text relocations.
    for &(offset, ix_idx) in &section.text_relocations {
        if offset + 8 <= rodata.len() {
            rodata[offset..offset + 8].copy_from_slice(&(ix_idx as u64).to_le_bytes());
        }
    }
}
