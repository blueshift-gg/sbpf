use {
    crate::{
        ast::AST,
        astnode::{ASTNode, Directive, EquDecl, ExternDecl, GlobalDecl, Label, ROData, RodataDecl},
        bug,
        dynsym::{DynamicSymbolMap, RelDynMap},
        errors::CompileError,
        instruction::Instruction,
        lexer::{ImmediateValue, Op, Token},
        messages::*,
        section::{CodeSection, DataSection},
    },
    sbpf_common::opcode::Opcode,
    std::collections::HashMap,
};

// BPF_X flag: Converts immediate variant opcodes to register variant opcodes
const BPF_X: u8 = 0x08;

pub struct ParseResult {
    // TODO: parse result is basically 1. static part 2. dynamic part of the program
    pub code_section: CodeSection,

    pub data_section: DataSection,

    pub dynamic_symbols: DynamicSymbolMap,

    pub relocation_data: RelDynMap,

    // TODO: this can be removed and dynamic-ness should just be
    // determined by if there's any dynamic symbol
    pub prog_is_static: bool,
}

// for now, we only return one error per parse for simpler error handling
pub trait Parse {
    fn parse(tokens: &[Token]) -> Result<(Self, &[Token]), CompileError>
    where
        Self: Sized;
}

pub trait ParseWithConstMap {
    fn parse_with_constmap<'a>(
        tokens: &'a [Token],
        const_map: &HashMap<String, ImmediateValue>,
    ) -> Result<(Self, &'a [Token]), CompileError>
    where
        Self: Sized;
}

impl Parse for GlobalDecl {
    fn parse(tokens: &[Token]) -> Result<(Self, &[Token]), CompileError> {
        let Token::Directive(_, span) = &tokens[0] else {
            bug!("GlobalDecl not a valid directive")
        };
        if tokens.len() < 2 {
            return Err(CompileError::InvalidGlobalDecl {
                span: span.clone(),
                custom_label: None,
            });
        }
        match &tokens[1] {
            Token::Identifier(name, span) => Ok((
                GlobalDecl {
                    entry_label: name.clone(),
                    span: span.clone(),
                },
                &tokens[2..],
            )),
            _ => Err(CompileError::InvalidGlobalDecl {
                span: span.clone(),
                custom_label: None,
            }),
        }
    }
}

impl ParseWithConstMap for EquDecl {
    fn parse_with_constmap<'a>(
        tokens: &'a [Token],
        const_map: &HashMap<String, ImmediateValue>,
    ) -> Result<(Self, &'a [Token]), CompileError> {
        let Token::Directive(_, span) = &tokens[0] else {
            bug!("EquDecl not a valid directive")
        };
        if tokens.len() < 3 {
            return Err(CompileError::InvalidEquDecl {
                span: span.clone(),
                custom_label: Some(EXPECTS_MORE_OPERAND.to_string()),
            });
        }
        let (value, advance_token_num) = inline_and_fold_constant(tokens, const_map, 3);
        if let Some(value) = value {
            match (
                &tokens[1], &tokens[2],
                // third operand is folded to an immediate value
            ) {
                (
                    Token::Identifier(name, span),
                    Token::Comma(_),
                    // third operand is folded to an immediate value
                ) => Ok((
                    EquDecl {
                        name: name.clone(),
                        value: Token::ImmediateValue(value, span.clone()),
                        span: span.clone(),
                    },
                    &tokens[advance_token_num..],
                )),
                _ => Err(CompileError::InvalidEquDecl {
                    span: span.clone(),
                    custom_label: Some(EXPECTS_IDEN_COM_IMM.to_string()),
                }),
            }
        } else {
            Err(CompileError::InvalidEquDecl {
                span: span.clone(),
                custom_label: Some(EXPECTS_IDEN_COM_IMM.to_string()),
            })
        }
    }
}

impl Parse for ExternDecl {
    fn parse(tokens: &[Token]) -> Result<(Self, &[Token]), CompileError> {
        let Token::Directive(_, span) = &tokens[0] else {
            bug!("ExternDecl not a valid directive")
        };
        if tokens.len() < 2 {
            return Err(CompileError::InvalidExternDecl {
                span: span.clone(),
                custom_label: Some(EXPECTS_MORE_OPERAND.to_string()),
            });
        }
        let mut args = Vec::new();
        let mut i = 1;
        while i < tokens.len() {
            match &tokens[i] {
                Token::Identifier(name, span) => {
                    args.push(Token::Identifier(name.clone(), span.clone()));
                    i += 1;
                }
                _ => {
                    break;
                }
            }
        }
        //
        if args.is_empty() {
            Err(CompileError::InvalidExternDecl {
                span: span.clone(),
                custom_label: Some(EXPECTS_IDEN.to_string()),
            })
        } else {
            Ok((
                ExternDecl {
                    args,
                    span: span.clone(),
                },
                &tokens[i..],
            ))
        }
    }
}

