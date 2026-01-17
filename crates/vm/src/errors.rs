use thiserror::Error;

/// VM errors
#[derive(Error, Debug, Clone)]
pub enum VmError {
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
}

pub type VmResult<T> = Result<T, VmError>;
