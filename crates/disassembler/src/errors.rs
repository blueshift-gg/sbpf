use {
    sbpf_common::errors::SBPFError,
    std::{ops::Range, string::FromUtf8Error},
    thiserror::Error,
};

/// Render the acceptable values for an ELF header field, e.g. `0xf7 or 0x107`.
fn hex_list(values: &[u64]) -> String {
    values
        .iter()
        .map(|v| format!("{v:#x}"))
        .collect::<Vec<_>>()
        .join(" or ")
}

#[derive(Debug, Error, Clone)]
pub enum DisassemblerError {
    #[error("Failed to parse ELF file: {source}; first bytes: {first_bytes:02x?}")]
    InvalidElfFile {
        first_bytes: Vec<u8>,
        #[source]
        source: object::read::Error,
    },
    #[error("Non-standard ELF header: {field} is {found:#x}, expected {}", hex_list(.expected))]
    NonStandardElfHeader {
        field: &'static str,
        expected: Vec<u64>,
        found: u64,
    },
    #[error("Invalid Program Type: {0:#x}")]
    InvalidProgramType(u32),
    #[error("Invalid Section Header Type: {0:#x}")]
    InvalidSectionHeaderType(u32),
    #[error("Invalid Relocation Type: {0:#x}")]
    InvalidRelocationType(u32),
    #[error("Failed to read {section} section data: {source}")]
    SectionDataError {
        section: &'static str,
        #[source]
        source: object::read::Error,
    },
    #[error("Invalid data length: {0}")]
    InvalidDataLength(usize),
    #[error("Bytecode error at bytes {span:?}: {error}")]
    BytecodeError { error: String, span: Range<usize> },
    #[error("Missing text section; sections present: {sections:?}")]
    MissingTextSection { sections: Vec<String> },
    #[error("Invalid offset in .dynstr section: offset {offset}, data length {data_len}")]
    InvalidDynstrOffset { offset: usize, data_len: usize },
    #[error("Non-UTF8 data in .dynstr section: {0}")]
    InvalidUtf8InDynstr(FromUtf8Error),
    #[error("Invalid section header string table index: {shstrndx}, section header count: {shnum}")]
    InvalidShstrndx { shstrndx: u16, shnum: usize },
    #[error(
        "Invalid section name offset: sh_name {sh_name:#x} exceeds string table length \
         {strtab_len:#x}"
    )]
    InvalidSectionName { sh_name: u32, strtab_len: usize },
    #[error(
        "Section {section} data out of bounds: offset {offset:#x} + size {size:#x} exceeds file \
         length {file_len:#x}"
    )]
    SectionDataOutOfBounds {
        section: String,
        offset: u64,
        size: u64,
        file_len: usize,
    },
}

impl From<SBPFError> for DisassemblerError {
    fn from(err: SBPFError) -> Self {
        match err {
            SBPFError::BytecodeError { error, span, .. } => {
                DisassemblerError::BytecodeError { error, span }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, object::read::elf::ElfFile64};

    #[test]
    fn test_from_sbpf_error() {
        let sbpf_error = SBPFError::BytecodeError {
            error: "test error".to_string(),
            span: 8..16,
            custom_label: None,
        };
        let disasm_error: DisassemblerError = sbpf_error.into();
        assert!(matches!(
            disasm_error,
            DisassemblerError::BytecodeError { error, span }
                if error == "test error" && span == (8..16)
        ));
    }

    #[test]
    fn test_error_display() {
        let empty: &[u8] = &[];
        let parse_error = ElfFile64::<object::Endianness>::parse(empty).unwrap_err();
        let msg = DisassemblerError::InvalidElfFile {
            first_bytes: vec![0x7f, 0x45],
            source: parse_error,
        }
        .to_string();
        assert!(msg.starts_with("Failed to parse ELF file: "));
        assert!(msg.ends_with("; first bytes: [7f, 45]"));

        assert_eq!(
            DisassemblerError::NonStandardElfHeader {
                field: "machine",
                expected: vec![0xf7, 0x107],
                found: 0x108,
            }
            .to_string(),
            "Non-standard ELF header: machine is 0x108, expected 0xf7 or 0x107"
        );
        assert_eq!(
            DisassemblerError::InvalidProgramType(0x6474e550).to_string(),
            "Invalid Program Type: 0x6474e550"
        );
        assert_eq!(
            DisassemblerError::InvalidSectionHeaderType(0x6ffffff6).to_string(),
            "Invalid Section Header Type: 0x6ffffff6"
        );
        assert_eq!(
            DisassemblerError::InvalidRelocationType(0x2a).to_string(),
            "Invalid Relocation Type: 0x2a"
        );
        let section_error = ElfFile64::<object::Endianness>::parse(empty).unwrap_err();
        assert!(
            DisassemblerError::SectionDataError {
                section: ".rel.dyn",
                source: section_error,
            }
            .to_string()
            .starts_with("Failed to read .rel.dyn section data: ")
        );
        assert_eq!(
            DisassemblerError::InvalidDataLength(13).to_string(),
            "Invalid data length: 13"
        );
        assert_eq!(
            DisassemblerError::BytecodeError {
                error: "custom".to_string(),
                span: 8..16,
            }
            .to_string(),
            "Bytecode error at bytes 8..16: custom"
        );
        assert_eq!(
            DisassemblerError::MissingTextSection {
                sections: vec![".rodata".to_string(), ".shstrtab".to_string()],
            }
            .to_string(),
            "Missing text section; sections present: [\".rodata\", \".shstrtab\"]"
        );
        assert_eq!(
            DisassemblerError::InvalidDynstrOffset {
                offset: 128,
                data_len: 64,
            }
            .to_string(),
            "Invalid offset in .dynstr section: offset 128, data length 64"
        );

        let utf8_error = String::from_utf8(vec![0x66, 0xff]).unwrap_err();
        assert!(
            DisassemblerError::InvalidUtf8InDynstr(utf8_error)
                .to_string()
                .starts_with("Non-UTF8 data in .dynstr section: invalid utf-8")
        );
        assert_eq!(
            DisassemblerError::InvalidShstrndx {
                shstrndx: 9,
                shnum: 6,
            }
            .to_string(),
            "Invalid section header string table index: 9, section header count: 6"
        );
        assert_eq!(
            DisassemblerError::InvalidSectionName {
                sh_name: 0x40,
                strtab_len: 0x2a,
            }
            .to_string(),
            "Invalid section name offset: sh_name 0x40 exceeds string table length 0x2a"
        );
        assert_eq!(
            DisassemblerError::SectionDataOutOfBounds {
                section: ".text".to_string(),
                offset: 0x1000,
                size: 0x20,
                file_len: 0x800,
            }
            .to_string(),
            "Section .text data out of bounds: offset 0x1000 + size 0x20 exceeds file length 0x800"
        );
    }
}