impl Parse for ROData {
    fn parse(tokens: &[Token]) -> Result<(Self, &[Token]), CompileError> {
        let Token::Label(_, span) = &tokens[0] else {
            bug!("ROData not a valid directive")
        };
        if tokens.len() < 3 {
            return Err(CompileError::InvalidRodataDecl {
                span: span.clone(),
                custom_label: Some(EXPECTS_MORE_OPERAND.to_string()),
            });
        }

        let mut args = Vec::new();
        match (&tokens[0], &tokens[1], &tokens[2]) {
            (Token::Label(name, span), Token::Directive(_, _), Token::StringLiteral(_, _)) => {
                args.push(tokens[1].clone());
                args.push(tokens[2].clone());
                Ok((
                    ROData {
                        name: name.clone(),
                        args,
                        span: span.clone(),
                    },
                    &tokens[3..],
                ))
            }
            (Token::Label(name, span), Token::Directive(_, _), Token::ImmediateValue(val, _)) => {
                let mut data_vector = vec![val.clone()];
                let idx = parse_vector_literal(tokens, &mut data_vector, 3);
                args.push(tokens[1].clone());
                args.push(Token::VectorLiteral(data_vector, span.clone()));
                Ok((
                    ROData {
                        name: name.clone(),
                        args,
                        span: span.clone(),
                    },
                    &tokens[idx..],
                ))
            }
            _ => Err(CompileError::InvalidRodataDecl {
                span: span.clone(),
                custom_label: Some(EXPECTS_LABEL_DIR_STR.to_string()),
            }),
        }
    }
}

