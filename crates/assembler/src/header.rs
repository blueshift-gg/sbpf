use crate::SbpfArch;

#[derive(Debug)]
pub struct ElfHeader {
    pub e_ident: [u8; 16], // ELF identification bytes = [127, 69, 76, 70, 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    pub e_type: u16,       // Object file type = 3 (ET_DYN)
    pub e_machine: u16,    // Machine architecture = 247 (BPF)
    pub e_version: u32,    // Object file version = 1
    pub e_entry: u64,      // Entry point address
    pub e_phoff: u64,      // Program header offset
    pub e_shoff: u64,      // Section header offset
    pub e_flags: u32,      // Processor-specific flags
    pub e_ehsize: u16,     // ELF header size = 64
    pub e_phentsize: u16,  // Size of program header entry = 56
    pub e_phnum: u16,      // Number of program header entries
    pub e_shentsize: u16,  // Size of section header entry = 64
    pub e_shnum: u16,      // Number of section header entries
    pub e_shstrndx: u16,   // Section name string table index
}

#[derive(Debug)]
pub struct ProgramHeader {
    pub p_type: u32,   // Type of segment
    pub p_flags: u32,  // Segment attributes
    pub p_offset: u64, // Offset in file
    pub p_vaddr: u64,  // Virtual address in memory
    pub p_paddr: u64,  // Physical address (reserved)
    pub p_filesz: u64, // Size of segment in file
    pub p_memsz: u64,  // Size of segment in memory
    pub p_align: u64,  // Alignment of segment
}

impl Default for ElfHeader {
    fn default() -> Self {
        Self {
            e_ident: Self::SOLANA_IDENT,
            e_type: Self::SOLANA_TYPE,
            e_machine: Self::SOLANA_MACHINE,
            e_version: Self::SOLANA_VERSION,
            e_entry: 0,
            e_phoff: Self::ELF64_HEADER_SIZE as u64,
            e_shoff: 0,
            e_flags: 0,
            e_ehsize: Self::ELF64_HEADER_SIZE,
            e_phentsize: Self::PROGRAM_HEADER_SIZE,
            e_phnum: 0,
            e_shentsize: Self::SECTION_HEADER_SIZE,
            e_shnum: 0,
            e_shstrndx: 0,
        }
    }
}

impl ElfHeader {
    const SOLANA_IDENT: [u8; 16] = [
        0x7f, 0x45, 0x4c, 0x46, // EI_MAG0..EI_MAG3 ("\x7FELF")
        0x02, // EI_CLASS (64-bit)
        0x01, // EI_DATA (little endian)
        0x01, // EI_VERSION
        0x00, // EI_OSABI
        0x00, // EI_ABIVERSION
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // EI_PAD
    ];
    const SOLANA_TYPE: u16 = 3; // ET_DYN
    const SOLANA_MACHINE: u16 = 247; // BPF
    const SOLANA_VERSION: u32 = 1; // EV_CURRENT
    const ELF64_HEADER_SIZE: u16 = 64;
    const PROGRAM_HEADER_SIZE: u16 = 56;
    const SECTION_HEADER_SIZE: u16 = 64;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn bytecode(&self) -> Vec<u8> {
        let mut bytecode = Vec::with_capacity(Self::ELF64_HEADER_SIZE as usize);

        // e_ident (16 bytes)
        bytecode.extend_from_slice(&self.e_ident);

        // Emit remaining fields in little-endian order
        bytecode.extend_from_slice(&self.e_type.to_le_bytes());
        bytecode.extend_from_slice(&self.e_machine.to_le_bytes());
        bytecode.extend_from_slice(&self.e_version.to_le_bytes());
        bytecode.extend_from_slice(&self.e_entry.to_le_bytes());
        bytecode.extend_from_slice(&self.e_phoff.to_le_bytes());
        bytecode.extend_from_slice(&self.e_shoff.to_le_bytes());
        bytecode.extend_from_slice(&self.e_flags.to_le_bytes());
        bytecode.extend_from_slice(&self.e_ehsize.to_le_bytes());
        bytecode.extend_from_slice(&self.e_phentsize.to_le_bytes());
        bytecode.extend_from_slice(&self.e_phnum.to_le_bytes());
        bytecode.extend_from_slice(&self.e_shentsize.to_le_bytes());
        bytecode.extend_from_slice(&self.e_shnum.to_le_bytes());
        bytecode.extend_from_slice(&self.e_shstrndx.to_le_bytes());

        bytecode
    }
}

#[rustfmt::skip]
impl ProgramHeader {

    const PT_LOAD: u32 = 1;      // Loadable segment
    const PT_DYNAMIC: u32 = 2;   // Dynamic linking information
    
    pub const PF_X: u32 = 1;         // Executable
    pub const PF_W: u32 = 2;         // Writable
    pub const PF_R: u32 = 4;         // Readable
    
    const PAGE_SIZE: u64 = 4096;          // Standard page size
    
    pub const V3_RODATA_VADDR: u64 = 0 << 32;
    pub const V3_BYTECODE_VADDR: u64 = 1 << 32;

