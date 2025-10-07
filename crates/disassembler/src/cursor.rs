use std::io::{Cursor, Read, Seek, SeekFrom};

use crate::{
    elf_header::{
        E_MACHINE, E_MACHINE_SBPF, E_TYPE, E_VERSION, EI_ABIVERSION, EI_CLASS, EI_DATA, EI_MAGIC,
        EI_OSABI, EI_PAD, EI_VERSION, ELFHeader,
    },
    errors::EZBpfError,
    instructions::Ix,
    opcodes::OpCode,
    program_header::{ProgramFlags, ProgramHeader, ProgramType},
    section_header::{SectionHeader, SectionHeaderType},
};

pub trait ELFCursor {
    fn read_elf_header(&mut self) -> Result<ELFHeader, EZBpfError>;
    fn read_program_header(&mut self) -> Result<ProgramHeader, EZBpfError>;
    fn read_section_header(&mut self) -> Result<SectionHeader, EZBpfError>;
    fn read_ix(&mut self) -> Result<Ix, EZBpfError>;
    fn read_lddw_imm(&mut self) -> Result<i64, EZBpfError>;
    fn read_u8(&mut self) -> Result<u8, EZBpfError>;
    fn read_i16(&mut self) -> Result<i16, EZBpfError>;
    fn read_u16(&mut self) -> Result<u16, EZBpfError>;
    fn read_i32(&mut self) -> Result<i32, EZBpfError>;
    fn read_u32(&mut self) -> Result<u32, EZBpfError>;
    fn read_u64(&mut self) -> Result<u64, EZBpfError>;
    fn read_bytes(&mut self, l: usize) -> Result<Vec<u8>, EZBpfError>;
    fn remainder(&mut self) -> u64;
}

