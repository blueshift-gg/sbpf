use std::fmt;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

impl Span {
    pub fn new(start: usize, end: usize, line: usize, column: usize) -> Self {
        Self { start, end, line, column }
    }
    
    pub fn single_char(pos: usize, line: usize, column: usize) -> Self {
        Self { start: pos, end: pos + 1, line, column }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}:{}", self.line, self.column)
    }
}

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("Syntax error at {span}: {message}")]
    Syntax { message: String, span: Span },
    
    #[error("Semantic error at {span}: {message}")]
    Semantic { message: String, span: Span },
    
    #[error("Lexer error at {span}: {message}")]
    Lexer { message: String, span: Span },
    
    #[error("Parser error at {span}: {message}")]
    Parser { message: String, span: Span },
    
    #[error("Codegen error: {message}")]
    Codegen { message: String },
    
    #[error("Linker error: {message}")]
    Linker { message: String },
    
    #[error("Validation error at {span}: {message}")]
    Validation { message: String, span: Span },
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Program too large: {actual} bytes (max: {max})")]
    ProgramTooLarge { actual: usize, max: usize },
    
    #[error("Invalid instruction '{instruction}' at {span}")]
    InvalidInstruction { instruction: String, span: Span },
    
    #[error("Undefined symbol '{symbol}' at {span}")]
    UndefinedSymbol { symbol: String, span: Span },
    
    #[error("Register out of bounds: {register} (valid: 0-9)")]
    InvalidRegister { register: u8, span: Span },
    
    #[error("Immediate value out of range: {value}")]
    ImmediateOutOfRange { value: i64, span: Span },
}

impl CompileError {
    pub fn with_help(self, help: &str) -> CompileErrorWithHelp {
        CompileErrorWithHelp {
            error: self,
            help: help.to_string(),
        }
    }
}

#[derive(Debug)]
pub struct CompileErrorWithHelp {
    pub error: CompileError,
    pub help: String,
}

impl fmt::Display for CompileErrorWithHelp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\n\n💡 {}", self.error, self.help)
    }
}


pub type ParserError = CompileError;
pub type ProgramError = CompileError;
pub type TokenizerError = CompileError;


#[derive(Debug)]
pub struct ErrorContext {
    pub source: String,
    pub span: Span,
    pub context_lines: Vec<String>,
}

impl ErrorContext {
    pub fn new(source: &str, span: &Span) -> Self {
        let lines: Vec<&str> = source.lines().collect();
        let start_line = span.line.saturating_sub(1);
        let end_line = (span.line + 1).min(lines.len());
        
        let context_lines = lines
            .iter()
            .enumerate()
            .filter(|(i, _)| *i >= start_line && *i < end_line)
            .map(|(i, line)| {
                if i == span.line - 1 {
                    format!("{:>4} | {} ← Error here", i + 1, line)
                } else {
                    format!("{:>4} | {}", i + 1, line)
                }
            })
            .collect();
        
        Self {
            source: source.to_string(),
            span: span.clone(),
            context_lines,
        }
    }
} 