impl ParseWithConstMap for Instruction {
    fn parse_with_constmap<'a>(
        tokens: &'a [Token],
        const_map: &HashMap<String, ImmediateValue>,
    ) -> Result<(Self, &'a [Token]), CompileError> {
        let next_token_num;
        match &tokens[0] {
            Token::Opcode(opcode, span) => {
                let mut opcode = *opcode;
                let mut operands = Vec::new();
                match opcode {
                    Opcode::Lddw => {
                        if tokens.len() < 4 {
                            return Err(CompileError::InvalidInstruction {
                                //
                                instruction: opcode.to_string(), //
                                span: span.clone(),              //
                                custom_label: Some(EXPECTS_MORE_OPERAND.to_string()),
                            });
                        }
                        let (value, advance_token_num) =
                            inline_and_fold_constant(tokens, const_map, 3);
                        if let Some(value) = value {
                            match (
                                &tokens[1],
                                &tokens[2],
                                // Third operand is folded to an immediate value
                            ) {
                                (
                                    Token::Register(_, _),
                                    Token::Comma(_),
                                    // Third operand is folded to an immediate value
                                ) => {
                                    operands.push(tokens[1].clone());
                                    operands.push(Token::ImmediateValue(value, span.clone()));
                                }
                                _ => {
                                    return Err(CompileError::InvalidInstruction {
                                        //
                                        instruction: opcode.to_string(), //
                                        span: span.clone(),              //
                                        custom_label: Some(EXPECTS_REG_COM_IMM_OR_IDEN.to_string()),
                                    });
                                }
                            }
                            next_token_num = advance_token_num;
                        } else {
                            match (&tokens[1], &tokens[2], &tokens[3]) {
                                (
                                    Token::Register(_, _),
                                    Token::Comma(_),
                                    Token::Identifier(_, _),
                                ) => {
                                    operands.push(tokens[1].clone());
                                    operands.push(tokens[3].clone());
                                }
                                _ => {
                                    return Err(CompileError::InvalidInstruction {
                                        //
                                        instruction: opcode.to_string(), //
                                        span: span.clone(),              //
                                        custom_label: Some(EXPECTS_REG_COM_IMM_OR_IDEN.to_string()),
                                    });
                                }
                            }
                            next_token_num = 4;
                        }
                    }
                    Opcode::Ldxw | Opcode::Ldxh | Opcode::Ldxb | Opcode::Ldxdw => {
                        if tokens.len() < 8 {
                            return Err(CompileError::InvalidInstruction {
                                //
                                instruction: opcode.to_string(), //
                                span: span.clone(),              //
                                custom_label: Some(EXPECTS_MORE_OPERAND.to_string()),
                            });
                        }
                        let (value, advance_token_num) =
                            inline_and_fold_constant(tokens, const_map, 6);
                        if let Some(value) = value {
                            match (
                                &tokens[1],
                                &tokens[2],
                                &tokens[3],
                                &tokens[4],
                                &tokens[5],
                                // Sixth operand is folded to an immediate value
                                &tokens[advance_token_num],
                            ) {
                                (
                                    Token::Register(_, _),
                                    Token::Comma(_),
                                    Token::LeftBracket(_),
                                    Token::Register(_, _),
                                    Token::BinaryOp(_, _),
                                    // Sixth operand is folded to an immediate value
                                    Token::RightBracket(_),
                                ) => {
                                    operands.push(tokens[1].clone());
                                    operands.push(tokens[4].clone());
                                    operands.push(Token::ImmediateValue(value, span.clone()));
                                }
                                _ => {
                                    return Err(CompileError::InvalidInstruction {
                                        //
                                        instruction: opcode.to_string(), //
                                        span: span.clone(),              //
                                        custom_label: Some(
                                            EXPECTS_REG_COM_LB_REG_BIOP_IMM_RB.to_string(),
                                        ),
                                    });
                                }
                            }
                            next_token_num = advance_token_num + 1;
                        } else {
                            return Err(CompileError::InvalidInstruction {
                                //
                                instruction: opcode.to_string(), //
                                span: span.clone(),              //
                                custom_label: Some(EXPECTS_REG_COM_LB_REG_BIOP_IMM_RB.to_string()),
                            });
                        }
                    }
                    Opcode::Stw | Opcode::Sth | Opcode::Stb | Opcode::Stdw => {
                        if tokens.len() < 8 {
                            return Err(CompileError::InvalidInstruction {
                                //
                                instruction: opcode.to_string(), //
                                span: span.clone(),              //
                                custom_label: Some(EXPECTS_MORE_OPERAND.to_string()),
                            });
                        }
                        let (value, advance_token_num) =
                            inline_and_fold_constant(tokens, const_map, 4);
                        if let Some(value) = value {
                            // Now we need to fold the second immediate value (after the comma)
                            let (value2, advance_token_num2) =
                                inline_and_fold_constant(tokens, const_map, advance_token_num + 2);
                            if let Some(value2) = value2 {
                                match (
                                    &tokens[1],
                                    &tokens[2],
                                    &tokens[3],
                                    // Fourth operand is folded to an immediate value
                                    &tokens[advance_token_num],
                                    &tokens[advance_token_num + 1],
                                    // Sixth operand is also folded to an immediate value
                                ) {
                                    (
                                        Token::LeftBracket(_),
                                        Token::Register(_, _),
                                        Token::BinaryOp(_, _),
                                        // Fourth operand is folded to an immediate value
                                        Token::RightBracket(_),
                                        Token::Comma(_),
                                    ) => {
                                        operands.push(tokens[2].clone());
                                        operands.push(Token::ImmediateValue(value2, span.clone()));
                                        operands.push(Token::ImmediateValue(value, span.clone()));
                                    }
                                    _ => {
                                        return Err(CompileError::InvalidInstruction {
                                            //
                                            instruction: opcode.to_string(), //
                                            span: span.clone(),              //
                                            custom_label: Some(
                                                EXPECTS_LB_REG_BIOP_IMM_RB_COM_IMM.to_string(),
                                            ),
                                        });
                                    }
                                }
                                next_token_num = advance_token_num2;
                            } else {
                                return Err(CompileError::InvalidInstruction {
                                    //
                                    instruction: opcode.to_string(), //
                                    span: span.clone(),              //
                                    custom_label: Some(
                                        EXPECTS_LB_REG_BIOP_IMM_RB_COM_IMM.to_string(),
                                    ),
                                });
                            }
                        } else {
                            return Err(CompileError::InvalidInstruction {
                                //
                                instruction: opcode.to_string(), //
                                span: span.clone(),              //
                                custom_label: Some(EXPECTS_LB_REG_BIOP_IMM_RB_COM_IMM.to_string()),
                            });
                        }
                    }
                    Opcode::Stxb | Opcode::Stxh | Opcode::Stxw | Opcode::Stxdw => {
                        if tokens.len() < 8 {
                            return Err(CompileError::InvalidInstruction {
                                //
                                instruction: opcode.to_string(), //
                                span: span.clone(),              //
                                custom_label: Some(EXPECTS_MORE_OPERAND.to_string()),
                            });
                        }
                        let (value, advance_token_num) =
                            inline_and_fold_constant(tokens, const_map, 4);
                        if let Some(value) = value {
                            match (
                                &tokens[1],
                                &tokens[2],
                                &tokens[3],
                                // Fourth operand is folded to an immediate value
                                &tokens[advance_token_num],
                                &tokens[advance_token_num + 1],
                                &tokens[advance_token_num + 2],
                            ) {
                                (
                                    Token::LeftBracket(_),
                                    Token::Register(_, _),
                                    Token::BinaryOp(_, _),
                                    // Fourth operand is folded to an immediate value
                                    Token::RightBracket(_),
                                    Token::Comma(_),
                                    Token::Register(_, _),
                                ) => {
                                    operands.push(tokens[2].clone());
                                    operands.push(Token::ImmediateValue(value, span.clone()));
                                    operands.push(tokens[advance_token_num + 2].clone());
                                }
                                _ => {
                                    return Err(CompileError::InvalidInstruction {
                                        //
                                        instruction: opcode.to_string(), //
                                        span: span.clone(),              //
                                        custom_label: Some(
                                            EXPECTS_LB_REG_BIOP_IMM_RB_COM_REG.to_string(),
                                        ),
                                    });
                                }
                            }
                            next_token_num = advance_token_num + 3;
                        } else {
                            return Err(CompileError::InvalidInstruction {
                                //
                                instruction: opcode.to_string(), //
                                span: span.clone(),              //
                                custom_label: Some(EXPECTS_LB_REG_BIOP_IMM_RB_COM_REG.to_string()),
                            });
                        }
                    }
                    Opcode::Add32Imm
                    | Opcode::Sub32Imm
                    | Opcode::Mul32Imm
                    | Opcode::Div32Imm
                    | Opcode::Or32Imm
                    | Opcode::And32Imm
                    | Opcode::Lsh32Imm
                    | Opcode::Rsh32Imm
                    | Opcode::Mod32Imm
                    | Opcode::Xor32Imm
                    | Opcode::Mov32Imm
                    | Opcode::Arsh32Imm
                    | Opcode::Lmul32Imm
                    | Opcode::Udiv32Imm
                    | Opcode::Urem32Imm
                    | Opcode::Sdiv32Imm
                    | Opcode::Srem32Imm
                    | Opcode::Add64Imm
                    | Opcode::Sub64Imm
                    | Opcode::Mul64Imm
                    | Opcode::Div64Imm
                    | Opcode::Or64Imm
                    | Opcode::And64Imm
                    | Opcode::Lsh64Imm
                    | Opcode::Rsh64Imm
                    | Opcode::Mod64Imm
                    | Opcode::Xor64Imm
                    | Opcode::Mov64Imm
                    | Opcode::Arsh64Imm
                    | Opcode::Lmul64Imm
                    | Opcode::Uhmul64Imm
                    | Opcode::Udiv64Imm
                    | Opcode::Urem64Imm
                    | Opcode::Sdiv64Imm
                    | Opcode::Srem64Imm => {
                        if tokens.len() < 4 {
                            return Err(CompileError::InvalidInstruction {
                                //
                                instruction: opcode.to_string(), //
                                span: span.clone(),              //
                                custom_label: Some(EXPECTS_MORE_OPERAND.to_string()),
                            });
                        }
                        let (value, advance_token_num) =
                            inline_and_fold_constant(tokens, const_map, 3);
                        if let Some(value) = value {
                            match (
                                &tokens[1],
                                &tokens[2],
                                // Third operand is folded to an immediate value
                            ) {
                                (
                                    Token::Register(_, _),
                                    Token::Comma(_),
                                    // Third operand is folded to an immediate value
                                ) => {
                                    // Opcode already represents the immediate variant (no conversion needed)
                                    operands.push(tokens[1].clone());
                                    operands.push(Token::ImmediateValue(value, span.clone()));
                                }
                                _ => {
                                    return Err(CompileError::InvalidInstruction {
                                        //
                                        instruction: opcode.to_string(), //
                                        span: span.clone(),              //
                                        custom_label: Some(EXPECTS_REG_COM_IMM.to_string()),
                                    });
                                }
                            }
                            next_token_num = advance_token_num;
                        } else {
                            match (&tokens[1], &tokens[2], &tokens[3]) {
                                (Token::Register(_, _), Token::Comma(_), Token::Register(_, _)) => {
                                    // Convert immediate variant to register variant using BPF_X flag
                                    let new_opcode = Into::<u8>::into(opcode) | BPF_X;
                                    opcode = new_opcode.try_into().map_err(|e| {
                                        CompileError::BytecodeError {
                                            error: format!("Invalid opcode 0x{:02x}: {}", new_opcode, e),
                                            span: span.clone(),
                                            custom_label: None,
                                        }
                                    })?;
                                    operands.push(tokens[1].clone());
                                    operands.push(tokens[3].clone());
                                }
                                _ => {
                                    return Err(CompileError::InvalidInstruction {
                                        //
                                        instruction: opcode.to_string(), //
                                        span: span.clone(),              //
                                        custom_label: Some(EXPECTS_REG_COM_REG.to_string()),
                                    });
                                }
                            }
                            next_token_num = 4;
                        }
                    }
                    Opcode::Be | Opcode::Le => {
                        if tokens.len() < 4 {
                            return Err(CompileError::InvalidInstruction {
                                //
                                instruction: opcode.to_string(), //
                                span: span.clone(),              //
                                custom_label: Some(EXPECTS_MORE_OPERAND.to_string()),
                            });
                        }
                        let (value, advance_token_num) =
                            inline_and_fold_constant(tokens, const_map, 3);
                        if let Some(value) = value {
                            match (
                                &tokens[1],
                                &tokens[2],
                                // Third operand is folded to an immediate value
                            ) {
                                (
                                    Token::Register(_, _),
                                    Token::Comma(_),
                                    // Third operand is folded to an immediate value
                                ) => {
                                    operands.push(tokens[1].clone());
                                    operands.push(Token::ImmediateValue(value, span.clone()));
                                }
                                _ => {
                                    return Err(CompileError::InvalidInstruction {
                                        //
                                        instruction: opcode.to_string(), //
                                        span: span.clone(),              //
                                        custom_label: Some(EXPECTS_REG_COM_IMM.to_string()),
                                    });
                                }
                            }
                            next_token_num = advance_token_num;
                        } else {
                            return Err(CompileError::InvalidInstruction {
                                //
                                instruction: opcode.to_string(), //
                                span: span.clone(),              //
                                custom_label: Some(EXPECTS_REG_COM_IMM.to_string()),
                            });
                        }
                    }
                    Opcode::JeqImm
                    | Opcode::JgtImm
                    | Opcode::JgeImm
                    | Opcode::JltImm
                    | Opcode::JleImm
                    | Opcode::JsetImm
                    | Opcode::JneImm
                    | Opcode::JsgtImm
                    | Opcode::JsgeImm
                    | Opcode::JsltImm
                    | Opcode::JsleImm => {
                        if tokens.len() < 6 {
                            return Err(CompileError::InvalidInstruction {
                                //
                                instruction: opcode.to_string(), //
                                span: span.clone(),              //
                                custom_label: Some(EXPECTS_MORE_OPERAND.to_string()),
                            });
                        }
                        let (value, advance_token_num) =
                            inline_and_fold_constant(tokens, const_map, 3);
                        if let Some(value) = value {
                            let (jump_val, jump_val_advance_token_num) =
                                inline_and_fold_constant(tokens, const_map, advance_token_num + 1);
                            if let Some(jump_val) = jump_val {
                                match (
                                    &tokens[1],
                                    &tokens[2],
                                    // Third operand is folded to an immediate value
                                    &tokens[advance_token_num],
                                    // Fifth operand is folded to an immediate value
                                ) {
                                    (
                                        Token::Register(_, _),
                                        Token::Comma(_),
                                        // Third operand is folded to an immediate value
                                        Token::Comma(_),
                                        // Fifth operand is folded to an immediate value
                                    ) => {
                                        // Opcode is already the Imm variant, no conversion needed
                                        operands.push(tokens[1].clone());
                                        operands.push(Token::ImmediateValue(value, span.clone()));
                                        operands
                                            .push(Token::ImmediateValue(jump_val, span.clone()));
                                    }
                                    _ => {
                                        return Err(CompileError::InvalidInstruction {
                                            instruction: opcode.to_string(),
                                            span: span.clone(),
                                            custom_label: Some(
                                                EXPECTS_REG_COM_IMM_COM_IMM_OR_IDEN.to_string(),
                                            ),
                                        });
                                    }
                                }
                                next_token_num = jump_val_advance_token_num;
                            } else {
                                match (
                                    &tokens[1],
                                    &tokens[2],
                                    // Third operand is folded to an immediate value
                                    &tokens[advance_token_num],
                                    &tokens[advance_token_num + 1],
                                ) {
                                    (
                                        Token::Register(_, _),
                                        Token::Comma(_),
                                        // Third operand is folded to an immediate value
                                        Token::Comma(_),
                                        Token::Identifier(_, _),
                                    ) => {
                                        // Opcode is already the Imm variant, no conversion needed
                                        operands.push(tokens[1].clone());
                                        operands.push(Token::ImmediateValue(value, span.clone()));
                                        operands.push(tokens[advance_token_num + 1].clone());
                                    }
                                    _ => {
                                        return Err(CompileError::InvalidInstruction {
                                            //
                                            instruction: opcode.to_string(), //
                                            span: span.clone(),              //
                                            custom_label: Some(
                                                EXPECTS_REG_COM_IMM_COM_IMM_OR_IDEN.to_string(),
                                            ),
                                        });
                                    }
                                }
                                next_token_num = advance_token_num + 2;
                            }
                        } else {
                            let (jump_val, jump_val_advance_token_num) =
                                inline_and_fold_constant(tokens, const_map, advance_token_num + 1);
                            if let Some(jump_val) = jump_val {
                                match (
                                    &tokens[1], &tokens[2], &tokens[3],
                                    &tokens[4],
                                    // Fifth operand is folded to an immediate value
                                ) {
                                    (
                                        Token::Register(_, _),
                                        Token::Comma(_),
                                        Token::Register(_, _),
                                        Token::Comma(_),
                                        // Fifth operand is folded to an immediate value
                                    ) => {
                                        // Convert Imm variant to Reg variant using BPF_X flag
                                        let new_opcode = Into::<u8>::into(opcode) | BPF_X;
                                        opcode = new_opcode.try_into().map_err(|e| {
                                            CompileError::BytecodeError {
                                                error: format!("Invalid opcode 0x{:02x}: {}", new_opcode, e),
                                                span: span.clone(),
                                                custom_label: None,
                                            }
                                        })?;
                                        operands.push(tokens[1].clone());
                                        operands.push(tokens[3].clone());
                                        operands
                                            .push(Token::ImmediateValue(jump_val, span.clone()));
                                    }
                                    _ => {
                                        return Err(CompileError::InvalidInstruction {
                                            //
                                            instruction: opcode.to_string(), //
                                            span: span.clone(),              //
                                            custom_label: Some(
                                                EXPECTS_REG_COM_IMM_COM_IMM_OR_IDEN.to_string(),
                                            ),
                                        });
                                    }
                                }
                                next_token_num = jump_val_advance_token_num;
                            } else {
                                match (&tokens[1], &tokens[2], &tokens[3], &tokens[4], &tokens[5]) {
                                    (
                                        Token::Register(_, _),
                                        Token::Comma(_),
                                        Token::Register(_, _),
                                        Token::Comma(_),
                                        Token::Identifier(_, _),
                                    ) => {
                                        // Convert Imm variant to Reg variant using BPF_X flag
                                        let new_opcode = Into::<u8>::into(opcode) | BPF_X;
                                        opcode = new_opcode.try_into().map_err(|e| {
                                            CompileError::BytecodeError {
                                                error: format!("Invalid opcode 0x{:02x}: {}", new_opcode, e),
                                                span: span.clone(),
                                                custom_label: None,
                                            }
                                        })?;
                                        operands.push(tokens[1].clone());
                                        operands.push(tokens[3].clone());
                                        operands.push(tokens[5].clone());
                                    }
                                    _ => {
                                        return Err(CompileError::InvalidInstruction {
                                            //
                                            instruction: opcode.to_string(), //
                                            span: span.clone(),              //
                                            custom_label: Some(
                                                EXPECTS_REG_COM_IMM_COM_IMM_OR_IDEN.to_string(),
                                            ),
                                        });
                                    }
                                }
                                next_token_num = 6;
                            }
                        }
                    }
                    Opcode::Neg32 | Opcode::Neg64 => {
                        if tokens.len() < 2 {
                            return Err(CompileError::InvalidInstruction {
                                //
                                instruction: opcode.to_string(), //
                                span: span.clone(),              //
                                custom_label: Some(EXPECTS_MORE_OPERAND.to_string()),
                            });
                        }
                        match &tokens[1] {
                            Token::Register(_, _) => {
                                operands.push(tokens[1].clone());
                            }
                            _ => {
                                return Err(CompileError::InvalidInstruction {
                                    //
                                    instruction: opcode.to_string(), //
                                    span: span.clone(),              //
                                    custom_label: Some(EXPECTS_REG.to_string()),
                                });
                            }
                        }
                        next_token_num = 2;
                    }
                    Opcode::Ja => {
                        if tokens.len() < 2 {
                            return Err(CompileError::InvalidInstruction {
                                //
                                instruction: opcode.to_string(), //
                                span: span.clone(),              //
                                custom_label: Some(EXPECTS_MORE_OPERAND.to_string()),
                            });
                        }
                        let (value, advance_token_num) =
                            inline_and_fold_constant(tokens, const_map, 1);
                        if let Some(value) = value {
                            operands.push(Token::ImmediateValue(value, span.clone()));
                            next_token_num = advance_token_num;
                        } else {
                            match &tokens[1] {
                                Token::Identifier(_, _) => {
                                    operands.push(tokens[1].clone());
                                }
                                _ => {
                                    return Err(CompileError::InvalidInstruction {
                                        //
                                        instruction: opcode.to_string(), //
                                        span: span.clone(),              //
                                        custom_label: Some(EXPECTS_IDEN.to_string()),
                                    });
                                }
                            }
                            next_token_num = 2;
                        }
                    }
                    Opcode::Call => {
                        if tokens.len() < 2 {
                            return Err(CompileError::InvalidInstruction {
                                //
                                instruction: opcode.to_string(), //
                                span: span.clone(),              //
                                custom_label: Some(EXPECTS_MORE_OPERAND.to_string()),
                            });
                        }
                        match &tokens[1] {
                            Token::Identifier(_, _) => {
                                operands.push(tokens[1].clone());
                            }
                            _ => {
                                return Err(CompileError::InvalidInstruction {
                                    //
                                    instruction: opcode.to_string(), //
                                    span: span.clone(),              //
                                    custom_label: Some(EXPECTS_IDEN.to_string()),
                                });
                            }
                        }
                        next_token_num = 2;
                    }
                    Opcode::Callx => {
                        if tokens.len() < 2 {
                            return Err(CompileError::InvalidInstruction {
                                //
                                instruction: opcode.to_string(), //
                                span: span.clone(),              //
                                custom_label: Some(EXPECTS_MORE_OPERAND.to_string()),
                            });
                        }
                        match &tokens[1] {
                            Token::Register(_, _) => {
                                operands.push(tokens[1].clone());
                            }
                            _ => {
                                return Err(CompileError::InvalidInstruction {
                                    //
                                    instruction: opcode.to_string(), //
                                    span: span.clone(),              //
                                    custom_label: Some(EXPECTS_IDEN.to_string()),
                                });
                            }
                        }
                        next_token_num = 2;
                    }
                    Opcode::Exit => {
                        next_token_num = 1;
                    }
                    _ => {
                        bug!("invalid opcode: {}", opcode.to_str());
                    }
                }
                Ok((
                    Instruction {
                        opcode,
                        operands,
                        span: span.clone(),
                    },
                    &tokens[next_token_num..],
                ))
            }
            _ => {
                bug!("invalid instruction");
            }
        }
    }
}

