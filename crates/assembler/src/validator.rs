use super::error::{CompileError, Span};
use super::opcode::Opcode;
use std::collections::HashSet;

#[derive(Debug)]
pub struct AssemblyValidator {
    pub max_file_size: usize,
    pub max_instructions: usize,
    pub max_registers: usize,
    pub max_immediate_value: i64,
    pub allowed_instructions: HashSet<String>,
    pub allowed_directives: HashSet<String>,
}

impl Default for AssemblyValidator {
    fn default() -> Self {
        let mut allowed_instructions = HashSet::new();
        for opcode in [
            Opcode::Lddw, Opcode::Ldxb, Opcode::Ldxh, Opcode::Ldxw, Opcode::Ldxdw,
            Opcode::Stb, Opcode::Sth, Opcode::Stw, Opcode::Stdw,
            Opcode::Stxb, Opcode::Stxh, Opcode::Stxw, Opcode::Stxdw,
            Opcode::Add32Imm, Opcode::Add32Reg, Opcode::Sub32Imm, Opcode::Sub32Reg,
            Opcode::Mul32Imm, Opcode::Mul32Reg, Opcode::Div32Imm, Opcode::Div32Reg,
            Opcode::Or32Imm, Opcode::Or32Reg, Opcode::And32Imm, Opcode::And32Reg,
            Opcode::Lsh32Imm, Opcode::Lsh32Reg, Opcode::Rsh32Imm, Opcode::Rsh32Reg,
            Opcode::Mod32Imm, Opcode::Mod32Reg, Opcode::Xor32Imm, Opcode::Xor32Reg,
            Opcode::Mov32Imm, Opcode::Mov32Reg, Opcode::Arsh32Imm, Opcode::Arsh32Reg,
            Opcode::Add64Imm, Opcode::Add64Reg, Opcode::Sub64Imm, Opcode::Sub64Reg,
            Opcode::Mul64Imm, Opcode::Mul64Reg, Opcode::Div64Imm, Opcode::Div64Reg,
            Opcode::Or64Imm, Opcode::Or64Reg, Opcode::And64Imm, Opcode::And64Reg,
            Opcode::Lsh64Imm, Opcode::Lsh64Reg, Opcode::Rsh64Imm, Opcode::Rsh64Reg,
            Opcode::Mod64Imm, Opcode::Mod64Reg, Opcode::Xor64Imm, Opcode::Xor64Reg,
            Opcode::Mov64Imm, Opcode::Mov64Reg, Opcode::Arsh64Imm, Opcode::Arsh64Reg,
            Opcode::Neg32, Opcode::Neg64,
            Opcode::Ja, Opcode::JeqImm, Opcode::JeqReg, Opcode::JgtImm, Opcode::JgtReg,
            Opcode::JgeImm, Opcode::JgeReg, Opcode::JltImm, Opcode::JltReg,
            Opcode::JleImm, Opcode::JleReg, Opcode::JsetImm, Opcode::JsetReg,
            Opcode::JneImm, Opcode::JneReg, Opcode::JsgtImm, Opcode::JsgtReg,
            Opcode::JsgeImm, Opcode::JsgeReg, Opcode::JsltImm, Opcode::JsltReg,
            Opcode::JsleImm, Opcode::JsleReg,
            Opcode::Call, Opcode::Callx, Opcode::Exit,
        ] {
            allowed_instructions.insert(opcode.to_str().to_string());
        }
        
        let mut allowed_directives = HashSet::new();
        for directive in ["globl", "rodata", "equ", "extern", "ascii", "asciiz"] {
            allowed_directives.insert(directive.to_string());
        }
        
        Self {
            max_file_size: 1024 * 1024, // 1MB
            max_instructions: 10000,
            max_registers: 10,
            max_immediate_value: 0x7fffffffffffffff, // i64::MAX
            allowed_instructions,
            allowed_directives,
        }
    }
}

impl AssemblyValidator {
    pub fn validate(&self, input: &str) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();
        
        // Check file size
        if input.len() > self.max_file_size {
            errors.push(ValidationError::FileTooLarge(input.len(), self.max_file_size));
        }
        
        // Check for null bytes (security)
        if input.contains('\0') {
            errors.push(ValidationError::NullBytes);
        }
        
