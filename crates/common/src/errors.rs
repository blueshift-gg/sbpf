use {std::ops::Range, thiserror::Error};

#[derive(Debug, Error)]
pub enum SBPFError {
    #[error("Bytecode error: {error}")]
    BytecodeError {
        error: String,
        span: Range<usize>,
        custom_label: Option<String>,
    },
}

impl SBPFError {
    pub fn label(&self) -> &str {
        match self {
            Self::BytecodeError { custom_label, .. } => {
                custom_label.as_deref().unwrap_or("Bytecode error")
            }
        }
    }

    pub fn span(&self) -> &Range<usize> {
        match self {
            Self::BytecodeError { span, .. } => span,
        }
    }
}

#[derive(Error, Debug, Clone)]
pub enum ExecutionError {
    #[error("Division by zero")]
    DivisionByZero,

    #[error("Invalid operand")]
    InvalidOperand,

    #[error("Invalid instruction format")]
    InvalidInstruction,

    #[error("Call depth exceeded (max {0})")]
    CallDepthExceeded(usize),

    #[error("Invalid memory access at address {0:#x}")]
    InvalidMemoryAccess(u64),

    #[error("Syscall error: {0}")]
    SyscallError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytecode_error() {
        let error = SBPFError::BytecodeError {
            error: "Invalid opcode".to_string(),
            span: 10..20,
            custom_label: Some("Custom error message".to_string()),
        };

        assert_eq!(error.label(), "Custom error message");
        assert_eq!(error.span(), &(10..20));
        assert_eq!(error.to_string(), "Bytecode error: Invalid opcode");
    }
}