fn parse_vector_literal(
    tokens: &[Token],
    stack: &mut Vec<ImmediateValue>,
    start_idx: usize,
) -> usize {
    let mut idx = start_idx;
    while idx < tokens.len() - 1 {
        match (&tokens[idx], &tokens[idx + 1]) {
            (Token::Comma(_), Token::ImmediateValue(val, _)) => {
                stack.push(val.clone());
                idx += 2;
            }
            _ => {
                break;
            }
        }
    }
    idx
}

fn fold_top(stack: &mut Vec<Token>) {
    if stack.len() < 3 {
        return;
    }

    if let (
        Token::ImmediateValue(val1, _),
        Token::BinaryOp(op, _),
        Token::ImmediateValue(val2, span),
    ) = (
        stack[stack.len() - 3].clone(),
        stack[stack.len() - 2].clone(),
        stack[stack.len() - 1].clone(),
    ) {
        let result = match op {
            Op::Add => val1.clone() + val2.clone(),
            Op::Sub => val1.clone() - val2.clone(),
            Op::Mul => val1.clone() * val2.clone(),
            Op::Div => val1.clone() / val2.clone(),
        };
        stack.pop();
        stack.pop();
        stack.pop();
        stack.push(Token::ImmediateValue(result, span));
    }
}

fn inline_and_fold_constant(
    tokens: &[Token],
    const_map: &std::collections::HashMap<String, ImmediateValue>,
    start_idx: usize,
) -> (Option<ImmediateValue>, usize) {
    inline_and_fold_constant_with_map(tokens, Some(const_map), start_idx)
}

