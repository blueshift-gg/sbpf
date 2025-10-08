use std::io::Cursor;
use std::str;

use serde::{Deserialize, Serialize, Serializer};

use crate::{cursor::ELFCursor, errors::EZBpfError};

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
    pub fn from_bytes(b: &[u8]) -> Result<Self, EZBpfError> {
        let mut c = Cursor::new(b);
        c.read_elf_header()
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
    use hex_literal::hex;

    use super::ELFHeader;

    #[test]
    fn serialize_e2e() {
        let b = hex!(
            "7F454C460201010000000000000000000300F7000100000078000000000000004000000000000000900000000000000000000000400038000100400003000200"
        );
        let h = ELFHeader::from_bytes(&b).unwrap();
        assert_eq!(h.to_bytes(), &b)
    }

    #[test]
    fn serialize_sbpf_machine_e2e() {
        let b = hex!(
            "7F454C46020101000000000000000000030007010100000020010000000000004000000000000000680200000000000000000000400038000300400006000500"
        );
        let h = ELFHeader::from_bytes(&b).unwrap();
        assert_eq!(h.to_bytes(), &b)
    }
}
