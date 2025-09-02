use crate::opcode::Opcode;
use crate::lexer::{Token, ImmediateValue};
use crate::dynsym::RelocationType;
use crate::debuginfo::{DebugInfo, RegisterHint, RegisterType};
use std::collections::HashMap;
use std::ops::Range;
use codespan_reporting::files::SimpleFile;
use crate::debuginfo::span_to_line_number;
use crate::errors::CompileError;

#[derive(Debug, Clone)]
pub enum ASTNode {
    // only present in the AST
    Directive {
        directive: Directive,
    },
    GlobalDecl {
        global_decl: GlobalDecl,
    },
    EquDecl {
        equ_decl: EquDecl,
    },
    ExternDecl {
        extern_decl: ExternDecl,
    },
    RodataDecl {
        rodata_decl: RodataDecl,
    },
    Label {
        label: Label,
    },
    // present in the bytecode
    ROData {
        rodata: ROData,
        offset: u64,
    },
    Instruction {
        instruction: Instruction,
        offset: u64,
    },

}

#[derive(Debug, Clone)]
pub struct Directive {
    pub name: String,
    pub args: Vec<Token>,
    pub span: Range<usize>,
}

#[derive(Debug, Clone)]
pub struct GlobalDecl {
    pub entry_label: String,
    pub span: Range<usize>,
}

impl GlobalDecl {
    pub fn get_entry_label(&self) -> String {
        self.entry_label.clone()
    }
}

#[derive(Debug, Clone)]
pub struct EquDecl {
    pub name: String,
    pub value: Token,
    pub span: Range<usize>,
}

