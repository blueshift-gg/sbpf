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
