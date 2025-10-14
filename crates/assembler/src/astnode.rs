use crate::opcode::Opcode;
use crate::instruction::Instruction;
use crate::lexer::{Token, ImmediateValue};
use crate::debuginfo::{DebugInfo, RegisterHint, RegisterType};
use std::collections::HashMap;
use std::ops::Range;
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
        offset: u64,
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
    /// Validates that an immediate value is within the specified range
    fn validate_immediate_range(
        value: &ImmediateValue,
        min: i64,
        max: i64,
        span: Range<usize>,
    ) -> Result<(), CompileError> {
        match value {
            ImmediateValue::Int(val) => {
                if *val < min || *val > max {
                    return Err(CompileError::OutOfRangeLiteral { span, custom_label: None });
                }
            }
            ImmediateValue::Addr(val) => {
                if *val < min || *val > max {
                    return Err(CompileError::OutOfRangeLiteral { span, custom_label: None });
                }
            }
        }
        Ok(())
    }

    pub fn get_size(&self) -> u64 {
        let size: u64;
        match (
            &self.args[0],
            &self.args[1],
        ) {
            (Token::Directive(_, _), Token::StringLiteral(s, _)) => {
                size = s.len() as u64;
            }
            (Token::Directive(directive, _), Token::VectorLiteral(values, _)) => {
                match directive.as_str() {
                    "byte" => {
                        size = values.len() as u64 * 1;
                    }
                    "short" => {
                        size = values.len() as u64 * 2;
                    }
                    "int" | "long" => {
                        size = values.len() as u64 * 4;
                    }
                    "quad" => {
                        size = values.len() as u64 * 8;
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
            (Token::Directive(directive, directive_span), Token::VectorLiteral(values, vector_literal_span)) => {
                match directive.as_str() {
                    "byte" => {
                        for value in values {
                            Self::validate_immediate_range(value, i8::MIN as i64, i8::MAX as i64, vector_literal_span.clone())?;
                        }
                    }
                    "short" => {
                        for value in values {
                            Self::validate_immediate_range(value, i16::MIN as i64, i16::MAX as i64, vector_literal_span.clone())?;
                        }
                    }
                    "int" | "long" => {
                        for value in values {
                            Self::validate_immediate_range(value, i32::MIN as i64, i32::MAX as i64, vector_literal_span.clone())?;
                        }
                    }
                    "quad" => {
                        for value in values {
                            Self::validate_immediate_range(value, i64::MIN as i64, i64::MAX as i64, vector_literal_span.clone())?;
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

impl ASTNode {
    pub fn bytecode_with_debug_map(&self) -> Option<(Vec<u8>, HashMap<u64, DebugInfo>)> {
        match self {
            ASTNode::Instruction { instruction: Instruction { opcode, operands, span }, offset } => {
                let mut bytes = Vec::new();
                let mut debug_map = HashMap::new();
                let mut debug_info = DebugInfo::new(span.clone());
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
                            if *opcode == Opcode::Callx {
                                bytes.push(0);
                                bytes.extend_from_slice(&[0, 0]);
                                bytes.extend_from_slice(&[*reg, 0, 0, 0]);
                            } else {
                                bytes.push(*reg);
                                bytes.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
                            }
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
                    (Token::Directive(_, _), Token::StringLiteral(str_literal, _)) => {
                        let str_bytes = str_literal.as_bytes().to_vec();
                        bytes.extend(str_bytes);
                    } 
                    (Token::Directive(directive, _), Token::VectorLiteral(values, _)) => {
                        if directive == "byte" {
                            for value in values {
                                let imm8 = match value {
                                    ImmediateValue::Int(val) => *val as i8,
                                    ImmediateValue::Addr(val) => *val as i8,
                                };
                                bytes.extend(imm8.to_le_bytes());
                            }
                        } else if directive == "short" {
                            for value in values {
                                let imm16 = match value {
                                    ImmediateValue::Int(val) => *val as i16,
                                    ImmediateValue::Addr(val) => *val as i16,
                                };
                                bytes.extend(imm16.to_le_bytes());
                            }
                        } else if directive == "int" || directive == "long" {
                            for value in values {
                                let imm32 = match value {
                                    ImmediateValue::Int(val) => *val as i32,
                                    ImmediateValue::Addr(val) => *val as i32,
                                };
                                bytes.extend(imm32.to_le_bytes());
                            }
                        } else if directive == "quad" {
                            for value in values {
                                let imm64 = match value {
                                    ImmediateValue::Int(val) => *val as i64,
                                    ImmediateValue::Addr(val) => *val as i64,
                                };
                                bytes.extend(imm64.to_le_bytes());
                            }
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
        self.bytecode_with_debug_map().map(|(bytes, _)| bytes)
    }
}
