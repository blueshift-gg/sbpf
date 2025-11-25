use {
    crate::errors::DisassemblerError,
    object::{Endianness, read::elf::ElfFile64},
    serde::{Deserialize, Serialize, Serializer},
    std::str,
};

pub const EI_MAGIC: [u8; 4] = *b"\x7fELF"; // ELF magic
pub const EI_CLASS: u8 = 0x02; // 64-bit
pub const EI_DATA: u8 = 0x01; // Little endian
pub const EI_VERSION: u8 = 0x01; // Version 1
pub const EI_OSABI: u8 = 0x00; // System V
pub const EI_ABIVERSION: u8 = 0x00; // No ABI version
pub const EI_PAD: [u8; 7] = [0u8; 7]; // Padding
pub const E_TYPE: u16 = 0x03; // ET_DYN - shared object
pub const E_MACHINE: u16 = 0xf7; // Berkeley Packet Filter
pub const E_MACHINE_SBPF: u16 = 0x0107; // Solana Berkeley Packet Filter
pub const E_VERSION: u32 = 0x01; // Original version of BPF

fn elf_magic<S>(magic: &[u8; 4], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let s = String::from_utf8_lossy(magic);
    serializer.serialize_str(&s)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ELFHeader {
    #[serde(serialize_with = "elf_magic")]
    pub ei_magic: [u8; 4],
    pub ei_class: u8,
    pub ei_data: u8,
    pub ei_version: u8,
    pub ei_osabi: u8,
    pub ei_abiversion: u8,
    pub ei_pad: [u8; 7],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: u64,
    pub e_phoff: u64,
    pub e_shoff: u64,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

impl ELFHeader {
    pub fn from_elf_file(elf_file: &ElfFile64<Endianness>) -> Result<Self, DisassemblerError> {
        let endian = elf_file.endian();
        let elf_header = elf_file.elf_header();

        // Extract ELF header fields.
        let e_ident = elf_header.e_ident;
        let e_type = elf_header.e_type.get(endian);
        let e_machine = elf_header.e_machine.get(endian);
        let e_version = elf_header.e_version.get(endian);
        let e_entry = elf_header.e_entry.get(endian);
        let e_phoff = elf_header.e_phoff.get(endian);
        let e_shoff = elf_header.e_shoff.get(endian);
        let e_flags = elf_header.e_flags.get(endian);
        let e_ehsize = elf_header.e_ehsize.get(endian);
        let e_phentsize = elf_header.e_phentsize.get(endian);
        let e_phnum = elf_header.e_phnum.get(endian);
        let e_shentsize = elf_header.e_shentsize.get(endian);
        let e_shnum = elf_header.e_shnum.get(endian);
        let e_shstrndx = elf_header.e_shstrndx.get(endian);

        // Validate ELF header fields.
        if e_ident.magic.ne(&EI_MAGIC)
            || e_ident.class.ne(&EI_CLASS)
            || e_ident.data.ne(&EI_DATA)
            || e_ident.version.ne(&EI_VERSION)
            || e_ident.os_abi.ne(&EI_OSABI)
            || e_ident.abi_version.ne(&EI_ABIVERSION)
            || e_ident.padding.ne(&EI_PAD)
            || (e_machine.ne(&E_MACHINE) && e_machine.ne(&E_MACHINE_SBPF))
            || e_version.ne(&E_VERSION)
        {
            return Err(DisassemblerError::NonStandardElfHeader);
        }

        Ok(ELFHeader {
            ei_magic: e_ident.magic,
            ei_class: e_ident.class,
            ei_data: e_ident.data,
            ei_version: e_ident.version,
            ei_osabi: e_ident.os_abi,
            ei_abiversion: e_ident.abi_version,
            ei_pad: e_ident.padding,
            e_type,
            e_machine,
            e_version,
            e_entry,
            e_phoff,
            e_shoff,
            e_flags,
            e_ehsize,
            e_phentsize,
            e_phnum,
            e_shentsize,
            e_shnum,
            e_shstrndx,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = self.ei_magic.to_vec();
        b.extend_from_slice(&[
            self.ei_class,
            self.ei_data,
            self.ei_version,
            self.ei_osabi,
            self.ei_abiversion,
        ]);
        b.extend_from_slice(&self.ei_pad);
        b.extend_from_slice(&self.e_type.to_le_bytes());
        b.extend_from_slice(&self.e_machine.to_le_bytes());
        b.extend_from_slice(&self.e_version.to_le_bytes());
        b.extend_from_slice(&self.e_entry.to_le_bytes());
        b.extend_from_slice(&self.e_phoff.to_le_bytes());
        b.extend_from_slice(&self.e_shoff.to_le_bytes());
        b.extend_from_slice(&self.e_flags.to_le_bytes());
        b.extend_from_slice(&self.e_ehsize.to_le_bytes());
        b.extend_from_slice(&self.e_phentsize.to_le_bytes());
        b.extend_from_slice(&self.e_phnum.to_le_bytes());
        b.extend_from_slice(&self.e_shentsize.to_le_bytes());
        b.extend_from_slice(&self.e_shnum.to_le_bytes());
        b.extend_from_slice(&self.e_shstrndx.to_le_bytes());
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use {
        crate::{
            elf_header::{
                E_MACHINE, E_MACHINE_SBPF, E_TYPE, E_VERSION, EI_ABIVERSION, EI_CLASS, EI_DATA,
                EI_MAGIC, EI_OSABI, EI_PAD, EI_VERSION,
            },
            program::Program,
        },
        hex_literal::hex,
    };

    #[test]
    fn test_elf_header() {
        let program = Program::from_bytes(&hex!("7F454C460201010000000000000000000300F700010000002001000000000000400000000000000028020000000000000000000040003800030040000600050001000000050000002001000000000000200100000000000020010000000000003000000000000000300000000000000000100000000000000100000004000000C001000000000000C001000000000000C0010000000000003C000000000000003C000000000000000010000000000000020000000600000050010000000000005001000000000000500100000000000070000000000000007000000000000000080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007912A000000000007911182900000000B7000000010000002D21010000000000B70000000000000095000000000000001E0000000000000004000000000000000600000000000000C0010000000000000B0000000000000018000000000000000500000000000000F0010000000000000A000000000000000C00000000000000160000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000120001002001000000000000300000000000000000656E747279706F696E7400002E74657874002E64796E737472002E64796E73796D002E64796E616D6963002E73687374727461620000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000010000000600000000000000200100000000000020010000000000003000000000000000000000000000000008000000000000000000000000000000170000000600000003000000000000005001000000000000500100000000000070000000000000000400000000000000080000000000000010000000000000000F0000000B0000000200000000000000C001000000000000C001000000000000300000000000000004000000010000000800000000000000180000000000000007000000030000000200000000000000F001000000000000F0010000000000000C00000000000000000000000000000001000000000000000000000000000000200000000300000000000000000000000000000000000000FC010000000000002A00000000000000000000000000000001000000000000000000000000000000")).unwrap();

        // Verify ELF header fields match expected constants.
        assert_eq!(program.elf_header.ei_magic, EI_MAGIC);
        assert_eq!(program.elf_header.ei_class, EI_CLASS);
        assert_eq!(program.elf_header.ei_data, EI_DATA);
        assert_eq!(program.elf_header.ei_version, EI_VERSION);
        assert_eq!(program.elf_header.ei_osabi, EI_OSABI);
        assert_eq!(program.elf_header.ei_abiversion, EI_ABIVERSION);
        assert_eq!(program.elf_header.ei_pad, EI_PAD);
        assert_eq!(program.elf_header.e_type, E_TYPE);
        assert!(
            program.elf_header.e_machine == E_MACHINE_SBPF
                || program.elf_header.e_machine == E_MACHINE
        );
        assert_eq!(program.elf_header.e_version, E_VERSION);
    }

    #[test]
    fn test_elf_header_to_bytes() {
        let header = ELFHeader {
            ei_magic: EI_MAGIC,
            ei_class: EI_CLASS,
            ei_data: EI_DATA,
            ei_version: EI_VERSION,
            ei_osabi: EI_OSABI,
            ei_abiversion: EI_ABIVERSION,
            ei_pad: EI_PAD,
            e_type: 0x03,
            e_machine: 0xf7,
            e_version: 0x01,
            e_entry: 0x120,
            e_phoff: 64,
            e_shoff: 552,
            e_flags: 0,
            e_ehsize: 64,
            e_phentsize: 56,
            e_phnum: 3,
            e_shentsize: 64,
            e_shnum: 6,
            e_shstrndx: 5,
        };

        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), 64);
        assert_eq!(&bytes[0..4], &EI_MAGIC);
        assert_eq!(bytes[4], EI_CLASS);
        assert_eq!(bytes[5], EI_DATA);
        assert_eq!(bytes[6], EI_VERSION);
        assert_eq!(bytes[7], EI_OSABI);
        assert_eq!(bytes[8], EI_ABIVERSION);
        assert_eq!(&bytes[9..16], &EI_PAD);
    }

    #[test]
    fn test_elf_header_validation_errors() {
        // Test invalid ELF headers

        // Invalid magic.
        let invalid_magic = hex!(
            "00454C460201010000000000000000000300F700010000000000000000000000400000000000000000000000000000000000000040003800000000000000000000"
        );
        let result = Program::from_bytes(&invalid_magic);
        assert!(result.is_err());

        // Invalid class (not 64-bit).
        let invalid_class = hex!(
            "7F454C460101010000000000000000000300F700010000000000000000000000400000000000000000000000000000000000000040003800000000000000000000"
        );
        let result = Program::from_bytes(&invalid_class);
        assert!(result.is_err());

        // Invalid endianness (big endian instead of little).
        let invalid_endian = hex!(
            "7F454C460202010000000000000000000300F700010000000000000000000000400000000000000000000000000000000000000040003800000000000000000000"
        );
        let result = Program::from_bytes(&invalid_endian);
        assert!(result.is_err());
    }
}
