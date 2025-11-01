use {
    crate::{debuginfo::DebugInfo, errors::CompileError, lexer::Token},
    sbpf_common::{
        inst_param::Number,
        instruction::Instruction,
        platform::BPFPlatform,
    },
    std::{collections::HashMap, ops::Range},
};

#[derive(Debug, Clone)]
pub enum ASTNode {
    // only present in AST
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
    // present in both AST and bytecode
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
    pub fn get_val(&self) -> Number {
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
        value: &Number,
        min: i64,
        max: i64,
        span: Range<usize>,
    ) -> Result<(), CompileError> {
        match value {
            Number::Int(val) => {
                if *val < min || *val > max {
                    return Err(CompileError::OutOfRangeLiteral {
                        span,
                        custom_label: None,
                    });
                }
            }
            Number::Addr(val) => {
                if *val < min || *val > max {
                    return Err(CompileError::OutOfRangeLiteral {
                        span,
                        custom_label: None,
                    });
                }
            }
        }
        Ok(())
    }

    pub fn get_size(&self) -> u64 {
        let size: u64;
        match (&self.args[0], &self.args[1]) {
            (Token::Directive(_, _), Token::StringLiteral(s, _)) => {
                size = s.len() as u64;
            }
            (Token::Directive(directive, _), Token::VectorLiteral(values, _)) => {
                match directive.as_str() {
                    "byte" => {
                        size = values.len() as u64;
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
        match (&self.args[0], &self.args[1]) {
            (Token::Directive(directive, directive_span), Token::StringLiteral(_, _)) => {
                if directive.as_str() != "ascii" {
                    return Err(CompileError::InvalidRODataDirective {
                        span: directive_span.clone(),
                        custom_label: None,
                    });
                }
            }
            (
                Token::Directive(directive, directive_span),
                Token::VectorLiteral(values, vector_literal_span),
            ) => match directive.as_str() {
                "byte" => {
                    for value in values {
                        Self::validate_immediate_range(
                            value,
                            i8::MIN as i64,
                            i8::MAX as i64,
                            vector_literal_span.clone(),
                        )?;
                    }
                }
                "short" => {
                    for value in values {
                        Self::validate_immediate_range(
                            value,
                            i16::MIN as i64,
                            i16::MAX as i64,
                            vector_literal_span.clone(),
                        )?;
                    }
                }
                "int" | "long" => {
                    for value in values {
                        Self::validate_immediate_range(
                            value,
                            i32::MIN as i64,
                            i32::MAX as i64,
                            vector_literal_span.clone(),
                        )?;
                    }
                }
                "quad" => {
                    for value in values {
                        Self::validate_immediate_range(
                            value,
                            i64::MIN,
                            i64::MAX,
                            vector_literal_span.clone(),
                        )?;
                    }
                }
                _ => {
                    return Err(CompileError::InvalidRODataDirective {
                        span: directive_span.clone(),
                        custom_label: None,
                    });
                }
            },
            _ => {
                return Err(CompileError::InvalidRodataDecl {
                    span: self.span.clone(),
                    custom_label: None,
                });
            }
        }
        Ok(())
    }
}

impl ASTNode {
    pub fn bytecode_with_debug_map<Platform: BPFPlatform>(&self) -> Option<(Vec<u8>, HashMap<u64, DebugInfo>)> {
        match self {
            ASTNode::Instruction {
                instruction,
                offset,
            } => {
                // TODO: IMPLEMENT DEBUG INFO HANDLING AND DELETE THIS
                let mut debug_map = HashMap::new();
                let debug_info = DebugInfo::new(instruction.span.clone());

                debug_map.insert(*offset, debug_info);

                Some((instruction.to_bytes::<Platform>().unwrap(), debug_map))
            }
            ASTNode::ROData {
                rodata: ROData { name: _, args, .. },
                ..
            } => {
                let mut bytes = Vec::new();
                let debug_map = HashMap::<u64, DebugInfo>::new();
                match (&args[0], &args[1]) {
                    (Token::Directive(_, _), Token::StringLiteral(str_literal, _)) => {
                        let str_bytes = str_literal.as_bytes().to_vec();
                        bytes.extend(str_bytes);
                    }
                    (Token::Directive(directive, _), Token::VectorLiteral(values, _)) => {
                        if directive == "byte" {
                            for value in values {
                                let imm8 = match value {
                                    Number::Int(val) => *val as i8,
                                    Number::Addr(val) => *val as i8,
                                };
                                bytes.extend(imm8.to_le_bytes());
                            }
                        } else if directive == "short" {
                            for value in values {
                                let imm16 = match value {
                                    Number::Int(val) => *val as i16,
                                    Number::Addr(val) => *val as i16,
                                };
                                bytes.extend(imm16.to_le_bytes());
                            }
                        } else if directive == "int" || directive == "long" {
                            for value in values {
                                let imm32 = match value {
                                    Number::Int(val) => *val as i32,
                                    Number::Addr(val) => *val as i32,
                                };
                                bytes.extend(imm32.to_le_bytes());
                            }
                        } else if directive == "quad" {
                            for value in values {
                                let imm64 = match value {
                                    Number::Int(val) => *val,
                                    Number::Addr(val) => *val,
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
            }
            _ => None,
        }
    }

    // Keep the old bytecode method for backward compatibility
    pub fn bytecode<Platform: BPFPlatform>(&self) -> Option<Vec<u8>> {
        self.bytecode_with_debug_map::<Platform>().map(|(bytes, _)| bytes)
    }
}
