use crate::lexer::Op;
use crate::opcode::Opcode;
use crate::lexer::{Token, ImmediateValue};
use crate::section::{CodeSection, DataSection};
use crate::astnode::{ASTNode, Directive, GlobalDecl, EquDecl, ExternDecl, RodataDecl, Label, Instruction, ROData};
use crate::dynsym::{DynamicSymbolMap, RelDynMap, RelocationType};
use num_traits::FromPrimitive;
use std::collections::HashMap;
use crate::errors::CompileError;
use crate::messages::*;
use crate::bug;

pub struct Parser<> {
    tokens: Vec<Token>,

    pub m_prog_is_static: bool,
    pub m_accum_offset: u64,

    // TODO: consolidate all temporary parsing related informaion
    m_const_map: HashMap<String, ImmediateValue>,
    m_label_offsets: HashMap<String, u64>,
    m_label_spans: HashMap<String, std::ops::Range<usize>>,

    // TODO: consolidate all dynamic symbol information to one big map
    m_entry_label: Option<String>,
    m_dynamic_symbols: DynamicSymbolMap,
    m_rel_dyns: RelDynMap,

    m_rodata_size: u64,
}

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
        where Self: Sized;
}

pub trait ParseWithConstMap {
    fn parse_with_constmap<'a>(tokens: &'a [Token], const_map: &HashMap<String, ImmediateValue>) -> Result<(Self, &'a [Token]), CompileError>
        where Self: Sized;
}

impl Parse for GlobalDecl {
    fn parse(tokens: &[Token]) -> Result<(Self, &[Token]), CompileError> {
        let Token::Directive(_, span) = &tokens[0] else { bug!("GlobalDecl not a valid directive") };
        if tokens.len() < 2 {
            return Err(CompileError::InvalidGlobalDecl { span: span.clone(), custom_label: None });
        }
        match &tokens[1] {
            Token::Identifier(name, span) => Ok((
                GlobalDecl {
                    entry_label: name.clone(), 
                    span: span.clone()
                },
                &tokens[2..])),
            _ => Err(CompileError::InvalidGlobalDecl { span: span.clone(), custom_label: None }),
        }
    }
}

impl ParseWithConstMap for EquDecl {
    fn parse_with_constmap<'a>(tokens: &'a [Token], const_map: &HashMap<String, ImmediateValue>) -> Result<(Self, &'a [Token]), CompileError> {
        let Token::Directive(_, span) = &tokens[0] else { bug!("EquDecl not a valid directive") };
        if tokens.len() < 3 {
            return Err(CompileError::InvalidEquDecl { span: span.clone(), custom_label: Some(EXPECTS_MORE_OPERAND.to_string()) });
        }
        let (value, advance_token_num) = inline_and_fold_constant(tokens, const_map, 3);
        if let Some(value) = value {
            match (
                &tokens[1],
                &tokens[2],
                // third operand is folded to an immediate value
            ) {
                (
                    Token::Identifier(name, span),
                    Token::Comma(_),
                    // third operand is folded to an immediate value
                ) => {
                    Ok((
                        EquDecl {
                            name: name.clone(),
                            value: Token::ImmediateValue(value, span.clone()),
                            span: span.clone()
                        },
                        &tokens[advance_token_num..]
                    ))
                }
                _ => Err(CompileError::InvalidEquDecl { span: span.clone(), custom_label: Some(EXPECTS_IDEN_COM_IMM.to_string()) }),
            }
        } else {
            Err(CompileError::InvalidEquDecl { span: span.clone(), custom_label: Some(EXPECTS_IDEN_COM_IMM.to_string()) })
        }

    }
}

