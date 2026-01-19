use {sbpf_common::errors::ExecutionError, thiserror::Error};

/// VM errors
#[derive(Error, Debug, Clone)]
pub enum SbpfVmError {
    #[error("Division by zero")]
    DivisionByZero,

    #[error("Invalid memory access at address {0:#x}")]
    InvalidMemoryAccess(u64),

    #[error("Memory region out of bounds: address {0:#x}, length {1}")]
    MemoryOutOfBounds(u64, usize),

    #[error("Invalid operand")]
    InvalidOperand,

    #[error("Program counter out of bounds: {0}")]
    PcOutOfBounds(usize),

    #[error("Invalid instruction format")]
    InvalidInstruction,

    #[error("Call depth exceeded (max {0})")]
    CallDepthExceeded(usize),

    #[error("Execution limit reached ({0} steps)")]
    ExecutionLimitReached(u64),

    #[error("Syscall error: {0}")]
    SyscallError(String),
}

pub type SbpfVmResult<T> = Result<T, SbpfVmError>;

impl From<ExecutionError> for SbpfVmError {
    fn from(err: ExecutionError) -> Self {
        match err {
            ExecutionError::DivisionByZero => SbpfVmError::DivisionByZero,
            ExecutionError::InvalidOperand => SbpfVmError::InvalidOperand,
            ExecutionError::InvalidInstruction => SbpfVmError::InvalidInstruction,
            ExecutionError::CallDepthExceeded(n) => SbpfVmError::CallDepthExceeded(n),
            ExecutionError::InvalidMemoryAccess(addr) => SbpfVmError::InvalidMemoryAccess(addr),
            ExecutionError::SyscallError(s) => SbpfVmError::SyscallError(s),
        }
    }
}
