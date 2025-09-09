use anyhow::Result;
use crate::errors::CompileError;

// Tokenizer and parser
pub mod parser;
pub mod lexer;
pub mod opcode;

// Error handling and diagnostics
pub mod macros;
pub mod errors;
pub mod messages;

// Intermediate Representation
pub mod astnode;
pub mod dynsym;

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
    parser::Parser,
    program::Program,
    lexer::tokenize,
};

pub fn assemble(source: &str) -> Result<Vec<u8>, Vec<CompileError>>{
    let tokens = match tokenize(&source) {
        Ok(tokens) => tokens,
        Err(errors) => {
            return Err(errors);
        }
    };
    let mut parser = Parser::new(tokens);
    let parse_result = match parser.parse() {
        Ok(program) => program,
        Err(errors) => {
            return Err(errors);
        }
    };
    let program = Program::from_parse_result(parse_result);
    let bytecode = program.emit_bytecode();
    Ok(bytecode)
    
}

