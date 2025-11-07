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
    MissingTextSection
}

impl From<SBPFError> for DisassemblerError {
    fn from(err: SBPFError) -> Self {
        match err {
            SBPFError::BytecodeError { error, .. } => DisassemblerError::BytecodeError(error),
        }
    }
}
