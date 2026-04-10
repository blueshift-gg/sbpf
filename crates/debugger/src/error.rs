use thiserror::Error;

#[derive(Debug, Error)]
pub enum DebuggerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ELF parse error: {0}")]
    Elf(#[from] object::Error),
    #[error("DWARF error: {0}")]
    Dwarf(#[from] gimli::Error),
    #[error("Disassembler error: {0}")]
    Disassembler(#[from] sbpf_disassembler::errors::DisassemblerError),
    #[error("Runtime error: {0}")]
    Runtime(#[from] sbpf_runtime::errors::RuntimeError),
    #[error("Assembler error: {0}")]
    Assembler(String),
    #[error("Deserialize error: {0}")]
    DeserializeError(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

pub type DebuggerResult<T> = Result<T, DebuggerError>;