fn inline_and_fold_constant_with_map(
    tokens: &[Token],
    const_map: Option<&std::collections::HashMap<String, ImmediateValue>>,
    start_idx: usize,
) -> (Option<ImmediateValue>, usize) {
    let mut stack: Vec<Token> = Vec::new();
    let mut expect_number = true;
    let mut idx = start_idx;

    while idx < tokens.len() {
        match &tokens[idx] {
            Token::ImmediateValue(val, span) if expect_number => {
                stack.push(Token::ImmediateValue(val.clone(), span.clone()));
                expect_number = false;

                // Immediately fold * / if top
                if stack.len() > 2
                    && let Token::BinaryOp(op, _) = &stack[stack.len() - 2]
                    && matches!(op, Op::Mul | Op::Div)
                {
                    fold_top(&mut stack);
                }
            }

            Token::Identifier(name, span) if expect_number => {
                if let Some(const_map) = const_map {
                    if let Some(val) = const_map.get(name) {
                        stack.push(Token::ImmediateValue(val.clone(), span.clone()));
                        expect_number = false;

                        if stack.len() > 2
                            && let Token::BinaryOp(op, _) = &stack[stack.len() - 2]
                            && matches!(op, Op::Mul | Op::Div)
                        {
                            fold_top(&mut stack);
                        }
                    } else {
                        return (None, idx);
                    }
                } else {
                    // error out would be better here
                    return (None, idx);
                }
            }

            Token::BinaryOp(op, span) => {
                match op {
                    Op::Sub if expect_number => {
                        // unary minus → 0 - expr
                        stack.push(Token::ImmediateValue(ImmediateValue::Int(0), span.clone()));
                        stack.push(Token::BinaryOp(Op::Sub, span.clone()));
                    }
                    _ => {
                        stack.push(Token::BinaryOp(op.clone(), span.clone()));
                    }
                }
                expect_number = true;
            }

            Token::LeftParen(span) => {
                // Parse inside parentheses
                let (inner_val, new_idx) =
                    inline_and_fold_constant_with_map(tokens, const_map, idx + 1);
                idx = new_idx;
                if let Some(v) = inner_val {
                    stack.push(Token::ImmediateValue(v, span.clone()));
                    expect_number = false;

                    if stack.len() > 2
                        && let Token::BinaryOp(op, _) = &stack[stack.len() - 2]
                        && matches!(op, Op::Mul | Op::Div)
                    {
                        fold_top(&mut stack);
                    }
                } else {
                    return (None, idx);
                }
                continue; // skip normal idx++
            }

            Token::RightParen(_) => {
                // fold remaining + and -
                while stack.len() > 2 {
                    fold_top(&mut stack);
                }
                if let Token::ImmediateValue(v, _) = &stack[0] {
                    return (Some(v.clone()), idx + 1);
                } else {
                    return (None, idx + 1);
                }
            }

            _ => {
                // Unexpected token, stop
                break;
            }
        }
        idx += 1;
    }

    // Final fold at the end of expression
    while stack.len() > 2 {
        fold_top(&mut stack);
    }

    if let Some(Token::ImmediateValue(v, _)) = stack.pop() {
        (Some(v), idx)
    } else {
        (None, idx)
    }
}

