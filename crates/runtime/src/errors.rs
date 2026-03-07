use {sbpf_vm::errors::SbpfVmError, thiserror::Error};

#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("Failed to read ELF file: {0}")]
    ElfReadError(#[from] std::io::Error),

    #[error("Failed to parse ELF: {0}")]
    ElfParseError(String),

    #[error("VM error: {0}")]
    VmError(#[from] SbpfVmError),

    #[error("Missing account: {0}")]
    MissingAccount(String),

    #[error("Program not found: {0}")]
    ProgramNotFound(String),

    #[error("VM not prepared — call prepare() or run() first")]
    VmNotPrepared,

    #[error("Register index {0} out of range")]
    RegisterOutOfRange(usize),
}

pub type RuntimeResult<T> = Result<T, RuntimeError>;