        // Check instruction count
        let instruction_count = input.lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() && 
                !trimmed.starts_with("//") && 
                !trimmed.starts_with("#") &&
                !trimmed.starts_with(".") &&
                !trimmed.ends_with(":") // Don't count labels as instructions
            })
            .count();
            
        if instruction_count > self.max_instructions {
            errors.push(ValidationError::TooManyInstructions(instruction_count, self.max_instructions));
        }
        
        // Validate each line
        for (line_num, line) in input.lines().enumerate() {
            let span = Span::new(0, line.len(), line_num + 1, 0);
            self.validate_line(line, &span, &mut errors);
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    
    fn validate_line(&self, line: &str, span: &Span, errors: &mut Vec<ValidationError>) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("#") {
            return;
        }
        
        // Handle labels (lines ending with :)
        if trimmed.ends_with(':') {
            // Labels are valid, no validation needed
            return;
        }
        
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.is_empty() {
            return;
        }
        
        // Check directives
        if parts[0].starts_with('.') {
            let directive = &parts[0][1..];
            if !self.allowed_directives.contains(directive) {
                errors.push(ValidationError::InvalidDirective {
                    directive: directive.to_string(),
                    span: span.clone(),
                });
            }
            return;
        }
        
        // Check if this is a data section line (starts with a label and has a directive)
        if parts.len() >= 2 && parts[1].starts_with('.') {
            // This is a data section line like "message: .ascii "Hello, Solana!""
            // The first part is a label, second is a directive
            let directive = &parts[1][1..];
            if !self.allowed_directives.contains(directive) {
                errors.push(ValidationError::InvalidDirective {
                    directive: directive.to_string(),
                    span: span.clone(),
                });
            }
            return;
        }
        
        // Check instructions
        let instruction = parts[0];
        if !self.allowed_instructions.contains(instruction) {
            errors.push(ValidationError::InvalidInstruction {
                instruction: instruction.to_string(),
                span: span.clone(),
            });
        }
        
        // Validate operands
        for operand in &parts[1..] {
            self.validate_operand(operand, span, errors);
        }
    }
    
    fn validate_operand(&self, operand: &str, span: &Span, errors: &mut Vec<ValidationError>) {
        // Remove trailing commas
        let operand = operand.trim_end_matches(',');
        
        // Check registers
        if operand.starts_with('r') {
            if let Ok(reg_num) = operand[1..].parse::<u8>() {
                if usize::from(reg_num) >= self.max_registers {
                    errors.push(ValidationError::InvalidRegister {
                        register: reg_num,
                        span: span.clone(),
                    });
                }
            } else {
                errors.push(ValidationError::InvalidRegisterFormat {
                    operand: operand.to_string(),
                    span: span.clone(),
                });
            }
            return;
        }
        
        // Check immediate values
        if operand.starts_with("0x") {
            // Hex value
            if let Ok(value) = i64::from_str_radix(&operand[2..], 16) {
                if value > self.max_immediate_value {
                    errors.push(ValidationError::ImmediateOutOfRange {
                        value,
                        span: span.clone(),
                    });
                }
            } else {
                errors.push(ValidationError::InvalidHexValue {
                    operand: operand.to_string(),
                    span: span.clone(),
                });
            }
        } else if let Ok(value) = operand.parse::<i64>() {
            // Decimal value
            if value > self.max_immediate_value {
                errors.push(ValidationError::ImmediateOutOfRange {
                    value,
                    span: span.clone(),
                });
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("File too large: {0} bytes (max: {1})")]
    FileTooLarge(usize, usize),
    
    #[error("Too many instructions: {0} (max: {1})")]
    TooManyInstructions(usize, usize),
    
    #[error("Invalid instruction '{instruction}' at {span}")]
    InvalidInstruction { instruction: String, span: Span },
    
    #[error("Invalid directive '{directive}' at {span}")]
    InvalidDirective { directive: String, span: Span },
    
    #[error("Invalid register {register} at {span} (valid: 0-{})", 9)]
    InvalidRegister { register: u8, span: Span },
    
    #[error("Invalid register format '{operand}' at {span}")]
    InvalidRegisterFormat { operand: String, span: Span },
    
    #[error("Immediate value out of range: {value} at {span}")]
    ImmediateOutOfRange { value: i64, span: Span },
    
    #[error("Invalid hex value '{operand}' at {span}")]
    InvalidHexValue { operand: String, span: Span },
    
    #[error("Null bytes detected in input (security risk)")]
    NullBytes,
    
    #[error("Undefined symbol '{symbol}' at {span}")]
    UndefinedSymbol { symbol: String, span: Span },
}

impl From<ValidationError> for CompileError {
    fn from(error: ValidationError) -> Self {
        match error {
            ValidationError::FileTooLarge(actual, max) => CompileError::ProgramTooLarge { actual, max },
            ValidationError::InvalidInstruction { instruction, span } => CompileError::InvalidInstruction { instruction, span },
            ValidationError::InvalidRegister { register, span } => CompileError::InvalidRegister { register, span },
            ValidationError::ImmediateOutOfRange { value, span } => CompileError::ImmediateOutOfRange { value, span },
            ValidationError::UndefinedSymbol { symbol, span } => CompileError::UndefinedSymbol { symbol, span },
            _ => CompileError::Validation { 
                message: error.to_string(), 
                span: Span::new(0, 0, 0, 0) 
            },
        }
    }
} 