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

    #[error("CPI depth exceeded (max: {0})")]
    CpiDepthExceeded(usize),

    #[error("CPI privilege escalation: {0} on account {1}")]
    PrivilegeEscalation(String, String),

    #[error("VM not prepared — call prepare() or run() first")]
    VmNotPrepared,

    #[error("Register index {0} out of range")]
    RegisterOutOfRange(usize),

    #[error("Builtin program error: {0}")]
    BuiltinError(String),

    #[error("External account lamport spend: {0}")]
    ExternalAccountLamportSpend(String),

    #[error("Unbalanced lamports: pre={0}, post={1}")]
    UnbalancedInstruction(u64, u64),
}

pub type RuntimeResult<T> = Result<T, RuntimeError>;
