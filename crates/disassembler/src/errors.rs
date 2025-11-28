use {sbpf_common::errors::SBPFError, thiserror::Error};

#[derive(Debug, Error)]
pub enum DisassemblerError {
    #[error("Non-standard ELF header")]
    NonStandardElfHeader,
    #[error("Invalid Program Type")]
    InvalidProgramType,
    #[error("Invalid Section Header Type")]
    InvalidSectionHeaderType,
    #[error("Invalid OpCode")]
    InvalidOpcode,
    #[error("Invalid Immediate")]
    InvalidImmediate,
    #[error("Invalid data length")]
    InvalidDataLength,
    #[error("Invalid string")]
    InvalidString,
    #[error("Bytecode error: {0}")]
    BytecodeError(String),
    #[error("Missing text section")]
    MissingTextSection,
    #[error("Invalid offset in .dynstr section")]
    InvalidDynstrOffset,
    #[error("Non-UTF8 data in .dynstr section")]
    InvalidUtf8InDynstr,
}

impl From<SBPFError> for DisassemblerError {
    fn from(err: SBPFError) -> Self {
        match err {
            SBPFError::BytecodeError { error, .. } => DisassemblerError::BytecodeError(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_sbpf_error() {
        let sbpf_error = SBPFError::BytecodeError {
            error: "test error".to_string(),
            span: 0..8,
            custom_label: None,
        };
        let disasm_error: DisassemblerError = sbpf_error.into();
        assert!(matches!(disasm_error, DisassemblerError::BytecodeError(_)));
    }

    #[test]
    fn test_error_display() {
        assert_eq!(
            DisassemblerError::NonStandardElfHeader.to_string(),
            "Non-standard ELF header"
        );
        assert_eq!(
            DisassemblerError::InvalidProgramType.to_string(),
            "Invalid Program Type"
        );
        assert_eq!(
            DisassemblerError::InvalidSectionHeaderType.to_string(),
            "Invalid Section Header Type"
        );
        assert_eq!(
            DisassemblerError::InvalidOpcode.to_string(),
            "Invalid OpCode"
        );
        assert_eq!(
            DisassemblerError::InvalidImmediate.to_string(),
            "Invalid Immediate"
        );
        assert_eq!(
            DisassemblerError::InvalidDataLength.to_string(),
            "Invalid data length"
        );
        assert_eq!(
            DisassemblerError::InvalidString.to_string(),
            "Invalid string"
        );
        assert_eq!(
            DisassemblerError::BytecodeError("custom".to_string()).to_string(),
            "Bytecode error: custom"
        );
        assert_eq!(
            DisassemblerError::MissingTextSection.to_string(),
            "Missing text section"
        );
    }
}
