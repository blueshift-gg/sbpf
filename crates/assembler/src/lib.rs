extern crate num_traits;
extern crate num_derive;
extern crate anyhow;

use std::path::Path;
use anyhow::{Error, Result};

// Tokenizer and parser
pub mod parser;
pub mod lexer;
pub mod opcode;

// Intermediate Representation
pub mod astnode;
pub mod dynsym;

// ELF header, program, section
pub mod header;
pub mod program;
pub mod section;

// Debug info
pub mod debuginfo;

// Production-level systems
pub mod error;
pub mod validator;
pub mod metrics;

#[cfg(test)]
mod tests;

// Re-export main types
pub use self::{
    parser::Parser,
    program::Program,
    lexer::tokenize,
    error::{CompileError, Span},
    validator::{AssemblyValidator, ValidationError},
    metrics::{CompilationMetrics, PerformanceProfiler, AssemblyProfiler},
};

pub fn assemble(src: &str, deploy: &str) -> Result<CompilationMetrics> {
    let mut profiler = PerformanceProfiler::new();
    
    // Validate input
    profiler.checkpoint("validate");
    let validator = AssemblyValidator::default();
    validator.validate(src).map_err(|errors| {
        let mut error_msg = String::new();
        for error in errors {
            error_msg.push_str(&format!("{}\n", error));
        }
        Error::msg(error_msg)
    })?;
    profiler.end_checkpoint("validate");
    
    // Lexical analysis
    profiler.checkpoint("lex");
    let tokens = match tokenize(src) {
        Ok(tokens) => tokens,
        Err(e) => {
            profiler.increment_error_count();
            return Err(Error::msg(format!("Tokenizer error: {}", e)));
        }
    };
    profiler.set_instruction_count(tokens.len());
    profiler.end_checkpoint("lex");
    
    // Parsing
    profiler.checkpoint("parse");
    let mut parser = Parser::new(tokens);
    let parse_result = match parser.parse() {
        Ok(program) => program,
        Err(e) => {
            profiler.increment_error_count();
            return Err(Error::msg(format!("Parser error: {}", e)));
        }
    };
    profiler.end_checkpoint("parse");
    
    // Code generation
    profiler.checkpoint("codegen");
    let program = Program::from_parse_result(parse_result);
    let bytecode = program.emit_bytecode();
    profiler.set_bytecode_size(bytecode.len());
    profiler.set_memory_usage(std::mem::size_of_val(&bytecode));
    profiler.end_checkpoint("codegen");
    
    // Write output
    let output_path = Path::new(deploy)
        .join(Path::new(src)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .replace(".s", ".so"));
    
    std::fs::write(&output_path, bytecode)?;
    
    // Return metrics
    let metrics = profiler.finish();
    Ok(metrics)
}

pub fn assemble_with_validation(src: &str, _deploy: &str) -> Result<(Vec<u8>, CompilationMetrics)> {
    let mut profiler = PerformanceProfiler::new();
    
    // Validate input
    profiler.checkpoint("validate");
    let validator = AssemblyValidator::default();
    validator.validate(src).map_err(|errors| {
        let mut error_msg = String::new();
        for error in errors {
            error_msg.push_str(&format!("{}\n", error));
        }
        Error::msg(error_msg)
    })?;
    profiler.end_checkpoint("validate");
    
    // Lexical analysis
    profiler.checkpoint("lex");
    let tokens = match tokenize(src) {
        Ok(tokens) => tokens,
        Err(e) => {
            profiler.increment_error_count();
            return Err(Error::msg(format!("Tokenizer error: {}", e)));
        }
    };
    profiler.set_instruction_count(tokens.len());
    profiler.end_checkpoint("lex");
    
    // Parsing
    profiler.checkpoint("parse");
    let mut parser = Parser::new(tokens);
    let parse_result = match parser.parse() {
        Ok(program) => program,
        Err(e) => {
            profiler.increment_error_count();
            return Err(Error::msg(format!("Parser error: {}", e)));
        }
    };
    profiler.end_checkpoint("parse");
    
    // Code generation
    profiler.checkpoint("codegen");
    let program = Program::from_parse_result(parse_result);
    let bytecode = program.emit_bytecode();
    profiler.set_bytecode_size(bytecode.len());
    profiler.set_memory_usage(std::mem::size_of_val(&bytecode));
    profiler.end_checkpoint("codegen");
    
    // Return bytecode and metrics
    let metrics = profiler.finish();
    Ok((bytecode, metrics))
}

pub fn analyze_program(src: &str) -> Result<AssemblyProfiler> {
    let tokens = tokenize(src).map_err(|e| Error::msg(format!("Tokenizer error: {}", e)))?;
    let mut parser = Parser::new(tokens);
    let parse_result = parser.parse().map_err(|e| Error::msg(format!("Parser error: {}", e)))?;
    let program = Program::from_parse_result(parse_result);
    
    let mut profiler = AssemblyProfiler::new();
    let _report = profiler.analyze_program(&program);
    
    Ok(profiler)
}