    pub fn new_load(offset: u64, size: u64, executable: bool, arch: SbpfArch) -> Self {
        let (flags, vaddr, align) = match (arch, executable) {
            (SbpfArch::V0, true) => (Self::PF_R | Self::PF_X, offset, Self::PAGE_SIZE),
            (SbpfArch::V0, false) => (Self::PF_R, offset, Self::PAGE_SIZE),
            (SbpfArch::V3, true) => (Self::PF_X, Self::V3_BYTECODE_VADDR, 0),
            (SbpfArch::V3, false) => (Self::PF_R, Self::V3_RODATA_VADDR, 0),
        };

        ProgramHeader {
            p_type: Self::PT_LOAD,
            p_flags: flags,
            p_offset: offset,
            p_vaddr: vaddr,
            p_paddr: vaddr,
            p_filesz: size,
            p_memsz: size,
            p_align: align // p_align is ignored in v3
        }
    }

    pub fn new_dynamic(offset: u64, size: u64) -> Self {
        ProgramHeader {
            p_type: Self::PT_DYNAMIC,
            p_flags: Self::PF_R | Self::PF_W,
            p_offset: offset,
            p_vaddr: offset,
            p_paddr: offset,
            p_filesz: size,
            p_memsz: size,
            p_align: 8
        }
    }

    pub fn bytecode(&self) -> Vec<u8> {
        let mut bytecode = Vec::with_capacity(56); // Size of program header is 56 bytes
        
        bytecode.extend_from_slice(&self.p_type.to_le_bytes());
        bytecode.extend_from_slice(&self.p_flags.to_le_bytes());
        bytecode.extend_from_slice(&self.p_offset.to_le_bytes());
        bytecode.extend_from_slice(&self.p_vaddr.to_le_bytes());
        bytecode.extend_from_slice(&self.p_paddr.to_le_bytes());
        bytecode.extend_from_slice(&self.p_filesz.to_le_bytes());
        bytecode.extend_from_slice(&self.p_memsz.to_le_bytes());
        bytecode.extend_from_slice(&self.p_align.to_le_bytes());

        bytecode
    }
}
#[derive(Debug)]
pub struct SectionHeader {
    sh_name: u32,      // Section name (string table index)
    sh_type: u32,      // Section type
    sh_flags: u64,     // Section flags
    sh_addr: u64,      // Section virtual addr at execution
    sh_offset: u64,    // Section file offset
    sh_size: u64,      // Section size in bytes
    sh_link: u32,      // Link to another section
    sh_info: u32,      // Additional section info
    sh_addralign: u64, // Section alignment
    sh_entsize: u64,   // Entry size if section holds table
}

#[rustfmt::skip]
impl SectionHeader {
    // Section types
    pub const SHT_NULL: u32 = 0;          // Section header table entry unused
    pub const SHT_PROGBITS: u32 = 1;      // Program data
    pub const SHT_STRTAB: u32 = 3;        // String table
    pub const SHT_NOBITS: u32 = 8;        // Program space with no data (bss)
    pub const SHT_DYNAMIC: u32 = 6;       // Dynamic section
    pub const SHT_DYNSYM: u32 = 11;       // Dynamic symbol table
    pub const SHT_REL: u32 = 9;           // Relocation table
    
    // Section flags
    pub const SHF_WRITE: u64 = 0x1;       // Writable
    pub const SHF_ALLOC: u64 = 0x2;       // Occupies memory during execution
    pub const SHF_EXECINSTR: u64 = 0x4;   // Executable
    
    #[allow(clippy::too_many_arguments)]
    pub fn new(name_offset: u32, sh_type: u32, flags: u64, addr: u64, offset: u64, size: u64, link: u32, info: u32, addralign: u64, entsize: u64) -> Self {
        Self {
            sh_name: name_offset,
            sh_type,
            sh_flags: flags,
            sh_addr: addr,
            sh_offset: offset,
            sh_size: size,
            sh_link: link,
            sh_info: info,
            sh_addralign: addralign,
            sh_entsize: entsize,
        }
    }

    pub fn bytecode(&self) -> Vec<u8> {
        let mut bytecode = Vec::with_capacity(64); // Size of section header is 64 bytes
        
        bytecode.extend_from_slice(&self.sh_name.to_le_bytes());
        bytecode.extend_from_slice(&self.sh_type.to_le_bytes());
        bytecode.extend_from_slice(&self.sh_flags.to_le_bytes());
        bytecode.extend_from_slice(&self.sh_addr.to_le_bytes());
        bytecode.extend_from_slice(&self.sh_offset.to_le_bytes());
        bytecode.extend_from_slice(&self.sh_size.to_le_bytes());
        bytecode.extend_from_slice(&self.sh_link.to_le_bytes());
        bytecode.extend_from_slice(&self.sh_info.to_le_bytes());
        bytecode.extend_from_slice(&self.sh_addralign.to_le_bytes());
        bytecode.extend_from_slice(&self.sh_entsize.to_le_bytes());

        bytecode
    }
}