impl Parse for ExternDecl {
    fn parse(tokens: &[Token]) -> Result<(Self, &[Token]), CompileError> {
        let Token::Directive(_, span) = &tokens[0] else { bug!("ExternDecl not a valid directive") };
        if tokens.len() < 2 {
            return Err(CompileError::InvalidExternDecl { span: span.clone(), custom_label: Some(EXPECTS_MORE_OPERAND.to_string()) });
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
            Err(CompileError::InvalidExternDecl { span: span.clone(), custom_label: Some(EXPECTS_IDEN.to_string()) })
        } else {
            Ok((
                ExternDecl { 
                    args, 
                    span: span.clone()
                },
                &tokens[i..]
            ))
        }
    }
}

impl Parse for ROData {
    fn parse(tokens: &[Token]) -> Result<(Self, &[Token]), CompileError> {
        let Token::Label(_, span) = &tokens[0] else { bug!("ROData not a valid directive") };
        if tokens.len() < 3 {
            return Err(CompileError::InvalidRodataDecl { span: span.clone(), custom_label: Some(EXPECTS_MORE_OPERAND.to_string()) });
        }

        let mut args = Vec::new();
        match (
            &tokens[0],
            &tokens[1],
            &tokens[2],
        ) {
            (
                Token::Label(name, span),
                Token::Directive(_, _),
                Token::StringLiteral(_, _)
            ) => {
                args.push(tokens[1].clone());
                args.push(tokens[2].clone());
                Ok((
                    ROData {
                        name: name.clone(),
                        args,
                        span: span.clone()
                    },
                    &tokens[3..]
                ))
            }
            (
                Token::Label(name, span),
                Token::Directive(_, _),
                Token::ImmediateValue(_, _)
            ) => {
                args.push(tokens[1].clone());
                args.push(tokens[2].clone());
                Ok((
                    ROData {
                        name: name.clone(),
                        args,
                        span: span.clone()
                    },
                    &tokens[3..]
                ))
            }
            _ => Err(CompileError::InvalidRodataDecl { span: span.clone(), custom_label: Some(EXPECTS_LABEL_DIR_STR.to_string()) }),
        }
    }
}

