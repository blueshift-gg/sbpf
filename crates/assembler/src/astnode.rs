use {
    crate::{debuginfo::DebugInfo, errors::CompileError, parser::Token},
    sbpf_common::{inst_param::Number, instruction::Instruction},
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
        max: u64,
        span: Range<usize>,
    ) -> Result<(), CompileError> {
        let raw = value.to_i64();

        if raw < min || (raw >= 0 && (raw as u64) > max) {
            return Err(CompileError::OutOfRangeLiteral {
                span,
                custom_label: None,
            });
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
                            u8::MAX as u64,
                            vector_literal_span.clone(),
                        )?;
                    }
                }
                "short" => {
                    for value in values {
                        Self::validate_immediate_range(
                            value,
                            i16::MIN as i64,
                            u16::MAX as u64,
                            vector_literal_span.clone(),
                        )?;
                    }
                }
                "int" | "long" => {
                    for value in values {
                        Self::validate_immediate_range(
                            value,
                            i32::MIN as i64,
                            u32::MAX as u64,
                            vector_literal_span.clone(),
                        )?;
                    }
                }
                "quad" => {
                    for value in values {
                        Self::validate_immediate_range(
                            value,
                            i64::MIN,
                            u64::MAX,
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
    pub fn bytecode_with_debug_map(&self) -> Option<(Vec<u8>, HashMap<u64, DebugInfo>)> {
        match self {
            ASTNode::Instruction {
                instruction,
                offset,
            } => {
                // TODO: IMPLEMENT DEBUG INFO HANDLING AND DELETE THIS
                let mut debug_map = HashMap::new();
                let debug_info = DebugInfo::new(instruction.span.clone());

                debug_map.insert(*offset, debug_info);

                Some((instruction.to_bytes().unwrap(), debug_map))
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
    pub fn bytecode(&self) -> Option<Vec<u8>> {
        self.bytecode_with_debug_map().map(|(bytes, _)| bytes)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        sbpf_common::{instruction::Instruction, opcode::Opcode},
    };

    #[test]
    fn test_global_decl_get_entry_label() {
        let global = GlobalDecl {
            entry_label: "entrypoint".to_string(),
            span: 0..10,
        };
        assert_eq!(global.get_entry_label(), "entrypoint");
    }

    #[test]
    fn test_equ_decl_methods() {
        let equ = EquDecl {
            name: "MY_CONST".to_string(),
            value: Token::ImmediateValue(Number::Int(42), 5..7),
            span: 0..15,
        };
        assert_eq!(equ.get_name(), "MY_CONST");
        assert_eq!(equ.get_val(), Number::Int(42));
    }

    #[test]
    #[should_panic(expected = "Invalid Equ declaration")]
    fn test_equ_decl_invalid_value() {
        let equ = EquDecl {
            name: "INVALID".to_string(),
            value: Token::Identifier("not_a_number".to_string(), 0..5),
            span: 0..10,
        };
        let _ = equ.get_val(); // Should panic
    }

    #[test]
    fn test_rodata_get_size_ascii() {
        let rodata = ROData {
            name: "my_string".to_string(),
            args: vec![
                Token::Directive("ascii".to_string(), 0..5),
                Token::StringLiteral("Hello".to_string(), 6..13),
            ],
            span: 0..13,
        };
        assert_eq!(rodata.get_size(), 5);
    }

    #[test]
    fn test_rodata_get_size_byte() {
        let rodata = ROData {
            name: "my_bytes".to_string(),
            args: vec![
                Token::Directive("byte".to_string(), 0..4),
                Token::VectorLiteral(vec![Number::Int(1), Number::Int(2), Number::Int(3)], 5..14),
            ],
            span: 0..14,
        };
        assert_eq!(rodata.get_size(), 3);
    }

    #[test]
    fn test_rodata_get_size_short() {
        let rodata = ROData {
            name: "my_shorts".to_string(),
            args: vec![
                Token::Directive("short".to_string(), 0..5),
                Token::VectorLiteral(vec![Number::Int(1), Number::Int(2)], 6..12),
            ],
            span: 0..12,
        };
        assert_eq!(rodata.get_size(), 4); // 2 shorts * 2 bytes
    }

    #[test]
    fn test_rodata_get_size_int() {
        let rodata = ROData {
            name: "my_ints".to_string(),
            args: vec![
                Token::Directive("int".to_string(), 0..3),
                Token::VectorLiteral(vec![Number::Int(100)], 4..7),
            ],
            span: 0..7,
        };
        assert_eq!(rodata.get_size(), 4); // 1 int * 4 bytes
    }

    #[test]
    fn test_rodata_get_size_quad() {
        let rodata = ROData {
            name: "my_quads".to_string(),
            args: vec![
                Token::Directive("quad".to_string(), 0..4),
                Token::VectorLiteral(vec![Number::Int(1000)], 5..9),
            ],
            span: 0..9,
        };
        assert_eq!(rodata.get_size(), 8); // 1 quad * 8 bytes
    }

    #[test]
    fn test_rodata_verify_ascii() {
        let rodata = ROData {
            name: "str".to_string(),
            args: vec![
                Token::Directive("ascii".to_string(), 0..5),
                Token::StringLiteral("test".to_string(), 6..12),
            ],
            span: 0..12,
        };
        assert!(rodata.verify().is_ok());
    }

    #[test]
    fn test_rodata_verify_byte_valid() {
        let rodata = ROData {
            name: "bytes".to_string(),
            args: vec![
                Token::Directive("byte".to_string(), 0..4),
                Token::VectorLiteral(
                    vec![Number::Int(0), Number::Int(127), Number::Int(-128)],
                    5..15,
                ),
            ],
            span: 0..15,
        };
        assert!(rodata.verify().is_ok());
    }

    #[test]
    fn test_rodata_verify_byte_out_of_range() {
        let rodata = ROData {
            name: "bytes".to_string(),
            args: vec![
                Token::Directive("byte".to_string(), 0..4),
                Token::VectorLiteral(vec![Number::Int(256)], 5..10),
            ],
            span: 0..10,
        };
        assert!(rodata.verify().is_err());
    }

    #[test]
    fn test_rodata_verify_short_valid() {
        let rodata = ROData {
            name: "shorts".to_string(),
            args: vec![
                Token::Directive("short".to_string(), 0..5),
                Token::VectorLiteral(vec![Number::Int(32767), Number::Int(-32768)], 6..16),
            ],
            span: 0..16,
        };
        assert!(rodata.verify().is_ok());
    }

    #[test]
    fn test_rodata_verify_int_valid() {
        let rodata = ROData {
            name: "ints".to_string(),
            args: vec![
                Token::Directive("int".to_string(), 0..3),
                Token::VectorLiteral(vec![Number::Int(2147483647)], 4..14),
            ],
            span: 0..14,
        };
        assert!(rodata.verify().is_ok());
    }

    #[test]
    fn test_rodata_verify_quad_valid() {
        let rodata = ROData {
            name: "quads".to_string(),
            args: vec![
                Token::Directive("quad".to_string(), 0..4),
                Token::VectorLiteral(vec![Number::Int(9223372036854775807)], 5..20),
            ],
            span: 0..20,
        };
        assert!(rodata.verify().is_ok());
    }

    #[test]
    fn test_rodata_verify_invalid_directive() {
        let rodata = ROData {
            name: "invalid".to_string(),
            args: vec![
                Token::Directive("invalid".to_string(), 0..7),
                Token::VectorLiteral(vec![Number::Int(1)], 8..11),
            ],
            span: 0..11,
        };
        assert!(rodata.verify().is_err());
    }

    #[test]
    fn test_astnode_instruction_bytecode() {
        let inst = Instruction {
            opcode: Opcode::Exit,
            dst: None,
            src: None,
            off: None,
            imm: None,
            span: 0..4,
        };
        let node = ASTNode::Instruction {
            instruction: inst,
            offset: 0,
        };

        let bytecode = node.bytecode();
        assert!(bytecode.is_some());
        assert_eq!(bytecode.unwrap().len(), 8);
    }

    #[test]
    fn test_astnode_rodata_bytecode_ascii() {
        let rodata = ROData {
            name: "msg".to_string(),
            args: vec![
                Token::Directive("ascii".to_string(), 0..5),
                Token::StringLiteral("Hi".to_string(), 6..10),
            ],
            span: 0..10,
        };
        let node = ASTNode::ROData { rodata, offset: 0 };

        let bytecode = node.bytecode();
        assert!(bytecode.is_some());
        assert_eq!(bytecode.unwrap(), b"Hi");
    }

    #[test]
    fn test_astnode_rodata_bytecode_byte() {
        let rodata = ROData {
            name: "data".to_string(),
            args: vec![
                Token::Directive("byte".to_string(), 0..4),
                Token::VectorLiteral(vec![Number::Int(0x42), Number::Int(0x43)], 5..13),
            ],
            span: 0..13,
        };
        let node = ASTNode::ROData { rodata, offset: 0 };

        let bytecode = node.bytecode();
        assert!(bytecode.is_some());
        assert_eq!(bytecode.unwrap(), vec![0x42u8, 0x43u8]);
    }

    #[test]
    fn test_astnode_rodata_bytecode_short() {
        let rodata = ROData {
            name: "data".to_string(),
            args: vec![
                Token::Directive("short".to_string(), 0..5),
                Token::VectorLiteral(vec![Number::Int(0x1234)], 6..12),
            ],
            span: 0..12,
        };
        let node = ASTNode::ROData { rodata, offset: 0 };

        let bytecode = node.bytecode();
        assert!(bytecode.is_some());
        let bytes = bytecode.unwrap();
        assert_eq!(bytes.len(), 2);
        assert_eq!(i16::from_le_bytes([bytes[0], bytes[1]]), 0x1234);
    }

    #[test]
    fn test_astnode_rodata_bytecode_int() {
        let rodata = ROData {
            name: "data".to_string(),
            args: vec![
                Token::Directive("int".to_string(), 0..3),
                Token::VectorLiteral(vec![Number::Int(0x12345678)], 4..14),
            ],
            span: 0..14,
        };
        let node = ASTNode::ROData { rodata, offset: 0 };

        let bytecode = node.bytecode();
        assert!(bytecode.is_some());
        let bytes = bytecode.unwrap();
        assert_eq!(bytes.len(), 4);
    }

    #[test]
    fn test_astnode_rodata_bytecode_quad() {
        let rodata = ROData {
            name: "data".to_string(),
            args: vec![
                Token::Directive("quad".to_string(), 0..4),
                Token::VectorLiteral(vec![Number::Int(0x123456789ABCDEF0)], 5..21),
            ],
            span: 0..21,
        };
        let node = ASTNode::ROData { rodata, offset: 0 };

        let bytecode = node.bytecode();
        assert!(bytecode.is_some());
        let bytes = bytecode.unwrap();
        assert_eq!(bytes.len(), 8);
    }

    #[test]
    fn test_astnode_label_no_bytecode() {
        let node = ASTNode::Label {
            label: Label {
                name: "loop".to_string(),
                span: 0..4,
            },
            offset: 0,
        };
        assert!(node.bytecode().is_none());
    }

    #[test]
    fn test_astnode_directive_no_bytecode() {
        let node = ASTNode::Directive {
            directive: Directive {
                name: "section".to_string(),
                args: vec![],
                span: 0..7,
            },
        };
        assert!(node.bytecode().is_none());
    }
}