impl EquDecl {
    pub fn get_name(&self) -> String {
        self.name.clone()
    }
    pub fn get_val(&self) -> ImmediateValue {
        match &self.value {
            Token::ImmediateValue(val, _) => val.clone(),
            _ => panic!("Invalid Equ declaration"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExternDecl {
    pub args: Vec<Token>,
    pub span: Range<usize>,
}

#[derive(Debug, Clone)]
pub struct RodataDecl {
    pub span: Range<usize>,
}

#[derive(Debug, Clone)]
pub struct Label {
    pub name: String,
    pub span: Range<usize>,
}

#[derive(Debug, Clone)]
pub struct ROData {
    pub name: String,
    pub args: Vec<Token>,
    pub span: Range<usize>,
}

impl ROData {
    pub fn get_size(&self) -> u64 {
        let size: u64;
        match (
            &self.args[0],
            &self.args[1],
        ) {
            (Token::Directive(_, _), Token::StringLiteral(s, _)) => {
                size = s.len() as u64;
            }
            (Token::Directive(directive, _), Token::ImmediateValue(_, _)) => {
                match directive.as_str() {
                    "byte" => {
                        size = 1;
                    }
                    "short" => {
                        size = 2;
                    }
                    "int" | "long" => {
                        size = 4;
                    }
                    "quad" => {
                        size = 8;
                    }
                    "octa" => {
                        size = 16;
                    }
                    _ => panic!("Invalid ROData declaration"),
                }
            }
            _ => panic!("Invalid ROData declaration"),
        }
        size
    }
    pub fn verify(&self) -> Result<(), CompileError> {
        match (
            &self.args[0],
            &self.args[1],
        ) {
            (Token::Directive(directive, directive_span), Token::StringLiteral(_, _)) => {
                if directive.as_str() != "ascii" {
                    return Err(CompileError::InvalidRODataDirective { span: directive_span.clone(), custom_label: None });
                }
            }
            (Token::Directive(directive, directive_span), Token::ImmediateValue(_, immediate_value_span)) => {
                match directive.as_str() {
                    "byte" => {
                        if let Token::ImmediateValue(value, _) = &self.args[1] {
                            match value {
                                ImmediateValue::Int(val) => if *val < i8::MIN as i64 || *val > i8::MAX as i64 {
                                    return Err(CompileError::OutOfRangeLiteral { span: immediate_value_span.clone(), custom_label: None });
                                },
                                ImmediateValue::Addr(val) => if *val < i8::MIN as i64 || *val > i8::MAX as i64 {
                                    return Err(CompileError::OutOfRangeLiteral { span: immediate_value_span.clone(), custom_label: None });
                                },
                            }
                        }
                    }
                    "short" => {
                        if let Token::ImmediateValue(value, _) = &self.args[1] {
                            match value {
                                ImmediateValue::Int(val) => if *val < i16::MIN as i64 || *val > i16::MAX as i64 {
                                    return Err(CompileError::OutOfRangeLiteral { span: immediate_value_span.clone(), custom_label: None });
                                },
                                ImmediateValue::Addr(val) => if *val < i16::MIN as i64 || *val > i16::MAX as i64 {
                                    return Err(CompileError::OutOfRangeLiteral { span: immediate_value_span.clone(), custom_label: None });
                                },
                            }
                        }
                    }
                    "int" | "long" => {
                        if let Token::ImmediateValue(value, _) = &self.args[1] {
                            match value {
                                ImmediateValue::Int(val) => if *val < i32::MIN as i64 || *val > i32::MAX as i64 {
                                    return Err(CompileError::OutOfRangeLiteral { span: immediate_value_span.clone(), custom_label: None });
                                },
                                ImmediateValue::Addr(val) => if *val < i32::MIN as i64 || *val > i32::MAX as i64 {
                                    return Err(CompileError::OutOfRangeLiteral { span: immediate_value_span.clone(), custom_label: None });
                                },
                            }
                        }
                    }
                    "quad" => {
                        if let Token::ImmediateValue(value, _) = &self.args[1] {
                            match value {
                                ImmediateValue::Int(val) => if *val < i64::MIN as i64 || *val > i64::MAX as i64 {
                                    return Err(CompileError::OutOfRangeLiteral { span: immediate_value_span.clone(), custom_label: None });
                                },
                                ImmediateValue::Addr(val) => if *val < i64::MIN as i64 || *val > i64::MAX as i64 {
                                    return Err(CompileError::OutOfRangeLiteral { span: immediate_value_span.clone(), custom_label: None });
                                },
                            }
                        }
                    }
                    "octa" => {
                        if let Token::ImmediateValue(value, _) = &self.args[1] {
                            match value {
                                ImmediateValue::Int(val) => if *val < i128::MIN as i64 || *val > i128::MAX as i64 {
                                    return Err(CompileError::OutOfRangeLiteral { span: directive_span.clone(), custom_label: None });
                                },
                                ImmediateValue::Addr(val) => if *val < i128::MIN as i64 || *val > i128::MAX as i64 {
                                    return Err(CompileError::OutOfRangeLiteral { span: directive_span.clone(), custom_label: None });
                                },
                            }
                        }
                    }
                    _ => {
                        return Err(CompileError::InvalidRODataDirective { span: directive_span.clone(), custom_label: None });
                    }
                }
            }
            _ => {
                return Err(CompileError::InvalidRodataDecl { span: self.span.clone(), custom_label: None });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Instruction {
    pub opcode: Opcode,
    pub operands: Vec<Token>,
    pub span: Range<usize>,
}

impl Instruction {
    pub fn get_size(&self) -> u64 {
        match self.opcode {
            Opcode::Lddw => 16,
            _ => 8,
        }
    }
    pub fn needs_relocation(&self) -> bool {
        match self.opcode {
            Opcode::Call | Opcode::Lddw => {
                match &self.operands.last() {
                    Some(Token::Identifier(_, _)) => true,
                    _ => false,
                }
            },
            _ => false,
        }
    }
    pub fn is_jump(&self) -> bool {
        match self.opcode {
            Opcode::Ja | Opcode::JeqImm | Opcode::JgtImm | Opcode::JgeImm   //
            | Opcode::JltImm | Opcode::JleImm | Opcode::JsetImm             // 
            | Opcode::JneImm | Opcode::JsgtImm | Opcode::JsgeImm            // 
            | Opcode::JsltImm | Opcode::JsleImm | Opcode::JeqReg            // 
            | Opcode::JgtReg | Opcode::JgeReg | Opcode::JltReg              // 
            | Opcode::JleReg | Opcode::JsetReg | Opcode::JneReg             // 
            | Opcode::JsgtReg | Opcode::JsgeReg | Opcode::JsltReg           // 
            | Opcode::JsleReg => true,
            _ => false,
        }
    }
    pub fn get_relocation_info(&self) -> (RelocationType, String) {
        match self.opcode {
            Opcode::Lddw => {
                match &self.operands[1] {
                    Token::Identifier(name, _) => (RelocationType::RSbf64Relative, name.clone()),
                    _ => panic!("Expected label operand"),
                }
            },
            _ => {
                if let Token::Identifier(name, _) = &self.operands[0] {
                    (RelocationType::RSbfSyscall, name.clone()) 
                } else {
                    panic!("Expected label operand")
                }
            },
        }
    }
}


impl ASTNode {
    pub fn bytecode_with_debug_map(&self, file: Option<&SimpleFile<String, String>>) -> Option<(Vec<u8>, HashMap<u64, DebugInfo>)> {
        match self {
            ASTNode::Instruction { instruction: Instruction { opcode, operands, span }, offset } => {
                let mut bytes = Vec::new();
                let mut line_map = HashMap::new();
                let mut debug_map = HashMap::new();
                // Record the start of this instruction
                let line_number = if let Some(file) = file {
                    span_to_line_number(span.clone(), file)
                } else {
                    1 // fallback
                };
                line_map.insert(*offset, line_number);
                let mut debug_info = DebugInfo::new(line_number);
                bytes.push(opcode.to_bytecode());  // 1 byte opcode
                
                if *opcode == Opcode::Call {
                    bytes.extend_from_slice(&[0x10, 0x00, 0x00]);
                    if let Some(Token::ImmediateValue(imm, _)) = operands.last() {
                        let imm32 = match imm {
                            ImmediateValue::Int(val) => *val as i32,
                            ImmediateValue::Addr(val) => *val as i32,
                        };
                        bytes.extend_from_slice(&imm32.to_le_bytes());
                    } else {
                        // external calls
                        bytes.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
                    } 
                } else if *opcode == Opcode::Lddw {
                    match &operands[..] {
                        [Token::Register(reg, _), Token::ImmediateValue(imm, _)] => {
                            // 1 byte register number (strip 'r' prefix)
                            bytes.push(*reg);
                            
                            // 2 bytes of zeros (offset/reserved)
                            bytes.extend_from_slice(&[0, 0]);

                            // 8 bytes immediate value in little-endian
                            let imm64 = match imm {
                                ImmediateValue::Int(val) => *val as i64,
                                ImmediateValue::Addr(val) => *val as i64,
                            };
                            bytes.extend_from_slice(&imm64.to_le_bytes()[..4]);
                            bytes.extend_from_slice(&[0, 0, 0, 0]);
                            bytes.extend_from_slice(&imm64.to_le_bytes()[4..8]);
                        }
                        _ => {}
                    }
                } else {
                    match &operands[..] {
                        [Token::ImmediateValue(imm, _)] => {
                            // 1 byte of zeros (no register)
                            bytes.push(0);
                            
                            if *opcode == Opcode::Ja {
                                // 2 bytes immediate value in little-endian for 'ja'
                                let imm16 = match imm {
                                    ImmediateValue::Int(val) => *val as i16,
                                    ImmediateValue::Addr(val) => *val as i16,
                                };
                                bytes.extend_from_slice(&imm16.to_le_bytes());
                            } else {
                                // 4 bytes immediate value in little-endian
                                let imm32 = match imm {
                                    ImmediateValue::Int(val) => *val as i32,
                                    ImmediateValue::Addr(val) => *val as i32,
                                };
                                bytes.extend_from_slice(&imm32.to_le_bytes());
                            }
                        },

                        [Token::Register(reg, _)] => {
                            bytes.push(*reg);
                            bytes.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
                        },

                        [Token::Register(reg, _), Token::ImmediateValue(imm, _)] => {
                            // 1 byte register number (strip 'r' prefix)
                            bytes.push(*reg);
                            
                            // 2 bytes of zeros (offset/reserved)
                            bytes.extend_from_slice(&[0, 0]);
                            
                            // 4 bytes immediate value in little-endian
                            let imm32 = match imm {
                                ImmediateValue::Int(val) => *val as i32,
                                ImmediateValue::Addr(val) => {
                                    debug_info.register_hint = RegisterHint {
                                        register: *reg as usize,
                                        register_type: RegisterType::Addr
                                    };
                                    *val as i32
                                }
                            };
                            bytes.extend_from_slice(&imm32.to_le_bytes());
                        },

                        [Token::Register(reg, _), Token::ImmediateValue(imm, _), Token::ImmediateValue(offset, _)] => {
                            // 1 byte register number (strip 'r' prefix)
                            bytes.push(*reg);
                            
                            // 2 bytes of offset in little-endian
                            let offset16 = match offset {
                                ImmediateValue::Int(val) => *val as u16,
                                ImmediateValue::Addr(val) => *val as u16,
                            };
                            bytes.extend_from_slice(&offset16.to_le_bytes());
                            
                            // 4 bytes immediate value in little-endianß
                            let imm32 = match imm {
                                ImmediateValue::Int(val) => *val as i32,
                                ImmediateValue::Addr(val) => {
                                    debug_info.register_hint = RegisterHint {
                                        register: *reg as usize,
                                        register_type: RegisterType::Addr
                                    };
                                    *val as i32
                                }
                            };
                            bytes.extend_from_slice(&imm32.to_le_bytes());
                        },                    
                        
                        [Token::Register(dst, _), Token::Register(src, _)] => {
                            // Convert register strings to numbers
                            let dst_num = dst;
                            let src_num = src;
                            
                            // Combine src and dst into a single byte (src in high nibble, dst in low nibble)
                            let reg_byte = (src_num << 4) | dst_num;
                            bytes.push(reg_byte);
                        },
                        [Token::Register(dst, _), Token::Register(reg, _), Token::ImmediateValue(offset, _)] => {
                            // Combine base register and destination register into a single byte
                            let reg_byte = (reg << 4) | dst;
                            bytes.push(reg_byte);
                            
                            // Add the offset as a 16-bit value in little-endian
                            let offset16 = match offset {
                                ImmediateValue::Int(val) => *val as u16,
                                ImmediateValue::Addr(val) => *val as u16,
                            };
                            bytes.extend_from_slice(&offset16.to_le_bytes());
                        },
                        [Token::Register(reg, _), Token::ImmediateValue(offset, _), Token::Register(dst, _)] => {
                            // Combine base register and destination register into a single byte
                            let reg_byte = (dst << 4) | reg;
                            bytes.push(reg_byte);
                            
                            // Add the offset as a 16-bit value in little-endian
                            let offset16 = match offset {
                                ImmediateValue::Int(val) => *val as u16,
                                ImmediateValue::Addr(val) => *val as u16,
                            };
                            bytes.extend_from_slice(&offset16.to_le_bytes());
                        }
                        
                        _ => {}
                    }
                }

                // Add padding to make it 8 or 16 bytes depending on opcode
                let target_len = if *opcode == Opcode::Lddw { 16 } else { 8 };
                while bytes.len() < target_len {
                    bytes.push(0);
                }

                debug_map.insert(*offset, debug_info);
                
                Some((bytes, debug_map))
            },
            ASTNode::ROData { rodata: ROData { name: _, args, .. }, .. } => {
                let mut bytes = Vec::new();
                let debug_map = HashMap::<u64, DebugInfo>::new();
                match (
                    &args[0],
                    &args[1],
                ) {
                    (Token::Directive(directive, _), Token::StringLiteral(str_literal, _)) => {
                        if directive == "ascii" {
                            // Convert string to bytes and add null terminator
                            let str_bytes = str_literal.as_bytes().to_vec();
                            bytes.extend(str_bytes);
                        } else {
                            panic!("Invalid ROData declaration");
                        }
                    } 
                    (Token::Directive(directive, _), Token::ImmediateValue(imm, _)) => {
                        if directive == "byte" {
                            let imm8 = match imm {
                                ImmediateValue::Int(val) => *val as i8,
                                ImmediateValue::Addr(val) => *val as i8,
                            };
                            bytes.extend(imm8.to_le_bytes());
                        } else if directive == "short" {
                            let imm16 = match imm {
                                ImmediateValue::Int(val) => *val as i16,
                                ImmediateValue::Addr(val) => *val as i16,
                            };
                            bytes.extend(imm16.to_le_bytes());
                        } else if directive == "int" || directive == "long" {
                            let imm32 = match imm {
                                ImmediateValue::Int(val) => *val as i32,
                                ImmediateValue::Addr(val) => *val as i32,
                            };
                            bytes.extend(imm32.to_le_bytes());
                        } else if directive == "quad" {
                            let imm64 = match imm {
                                ImmediateValue::Int(val) => *val as i64,
                                ImmediateValue::Addr(val) => *val as i64,
                            };
                            bytes.extend(imm64.to_le_bytes());
                        } else if directive == "octa" {
                            let imm128 = match imm {
                                ImmediateValue::Int(val) => *val as i128,
                                ImmediateValue::Addr(val) => *val as i128,
                            };
                            bytes.extend(imm128.to_le_bytes());
                        } else {
                            panic!("Invalid ROData declaration");
                        }
                    }
                    _ => panic!("Invalid ROData declaration"),
                }
                Some((bytes, debug_map))
            },
            _ => None
        }
    }

    // Keep the old bytecode method for backward compatibility
    pub fn bytecode(&self) -> Option<Vec<u8>> {
        self.bytecode_with_debug_map(None).map(|(bytes, _)| bytes)
    }
}