pub fn parse_tokens(mut tokens: &[Token]) -> Result<ParseResult, Vec<CompileError>> {
    let mut ast = AST::new();

    let mut rodata_phase = false;
    let mut accum_offset = 0;
    let mut rodata_accum_offset = 0;
    let mut const_map = HashMap::<String, ImmediateValue>::new();
    let mut label_spans = HashMap::<String, std::ops::Range<usize>>::new();
    let mut errors = Vec::new();

    while !tokens.is_empty() {
        match &tokens[0] {
            Token::Directive(name, span) => match name.as_str() {
                "global" | "globl" => match GlobalDecl::parse(tokens) {
                    Ok((node, rest)) => {
                        ast.entry_label = Some(node.get_entry_label());
                        ast.nodes.push(ASTNode::GlobalDecl { global_decl: node });
                        tokens = rest;
                    }
                    Err(e) => {
                        errors.push(e);
                        tokens = &tokens[1..];
                    }
                },
                "extern" => match ExternDecl::parse(tokens) {
                    Ok((node, rest)) => {
                        ast.nodes.push(ASTNode::ExternDecl { extern_decl: node });
                        tokens = rest;
                    }
                    Err(e) => {
                        errors.push(e);
                        tokens = &tokens[1..];
                    }
                },
                "text" => {
                    rodata_phase = false;
                    tokens = &tokens[1..];
                }
                "rodata" => {
                    ast.nodes.push(ASTNode::RodataDecl {
                        rodata_decl: RodataDecl { span: span.clone() },
                    });
                    rodata_phase = true;
                    tokens = &tokens[1..];
                }
                "equ" => match EquDecl::parse_with_constmap(tokens, &const_map) {
                    Ok((node, rest)) => {
                        const_map.insert(node.get_name(), node.get_val());
                        ast.nodes.push(ASTNode::EquDecl { equ_decl: node });
                        tokens = rest;
                    }
                    Err(e) => {
                        errors.push(e);
                        tokens = &tokens[1..];
                    }
                },
                "section" => {
                    ast.nodes.push(ASTNode::Directive {
                        directive: Directive {
                            name: name.clone(),
                            args: Vec::new(),
                            span: span.clone(),
                        },
                    });
                    tokens = &tokens[1..];
                }
                _ => {
                    errors.push(CompileError::InvalidDirective {
                        directive: name.clone(),
                        span: span.clone(),
                        custom_label: None,
                    });
                    tokens = &tokens[1..];
                }
            },
            Token::Label(name, span) => {
                if rodata_phase {
                    match ROData::parse(tokens) {
                        Ok((rodata, rest)) => {
                            if label_spans.contains_key(name) {
                                let original_span =
                                    label_spans.get(name).cloned().unwrap_or(span.clone());
                                errors.push(CompileError::DuplicateLabel {
                                    label: name.clone(),
                                    span: span.clone(),
                                    original_span,
                                    custom_label: Some(LABEL_REDEFINED.to_string()),
                                });
                            } else {
                                label_spans.insert(name.clone(), span.clone());
                                if let Err(e) = rodata.verify() {
                                    errors.push(e);
                                }
                            }
                            let rodata_size = rodata.get_size();
                            ast.rodata_nodes.push(ASTNode::ROData {
                                rodata,
                                offset: rodata_accum_offset,
                            });
                            rodata_accum_offset += rodata_size;
                            tokens = rest;
                        }
                        Err(e) => {
                            errors.push(e);
                            tokens = &tokens[1..];
                        }
                    }
                } else {
                    if label_spans.contains_key(name) {
                        let original_span = label_spans.get(name).cloned().unwrap_or(span.clone());
                        errors.push(CompileError::DuplicateLabel {
                            label: name.clone(),
                            span: span.clone(),
                            original_span,
                            custom_label: Some(LABEL_REDEFINED.to_string()),
                        });
                    } else {
                        label_spans.insert(name.clone(), span.clone());
                    }
                    ast.nodes.push(ASTNode::Label {
                        label: Label {
                            name: name.clone(),
                            span: span.clone(),
                        },
                        offset: accum_offset,
                    });
                    tokens = &tokens[1..];
                }
            }
            Token::Opcode(_, _) => match Instruction::parse_with_constmap(tokens, &const_map) {
                Ok((inst, rest)) => {
                    let offset = accum_offset;
                    accum_offset += inst.get_size();
                    ast.nodes.push(ASTNode::Instruction {
                        instruction: inst,
                        offset,
                    });
                    tokens = rest;
                }
                Err(e) => {
                    errors.push(e);
                    tokens = &tokens[1..];
                }
            },
            _ => {
                tokens = &tokens[1..];
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    ast.set_text_size(accum_offset);
    ast.set_rodata_size(rodata_accum_offset);

    let parse_result = ast.build_program();
    if let Ok(parse_result) = parse_result {
        Ok(parse_result)
    } else {
        Err(parse_result.err().unwrap())
    }
}
