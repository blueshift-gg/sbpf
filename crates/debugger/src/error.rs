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
    #[error("VM error: {0}")]
    Vm(#[from] sbpf_vm::errors::SbpfVmError),
    #[error("Assembler error: {0}")]
    Assembler(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

pub type DebuggerResult<T> = Result<T, DebuggerError>;
