use anyhow::Result;

// Parser
pub mod parser;

// Error handling and diagnostics
pub mod errors;
pub mod macros;
pub mod messages;

// Intermediate Representation
pub mod ast;
pub mod astnode;
pub mod dynsym;
pub mod syscall;

// ELF header, program, section
pub mod header;
pub mod program;
pub mod section;

// Debug info
pub mod debuginfo;

// WASM bindings
#[cfg(target_arch = "wasm32")]
pub mod wasm;

pub use self::{
    errors::CompileError,
    parser::{ParseResult, Token, parse},
    program::Program,
};

pub fn assemble(source: &str) -> Result<Vec<u8>, Vec<CompileError>> {
    let parse_result = match parse(source) {
        Ok(result) => result,
        Err(errors) => {
            return Err(errors);
        }
    };
    let program = Program::from_parse_result(parse_result);
    let bytecode = program.emit_bytecode();
    Ok(bytecode)
}