impl ELFCursor for Cursor<&[u8]> {
    fn read_elf_header(&mut self) -> Result<ELFHeader, EZBpfError> {
        let ei_magic = self.read_u32()?.to_le_bytes();
        let ei_class = self.read_u8()?;
        let ei_data = self.read_u8()?;
        let ei_version = self.read_u8()?;
        let ei_osabi = self.read_u8()?;
        let ei_abiversion = self.read_u8()?;
        let mut ei_pad = [0u8; 7];
        ei_pad.clone_from_slice(&self.read_bytes(7)?);
        let e_type = self.read_u16()?;
        let e_machine = self.read_u16()?;
        let e_version = self.read_u32()?;
        if ei_magic.ne(&EI_MAGIC)
            || ei_class.ne(&EI_CLASS)
            || ei_data.ne(&EI_DATA)
            || ei_version.ne(&EI_VERSION)
            || ei_osabi.ne(&EI_OSABI)
            || ei_abiversion.ne(&EI_ABIVERSION)
            || ei_pad.ne(&EI_PAD)
            || e_type.ne(&E_TYPE)
            || (e_machine.ne(&E_MACHINE) && e_machine.ne(&E_MACHINE_SBPF))
            || e_version.ne(&E_VERSION)
        {
            return Err(EZBpfError::NonStandardElfHeader);
        }

        let e_entry = self.read_u64()?;
        let e_phoff = self.read_u64()?;
        let e_shoff = self.read_u64()?;
        let e_flags = self.read_u32()?;
        let e_ehsize = self.read_u16()?;
        let e_phentsize = self.read_u16()?;
        let e_phnum = self.read_u16()?;
        let e_shentsize = self.read_u16()?;
        let e_shnum = self.read_u16()?;
        let e_shstrndx = self.read_u16()?;
        Ok(ELFHeader {
            ei_magic,
            ei_class,
            ei_data,
            ei_version,
            ei_osabi,
            ei_abiversion,
            ei_pad,
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

    fn read_program_header(&mut self) -> Result<ProgramHeader, EZBpfError> {
        let p_type = ProgramType::try_from(self.read_u32()?)?;
        let p_flags = ProgramFlags::from(self.read_u32()?);
        let p_offset = self.read_u64()?;
        let p_vaddr = self.read_u64()?;
        let p_paddr = self.read_u64()?;
        let p_filesz = self.read_u64()?;
        let p_memsz = self.read_u64()?;
        let p_align = self.read_u64()?;
        Ok(ProgramHeader {
            p_type,
            p_flags,
            p_offset,
            p_vaddr,
            p_paddr,
            p_filesz,
            p_memsz,
            p_align,
        })
    }

    fn read_section_header(&mut self) -> Result<SectionHeader, EZBpfError> {
        let sh_name = self.read_u32()?;
        let sh_type = SectionHeaderType::try_from(self.read_u32()?)?;
        let sh_flags = self.read_u64()?;
        let sh_addr = self.read_u64()?;
        let sh_offset = self.read_u64()?;
        let sh_size = self.read_u64()?;
        let sh_link = self.read_u32()?;
        let sh_info = self.read_u32()?;
        let sh_addralign = self.read_u64()?;
        let sh_entsize = self.read_u64()?;
        Ok(SectionHeader {
            sh_name,
            sh_type,
            sh_flags,
            sh_addr,
            sh_offset,
            sh_size,
            sh_link,
            sh_info,
            sh_addralign,
            sh_entsize,
        })
    }

    fn read_u8(&mut self) -> Result<u8, EZBpfError> {
        let mut b = [0u8];
        self.read_exact(&mut b)
            .map_err(|_| EZBpfError::CursorError)?;
        Ok(b[0])
    }

    fn read_u16(&mut self) -> Result<u16, EZBpfError> {
        let mut b = [0u8; 2];
        self.read_exact(&mut b)
            .map_err(|_| EZBpfError::CursorError)?;
        Ok(u16::from_le_bytes(b))
    }

    fn read_i16(&mut self) -> Result<i16, EZBpfError> {
        let mut b = [0u8; 2];
        self.read_exact(&mut b)
            .map_err(|_| EZBpfError::CursorError)?;
        Ok(i16::from_le_bytes(b))
    }

    fn read_u32(&mut self) -> Result<u32, EZBpfError> {
        let mut b = [0u8; 4];
        self.read_exact(&mut b)
            .map_err(|_| EZBpfError::CursorError)?;
        Ok(u32::from_le_bytes(b))
    }

    fn read_i32(&mut self) -> Result<i32, EZBpfError> {
        let mut b = [0u8; 4];
        self.read_exact(&mut b)
            .map_err(|_| EZBpfError::CursorError)?;
        Ok(i32::from_le_bytes(b))
    }

    fn read_u64(&mut self) -> Result<u64, EZBpfError> {
        let mut b = [0u8; 8];
        self.read_exact(&mut b)
            .map_err(|_| EZBpfError::CursorError)?;
        Ok(u64::from_le_bytes(b))
    }

    fn read_lddw_imm(&mut self) -> Result<i64, EZBpfError> {
        let mut b = [0u8; 8];
        b[0..4].clone_from_slice(&self.read_bytes(4)?);
        if self.read_u32()? != 0 {
            return Err(EZBpfError::InvalidImmediate);
        }
        b[4..8].clone_from_slice(&self.read_bytes(4)?);
        Ok(i64::from_le_bytes(b))
    }

    fn read_ix(&mut self) -> Result<Ix, EZBpfError> {
        let op = OpCode::try_from(self.read_u8()?)?;
        let reg = self.read_u8()?;
        let src = reg >> 4;
        let dst = reg & 0x0f;
        let off = self.read_i16()?;
        let imm = match op {
            OpCode::Lddw => self.read_lddw_imm()?,
            _ => self.read_i32()? as i64,
        };
        Ok(Ix {
            op,
            src,
            dst,
            off,
            imm,
        })
    }

    fn read_bytes(&mut self, l: usize) -> Result<Vec<u8>, EZBpfError> {
        let mut v = vec![0_u8; l];
        self.read_exact(&mut v)
            .map_err(|_| EZBpfError::CursorError)?;
        Ok(v)
    }

    fn remainder(&mut self) -> u64 {
        let pos = self.position();
        let end = self.seek(SeekFrom::End(0)).unwrap();
        self.seek(SeekFrom::Start(pos)).unwrap();
        end - pos
    }
}
