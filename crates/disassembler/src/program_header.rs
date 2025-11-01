use {
    crate::errors::DisassemblerError,
    object::{Endianness, read::elf::ElfFile64},
    serde::{Deserialize, Serialize},
    std::fmt::Debug,
};

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
    type Error = DisassemblerError;

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
            _ => return Err(DisassemblerError::InvalidProgramType),
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
    pub fn from_elf_file(elf_file: &ElfFile64<Endianness>) -> Result<Vec<Self>, DisassemblerError> {
        let endian = elf_file.endian();
        let program_headers_data = elf_file.elf_program_headers();

        let mut program_headers = Vec::new();
        for ph in program_headers_data {
            let p_type = ProgramType::try_from(ph.p_type.get(endian))?;
            let p_flags = ProgramFlags::from(ph.p_flags.get(endian));
            let p_offset = ph.p_offset.get(endian);
            let p_vaddr = ph.p_vaddr.get(endian);
            let p_paddr = ph.p_paddr.get(endian);
            let p_filesz = ph.p_filesz.get(endian);
            let p_memsz = ph.p_memsz.get(endian);
            let p_align = ph.p_align.get(endian);

            program_headers.push(ProgramHeader {
                p_type,
                p_flags,
                p_offset,
                p_vaddr,
                p_paddr,
                p_filesz,
                p_memsz,
                p_align,
            });
        }

        Ok(program_headers)
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
    use {crate::program::Program, hex_literal::hex};
    use sbpf_common::platform::SbpfV0;

    #[test]
    fn test_program_headers() {
        let original_bytes = hex!(
            "7F454C460201010000000000000000000300F700010000002001000000000000400000000000000028020000000000000000000040003800030040000600050001000000050000002001000000000000200100000000000020010000000000003000000000000000300000000000000000100000000000000100000004000000C001000000000000C001000000000000C0010000000000003C000000000000003C000000000000000010000000000000020000000600000050010000000000005001000000000000500100000000000070000000000000007000000000000000080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007912A000000000007911182900000000B7000000010000002D21010000000000B70000000000000095000000000000001E0000000000000004000000000000000600000000000000C0010000000000000B0000000000000018000000000000000500000000000000F0010000000000000A000000000000000C00000000000000160000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000120001002001000000000000300000000000000000656E747279706F696E7400002E74657874002E64796E737472002E64796E73796D002E64796E616D6963002E73687374727461620000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000010000000600000000000000200100000000000020010000000000003000000000000000000000000000000008000000000000000000000000000000170000000600000003000000000000005001000000000000500100000000000070000000000000000400000000000000080000000000000010000000000000000F0000000B0000000200000000000000C001000000000000C001000000000000300000000000000004000000010000000800000000000000180000000000000007000000030000000200000000000000F001000000000000F0010000000000000C00000000000000000000000000000001000000000000000000000000000000200000000300000000000000000000000000000000000000FC010000000000002A00000000000000000000000000000001000000000000000000000000000000"
        );
        let program = Program::from_bytes::<SbpfV0>(&original_bytes).unwrap();

        // Verify we have the expected number of program headers.
        assert_eq!(program.program_headers.len(), 3);

        // Verify that serialized program headers match the original ELF data.
        for (i, program_header) in program.program_headers.iter().enumerate() {
            let serialized = program_header.to_bytes();
            let header_offset = 0x40 + (i * 56);
            let original_header_bytes = &original_bytes[header_offset..header_offset + 56];
            assert_eq!(serialized, original_header_bytes,);
        }
    }
}
