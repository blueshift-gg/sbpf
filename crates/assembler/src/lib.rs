use anyhow::Result;

// Tokenizer and parser
pub mod lexer;
pub mod parser;

// Error handling and diagnostics
pub mod errors;
pub mod macros;
pub mod messages;

// Intermediate Representation
pub mod ast;
pub mod astnode;
pub mod dynsym;
pub mod instruction;
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

pub use self::{errors::CompileError, lexer::tokenize, parser::parse_tokens, program::Program};

pub fn assemble(source: &str) -> Result<Vec<u8>, Vec<CompileError>> {
    let tokens = match tokenize(source) {
        Ok(tokens) => tokens,
        Err(errors) => {
            return Err(errors);
        }
    };
    let parse_result = match parse_tokens(&tokens) {
        Ok(program) => program,
        Err(errors) => {
            return Err(errors);
        }
    };
    let program = Program::from_parse_result(parse_result);
    let bytecode = program.emit_bytecode();
    Ok(bytecode)
}