impl ParseWithConstMap for Instruction {
    fn parse_with_constmap<'a>(tokens: &'a [Token], const_map: &HashMap<String, ImmediateValue>) -> Result<(Self, &'a [Token]), CompileError> {
        let next_token_num;
        match &tokens[0] {
            Token::Opcode(opcode, span) => {
                let mut opcode = opcode.clone();
                let mut operands = Vec::new();
                match opcode {
                    Opcode::Lddw => {
                        if tokens.len() < 4 {
                            return Err(
                                CompileError::InvalidInstruction {  // 
                                    instruction: opcode.to_string() //
                                    , span: span.clone()            //
                                    , custom_label: Some(EXPECTS_MORE_OPERAND.to_string()) });
                        }
                        let (value, advance_token_num) = inline_and_fold_constant(tokens, const_map, 3);
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
                                    return Err(
                                        CompileError::InvalidInstruction {  //
                                            instruction: opcode.to_string() //
                                            , span: span.clone()            //
                                            , custom_label: Some(EXPECTS_REG_COM_IMM_OR_IDEN.to_string()) });
                                }
                            }
                            next_token_num = advance_token_num;
                        } else {
                            match (
                                &tokens[1],
                                &tokens[2],
                                &tokens[3],
                            ) {
                                (
                                    Token::Register(_, _),
                                    Token::Comma(_),
                                    Token::Identifier(_, _)
                                ) => {
                                    operands.push(tokens[1].clone());
                                    operands.push(tokens[3].clone());
                                }
                                _ => {
                                    return Err(
                                        CompileError::InvalidInstruction {  //
                                            instruction: opcode.to_string() //
                                            , span: span.clone()            //
                                            , custom_label: Some(EXPECTS_REG_COM_IMM_OR_IDEN.to_string()) });
                                }
                            }
                            next_token_num = 4;
                        }
                    }
                    Opcode::Ldxw | Opcode::Ldxh | Opcode::Ldxb | Opcode::Ldxdw => {
                        if tokens.len() < 8 {
                            return Err(
                                CompileError::InvalidInstruction {  //
                                    instruction: opcode.to_string() //
                                    , span: span.clone()            //
                                    , custom_label: Some(EXPECTS_MORE_OPERAND.to_string()) });
                        }
                        let (value, advance_token_num) = inline_and_fold_constant(tokens, const_map, 6);
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
                                    Token::RightBracket(_)
                                ) => {
                                    operands.push(tokens[1].clone());
                                    operands.push(tokens[4].clone());
                                    operands.push(Token::ImmediateValue(value, span.clone()));                                    
                                }
                                _ => {
                                    return Err(
                                        CompileError::InvalidInstruction {  //
                                            instruction: opcode.to_string() //
                                            , span: span.clone()            //
                                            , custom_label: Some(EXPECTS_REG_COM_LB_REG_BIOP_IMM_RB.to_string()) });
                                }
                            }
                            next_token_num = advance_token_num + 1;
                        } else {
                            return Err(
                                CompileError::InvalidInstruction {  //
                                    instruction: opcode.to_string() //
                                    , span: span.clone()            //
                                    , custom_label: Some(EXPECTS_REG_COM_LB_REG_BIOP_IMM_RB.to_string()) });
                        }
                    }
                    Opcode::Stw | Opcode::Sth | Opcode::Stb | Opcode::Stdw => {
                        if tokens.len() < 8 {
                            return Err(
                                CompileError::InvalidInstruction {  //
                                    instruction: opcode.to_string() //
                                    , span: span.clone()            //
                                    , custom_label: Some(EXPECTS_MORE_OPERAND.to_string()) });
                        }
                        let (value, advance_token_num) = inline_and_fold_constant(tokens, const_map, 4);
                        if let Some(value) = value {
                            // Now we need to fold the second immediate value (after the comma)
                            let (value2, advance_token_num2) = inline_and_fold_constant(tokens, const_map, advance_token_num + 2);
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
                                        return Err(
                                            CompileError::InvalidInstruction {  //
                                                instruction: opcode.to_string() //
                                                , span: span.clone()            //
                                                , custom_label: Some(EXPECTS_LB_REG_BIOP_IMM_RB_COM_IMM.to_string()) });
                                    }
                                }
                                next_token_num = advance_token_num2;
                            } else {
                                return Err(
                                    CompileError::InvalidInstruction {  //
                                        instruction: opcode.to_string() //
                                        , span: span.clone()            //
                                        , custom_label: Some(EXPECTS_LB_REG_BIOP_IMM_RB_COM_IMM.to_string()) });
                            }
                        } else {
                            return Err(
                                CompileError::InvalidInstruction {  //
                                    instruction: opcode.to_string() //
                                    , span: span.clone()            //
                                    , custom_label: Some(EXPECTS_LB_REG_BIOP_IMM_RB_COM_IMM.to_string()) });
                        }
                    }
                    | Opcode::Stxb | Opcode::Stxh | Opcode::Stxw | Opcode::Stxdw => {
                        if tokens.len() < 8 {
                            return Err(
                                CompileError::InvalidInstruction {  //
                                    instruction: opcode.to_string() //
                                    , span: span.clone()            //
                                    , custom_label: Some(EXPECTS_MORE_OPERAND.to_string()) });
                        }
                        let (value, advance_token_num) = inline_and_fold_constant(tokens, const_map, 4);
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
                                    Token::Register(_, _)
                                ) => {
                                    operands.push(tokens[2].clone());
                                    operands.push(Token::ImmediateValue(value, span.clone()));
                                    operands.push(tokens[advance_token_num + 2].clone());
                                }
                                _ => {
                                    return Err(
                                        CompileError::InvalidInstruction {  //
                                            instruction: opcode.to_string() //
                                            , span: span.clone()            //
                                            , custom_label: Some(EXPECTS_LB_REG_BIOP_IMM_RB_COM_REG.to_string()) });
                                }
                            }
                            next_token_num = advance_token_num + 3;
                        } else {
                            return Err(
                                CompileError::InvalidInstruction {  //
                                    instruction: opcode.to_string() //
                                    , span: span.clone()            //
                                    , custom_label: Some(EXPECTS_LB_REG_BIOP_IMM_RB_COM_REG.to_string()) });
                        }
                    }
                    Opcode::Add32 | Opcode::Sub32 | Opcode::Mul32 
                    | Opcode::Div32 | Opcode::Or32 | Opcode::And32 
                    | Opcode::Lsh32 | Opcode::Rsh32 | Opcode::Mod32 
                    | Opcode::Xor32 | Opcode::Mov32 | Opcode::Arsh32 
                    | Opcode::Lmul32 | Opcode::Udiv32 | Opcode::Urem32 
                    | Opcode::Sdiv32 | Opcode::Srem32
                    | Opcode::Add64 | Opcode::Sub64 | Opcode::Mul64 
                    | Opcode::Div64 | Opcode::Or64 | Opcode::And64 
                    | Opcode::Lsh64 | Opcode::Rsh64 | Opcode::Mod64 
                    | Opcode::Xor64 | Opcode::Mov64 | Opcode::Arsh64 
                    | Opcode::Lmul64 | Opcode::Uhmul64 | Opcode::Udiv64 
                    | Opcode::Urem64 | Opcode::Sdiv64 | Opcode::Srem64 => {
                        if tokens.len() < 4 {
                            return Err(
                                CompileError::InvalidInstruction {  //
                                    instruction: opcode.to_string() //
                                    , span: span.clone()            //
                                    , custom_label: Some(EXPECTS_MORE_OPERAND.to_string()) });
                        }
                        let (value, advance_token_num) = inline_and_fold_constant(tokens, const_map, 3);
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
                                    opcode = FromPrimitive::from_u8((opcode as u8) + 1).expect("Invalid opcode conversion"); 
                                    operands.push(tokens[1].clone());
                                    operands.push(Token::ImmediateValue(value, span.clone()));
                                }
                                _ => {
                                    return Err(
                                        CompileError::InvalidInstruction {  //
                                            instruction: opcode.to_string() //
                                            , span: span.clone()            //
                                            , custom_label: Some(EXPECTS_REG_COM_IMM.to_string()) });
                                }
                            } 
                            next_token_num = advance_token_num;
                        } else {
                            match (
                                &tokens[1],
                                &tokens[2],
                                &tokens[3],
                            ) {
                                (
                                    Token::Register(_, _),
                                    Token::Comma(_),
                                    Token::Register(_, _)
                                ) => {
                                    opcode = FromPrimitive::from_u8((opcode as u8) + 2).expect("Invalid opcode conversion"); 
                                    operands.push(tokens[1].clone());
                                    operands.push(tokens[3].clone());
                                }
                                _ => {
                                    return Err(
                                        CompileError::InvalidInstruction {  //
                                            instruction: opcode.to_string() //
                                            , span: span.clone()            //
                                            , custom_label: Some(EXPECTS_REG_COM_REG.to_string()) });
                                }
                            }                           
                            next_token_num = 4;
                        }
                    }
                    Opcode::Jeq | Opcode::Jgt | Opcode::Jge
                    | Opcode::Jlt | Opcode::Jle | Opcode::Jset
                    | Opcode::Jne | Opcode::Jsgt | Opcode::Jsge
                    | Opcode::Jslt | Opcode::Jsle => {
                        if tokens.len() < 6 {
                            return Err(
                                CompileError::InvalidInstruction {  //
                                    instruction: opcode.to_string() //
                                    , span: span.clone()            //
                                    , custom_label: Some(EXPECTS_MORE_OPERAND.to_string()) });
                        }
                        let (value, advance_token_num) = inline_and_fold_constant(tokens, const_map, 3);
                        if let Some(value) = value {
                            let (jump_val, jump_val_advance_token_num) = inline_and_fold_constant(tokens, const_map, advance_token_num + 1);
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
                                        opcode = FromPrimitive::from_u8((opcode as u8) + 1).expect("Invalid opcode conversion"); 
                                        operands.push(tokens[1].clone());
                                        operands.push(Token::ImmediateValue(value, span.clone()));
                                        operands.push(Token::ImmediateValue(jump_val, span.clone()));
                                    }
                                    _ => {
                                        return Err(
                                            CompileError::InvalidInstruction {
                                                instruction: opcode.to_string()
                                                , span: span.clone()
                                                , custom_label: Some(EXPECTS_REG_COM_IMM_COM_IMM_OR_IDEN.to_string()) });
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
                                        Token::Identifier(_, _)
                                    ) => {
                                        opcode = FromPrimitive::from_u8((opcode as u8) + 1).expect("Invalid opcode conversion"); 
                                        operands.push(tokens[1].clone());
                                        operands.push(Token::ImmediateValue(value, span.clone()));
                                        operands.push(tokens[advance_token_num + 1].clone());
                                    }
                                    _ => {
                                        return Err(
                                            CompileError::InvalidInstruction {  //
                                                instruction: opcode.to_string() //
                                                , span: span.clone()            //
                                                , custom_label: Some(EXPECTS_REG_COM_IMM_COM_IMM_OR_IDEN.to_string()) });
                                    }
                                }
                                next_token_num = advance_token_num + 2;
                            }
                        } else {
                            let (jump_val, jump_val_advance_token_num) = inline_and_fold_constant(tokens, const_map, advance_token_num + 1);
                            if let Some(jump_val) = jump_val {
                                match (
                                    &tokens[1],
                                    &tokens[2],
                                    &tokens[3],
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
                                        // turn "invalid opcode" to a bug
                                        opcode = FromPrimitive::from_u8((opcode as u8) + 2).expect("Invalid opcode conversion"); 
                                        operands.push(tokens[1].clone());
                                        operands.push(tokens[3].clone());
                                        operands.push(Token::ImmediateValue(jump_val, span.clone()));
                                    }
                                    _ => {
                                        return Err(
                                            CompileError::InvalidInstruction {  //
                                                instruction: opcode.to_string() //
                                                , span: span.clone()            //
                                                , custom_label: Some(EXPECTS_REG_COM_IMM_COM_IMM_OR_IDEN.to_string()) });
                                    }
                                }
                                next_token_num = jump_val_advance_token_num;
                            } else {
                                match (
                                    &tokens[1],
                                    &tokens[2],
                                    &tokens[3],
                                    &tokens[4],
                                    &tokens[5],
                                ) {
                                    (
                                        Token::Register(_, _),
                                        Token::Comma(_),
                                        Token::Register(_, _),
                                        Token::Comma(_),
                                        Token::Identifier(_, _)
                                    ) => {
                                        // turn "invalid opcode" to a bug
                                        opcode = FromPrimitive::from_u8((opcode as u8) + 2).expect("Invalid opcode conversion"); 
                                        operands.push(tokens[1].clone());
                                        operands.push(tokens[3].clone());
                                        operands.push(tokens[5].clone());
                                    }
                                    _ => {
                                        return Err(
                                            CompileError::InvalidInstruction {  //
                                                instruction: opcode.to_string() //
                                                , span: span.clone()            //
                                                , custom_label: Some(EXPECTS_REG_COM_IMM_COM_IMM_OR_IDEN.to_string()) });
                                    }
                                }
                                next_token_num = 6;
                            }
                        }
                    }
                    Opcode::Neg32 | Opcode::Neg64 => {
                        if tokens.len() < 2 {
                            return Err(
                                CompileError::InvalidInstruction {  //
                                    instruction: opcode.to_string() //
                                    , span: span.clone()            //
                                    , custom_label: Some(EXPECTS_MORE_OPERAND.to_string()) });
                        }
                        match &tokens[1] {
                            Token::Register(_, _) => {
                                operands.push(tokens[1].clone());
                            }
                            _ => {
                                return Err(
                                    CompileError::InvalidInstruction {  //
                                        instruction: opcode.to_string() //
                                        , span: span.clone()            //
                                        , custom_label: Some(EXPECTS_REG.to_string()) });
                            }
                        }
                        next_token_num = 2;
                    }
                    Opcode::Ja => {
                        if tokens.len() < 2 {
                            return Err(
                                CompileError::InvalidInstruction {  //
                                    instruction: opcode.to_string() //
                                    , span: span.clone()            //
                                    , custom_label: Some(EXPECTS_MORE_OPERAND.to_string()) });
                        }
                        let (value, advance_token_num) = inline_and_fold_constant(tokens, const_map, 1);
                        if let Some(value) = value {
                            operands.push(Token::ImmediateValue(value, span.clone()));
                            next_token_num = advance_token_num;
                        } else {
                            match &tokens[1] {
                                Token::Identifier(_, _) => {
                                    operands.push(tokens[1].clone());
                                }
                                _ => {
                                    return Err(
                                        CompileError::InvalidInstruction {  //
                                            instruction: opcode.to_string() //
                                            , span: span.clone()            //
                                            , custom_label: Some(EXPECTS_IDEN.to_string()) });
                                }
                            }
                            next_token_num = 2;
                        }
                    }
                    Opcode::Call => {
                        if tokens.len() < 2 {
                            return Err(
                                CompileError::InvalidInstruction {  //
                                    instruction: opcode.to_string() //
                                    , span: span.clone()            //
                                    , custom_label: Some(EXPECTS_MORE_OPERAND.to_string()) });
                        }
                        match &tokens[1] {
                            Token::Identifier(_, _) => {
                                operands.push(tokens[1].clone());
                            }
                            _ => {
                                return Err(
                                    CompileError::InvalidInstruction {  //
                                        instruction: opcode.to_string() //
                                        , span: span.clone()            //
                                        , custom_label: Some(EXPECTS_IDEN.to_string()) });
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
                        span: span.clone()
                    },
                    &tokens[next_token_num..]
                ))
            }
            _ => {
                bug!("invalid instruction");
            }
        }
        
    }
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
    start_idx: usize
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
                if stack.len() > 2 {
                    if let Token::BinaryOp(op, _) = &stack[stack.len() - 2] {
                        if matches!(op, Op::Mul | Op::Div) {
                            fold_top(&mut stack);
                        }
                    }
                }
            }

            Token::Identifier(name, span) if expect_number => {
                if let Some(const_map) = const_map {
                    if let Some(val) = const_map.get(name) {
                    stack.push(Token::ImmediateValue(val.clone(), span.clone()));
                    expect_number = false;

                    if stack.len() > 2 {
                        if let Token::BinaryOp(op, _) = &stack[stack.len() - 2] {
                            if matches!(op, Op::Mul | Op::Div) {
                                fold_top(&mut stack);
                            }
                        }
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
                let (inner_val, new_idx) = inline_and_fold_constant_with_map(tokens, const_map, idx + 1);
                idx = new_idx;
                if let Some(v) = inner_val {
                    stack.push(Token::ImmediateValue(v, span.clone()));
                    expect_number = false;

                    if stack.len() > 2 {
                        if let Token::BinaryOp(op, _) = &stack[stack.len() - 2] {
                            if matches!(op, Op::Mul | Op::Div) {
                                fold_top(&mut stack);
                            }
                        }
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

impl Parser {

    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens
            , m_prog_is_static: true
            , m_accum_offset: 0
            , m_entry_label: None
            , m_const_map: HashMap::new()
            , m_label_offsets: HashMap::new()
            , m_label_spans: HashMap::new()
            , m_rodata_size: 0
            , m_dynamic_symbols: DynamicSymbolMap::new()
            , m_rel_dyns: RelDynMap::new()
        }
    }

    pub fn parse(&mut self) -> Result<ParseResult, Vec<CompileError>> {
        let mut nodes = Vec::new();
        let mut rodata_nodes = Vec::new();
        let mut rodata_phase = false;

        let mut errors = Vec::new();

        let mut tokens = self.tokens.as_slice();

        // TODO: when parse error occurs, we should probably just jump to the next line
        // if we're able to error out the scenario where users put 2 instructions in the same line
        // for now we just continue to the next token

        // TODO: it would be nice if we build a token iterator that can 
        // 1. peek the next multiple tokens (for detecting patterns)
        // 2. jump to the next line
        // 3. continue to the next token
        while !tokens.is_empty() {
            match &tokens[0] {
                Token::Directive(name, span) => {
                    match name.as_str() {
                        "global" | "globl" => {
                            match GlobalDecl::parse(tokens) {
                                Ok((node, rest)) => {
                                self.m_entry_label = Some(node.get_entry_label());
                                nodes.push(ASTNode::GlobalDecl { global_decl: node });
                                tokens = rest;
                                }
                                Err(e) => {
                                    errors.push(e);
                                    tokens = &tokens[1..];
                                }
                            }
                        }
                        "extern" => {
                            match ExternDecl::parse(tokens) {
                                Ok((node, rest)) => {
                                nodes.push(ASTNode::ExternDecl { extern_decl: node });
                                tokens = rest;
                                }
                                Err(e) => {
                                    errors.push(e);
                                    tokens = &tokens[1..];
                                }
                            }
                        }
                        "rodata" => {
                            nodes.push(ASTNode::RodataDecl { rodata_decl: RodataDecl { span: span.clone() } });
                            rodata_phase = true;
                            tokens = &tokens[1..];
                        }
                        "equ" => {
                            match EquDecl::parse_with_constmap(tokens, &self.m_const_map) {
                                Ok((node, rest)) => {
                                self.m_const_map.insert(node.get_name(), node.get_val());
                                nodes.push(ASTNode::EquDecl { equ_decl: node });
                                tokens = rest;
                                }
                                Err(e) => {
                                    errors.push(e);
                                    tokens = &tokens[1..];
                                }
                            }
                        }
                        "section" => {
                            nodes.push(ASTNode::Directive { directive: Directive { name: name.clone(), args: Vec::new(), span: span.clone() } });
                            tokens = &tokens[1..];
                        }
                        _ => {
                            errors.push(CompileError::InvalidDirective { directive: name.clone(), span: span.clone(), custom_label: None });
                            tokens = &tokens[1..];
                        }
                    }
                }
                Token::Label(name, span) => {
                    if rodata_phase {
                        match ROData::parse(tokens) {
                            Ok((rodata, rest)) => {
                                if self.m_label_offsets.contains_key(name) {
                                    let original_span = self.m_label_spans.get(name).cloned().unwrap_or(span.clone());
                                    errors.push(CompileError::DuplicateLabel { label: name.clone(), span: span.clone(), original_span, custom_label: Some(LABEL_REDEFINED.to_string()) });
                                } else {
                                    self.m_label_offsets.insert(name.clone(), self.m_accum_offset + self.m_rodata_size);
                                    if let Err(e) = rodata.verify() {
                                        errors.push(e);
                                    } else {
                                        self.m_rodata_size += rodata.get_size();
                                    }
                                }
                                rodata_nodes.push(ASTNode::ROData { rodata, offset: self.m_accum_offset });
                                tokens = rest;
                            }
                            Err(e) => {
                                errors.push(e);
                                tokens = &tokens[1..];
                            }
                        }
                    } else {
                        if self.m_label_offsets.contains_key(name) {
                            let original_span = self.m_label_spans.get(name).cloned().unwrap_or(span.clone());
                            errors.push(CompileError::DuplicateLabel { label: name.clone(), span: span.clone(), original_span, custom_label: Some(LABEL_REDEFINED.to_string()) });
                        } else {
                            self.m_label_offsets.insert(name.clone(), self.m_accum_offset);
                            self.m_label_spans.insert(name.clone(), span.clone());
                        }
                        nodes.push(ASTNode::Label { label: Label { name: name.clone(), span: span.clone() } });
                        tokens = &tokens[1..];
                    }
                }
                Token::Opcode(_, _) => {
                    match Instruction::parse_with_constmap(tokens, &self.m_const_map) {
                        Ok((inst, rest)) => {
                            let offset = self.m_accum_offset;
                            self.m_accum_offset += inst.get_size();
                            nodes.push(ASTNode::Instruction { instruction: inst, offset });
                            tokens = rest;
                        }
                        Err(e) => {
                            errors.push(e);
                            tokens = &tokens[1..];
                        }
                    }
                }
                _ => {
                    tokens = &tokens[1..];
                }
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        // Second pass to resolve labels
        for node in &mut nodes {
            match node {
                ASTNode::Instruction { instruction: inst, offset, .. } => {
                    // For jump/call instructions, replace label with relative offsets
                    if inst.is_jump() || inst.opcode == Opcode::Call {
                        if let Some(Token::Identifier(label, span)) = inst.operands.last() {
                            let label = label.clone();
                            if let Some(target_offset) = self.m_label_offsets.get(&label) {
                                let rel_offset = (*target_offset as i64 - *offset as i64) / 8 - 1;
                                let last_idx = inst.operands.len() - 1;
                                inst.operands[last_idx] = Token::ImmediateValue(ImmediateValue::Int(rel_offset), span.clone());
                            } else if inst.is_jump() {
                                // only error out unresolved jump labels, since call 
                                // labels could exist externally
                                errors.push(CompileError::UndefinedLabel { label: label.clone(), span: span.clone(), custom_label: None });
                            }
                        }
                    }
                    // This has to be done before resolving lddw labels since lddw 
                    // operand needs to be absolute offset values
                    if inst.needs_relocation() {
                        self.m_prog_is_static = false;
                        let (reloc_type, label) = inst.get_relocation_info();
                        self.m_rel_dyns.add_rel_dyn(*offset, reloc_type, label.clone());
                        if reloc_type == RelocationType::RSbfSyscall {
                            self.m_dynamic_symbols.add_call_target(label.clone(), *offset);
                        }
                    }
                    if inst.opcode == Opcode::Lddw {
                        if let Some(Token::Identifier(name, span)) = inst.operands.last() {
                            let label = name.clone();
                            if let Some(target_offset) = self.m_label_offsets.get(&label) {
                                let ph_count = if self.m_prog_is_static { 1 } else { 3 };
                                let ph_offset = 64 + (ph_count as u64 * 56) as i64;
                                let abs_offset = *target_offset as i64 + ph_offset;
                                // Replace label with immediate value
                                let last_idx = inst.operands.len() - 1;
                                inst.operands[last_idx] = Token::ImmediateValue(ImmediateValue::Addr(abs_offset), span.clone());
                            }  else {
                                errors.push(CompileError::UndefinedLabel { label: name.clone(), span: span.clone(), custom_label: None });
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Set entry point offset if an entry label was specified
        if let Some(entry_label) = &self.m_entry_label {
            if let Some(offset) = self.m_label_offsets.get(entry_label) {
                self.m_dynamic_symbols.add_entry_point(entry_label.clone(), *offset);
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        } else {
            Ok(ParseResult {
                code_section: CodeSection::new(nodes, self.m_accum_offset),
                data_section: DataSection::new(rodata_nodes, self.m_rodata_size),
                dynamic_symbols: DynamicSymbolMap::copy(&self.m_dynamic_symbols),
                relocation_data: RelDynMap::copy(&self.m_rel_dyns),
                prog_is_static: self.m_prog_is_static,
            })
        }
    }
}
