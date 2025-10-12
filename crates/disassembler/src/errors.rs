use thiserror::Error;

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
}
