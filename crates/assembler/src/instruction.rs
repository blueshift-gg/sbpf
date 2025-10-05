use crate::opcode::Opcode;
use crate::lexer::{Token, ImmediateValue};
use std::ops::Range;
use crate::dynsym::RelocationType;

#[derive(Debug, Clone)]
pub struct Instruction {
    pub opcode: Opcode,
    pub operands: Vec<Token>,
    pub span: Range<usize>,
}

impl Instruction {
    //
    pub fn get_size(&self) -> u64 {
        match self.opcode {
            Opcode::Lddw => 16,
            _ => 8,
        }
    }
    //
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
    //
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
    //
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
    //
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let mut operands = Vec::new();
        let span = 0..bytes.len();

        let opcode = Opcode::from_u8(bytes[0]).unwrap();

        match opcode {
            Opcode::Lddw => {
                // lddw format: [opcode(1)] [dst_reg(1)] [0(1)] [0(1)] [imm32_low(4)] [0(4)] [imm32_high(4)]
                if bytes.len() < 16 {
                    return None;
                }
            
                let dst_reg = bytes[1];
                let imm_low = i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
                let imm_high = i32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
                let imm64 = ((imm_high as i64) << 32) | (imm_low as u32 as i64);
            
                operands.push(Token::Register(dst_reg, 1..2));
                operands.push(Token::ImmediateValue(ImmediateValue::Int(imm64), 4..12));
            }
        
            Opcode::Call => {
                // call format: [opcode(1)] [0(1)] [0(1)] [0(1)] [imm32(4)]
                if bytes.len() < 8 {
                    return None;
                }
            
                let imm32 = i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
                // HARDCODE call imm 4242 to sol_log_
                if imm32 == 4242 {
                    operands.push(Token::Identifier("sol_log_".to_string(), 4..8));
                } else {
                    operands.push(Token::ImmediateValue(ImmediateValue::Int(imm32 as i64), 4..8));
                }
            }
        
            Opcode::Ja => {
                // ja format: [opcode(1)] [0(1)] [imm16(2)] [0(4)]
                if bytes.len() < 8 {
                    return None;
                }
            
                let imm16 = i16::from_le_bytes([bytes[2], bytes[3]]);
                operands.push(Token::ImmediateValue(ImmediateValue::Int(imm16 as i64), 2..4));
            }
        
            // Jump instructions with immediate values
            Opcode::JeqImm | Opcode::JgtImm | Opcode::JgeImm | Opcode::JltImm | 
            Opcode::JleImm | Opcode::JsetImm | Opcode::JneImm | Opcode::JsgtImm | 
            Opcode::JsgeImm | Opcode::JsltImm | Opcode::JsleImm => {
                // Format: [opcode(1)] [dst_reg(1)] [0(1)] [0(1)] [imm32(4)]
                if bytes.len() < 8 {
                    return None;
                }
            
                let dst_reg = bytes[1];
                let imm32 = i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
            
                operands.push(Token::Register(dst_reg, 1..2));
                operands.push(Token::ImmediateValue(ImmediateValue::Int(imm32 as i64), 4..8));
            }
        
            // Jump instructions with register operands
            Opcode::JeqReg | Opcode::JgtReg | Opcode::JgeReg | Opcode::JltReg | 
            Opcode::JleReg | Opcode::JsetReg | Opcode::JneReg | Opcode::JsgtReg | 
            Opcode::JsgeReg | Opcode::JsltReg | Opcode::JsleReg => {
                // Format: [opcode(1)] [dst_reg(1)] [src_reg(1)] [0(1)] [0(4)]
                if bytes.len() < 8 {
                    return None;
                }
            
                let dst_reg = bytes[1];
                let src_reg = bytes[2];
            
                operands.push(Token::Register(dst_reg, 1..2));
                operands.push(Token::Register(src_reg, 2..3));
            }
        
            // Arithmetic instructions with immediate values
            Opcode::Add32Imm | Opcode::Sub32Imm | Opcode::Mul32Imm | Opcode::Div32Imm |
            Opcode::Or32Imm | Opcode::And32Imm | Opcode::Lsh32Imm | Opcode::Rsh32Imm |
            Opcode::Mod32Imm | Opcode::Xor32Imm | Opcode::Mov32Imm | Opcode::Arsh32Imm |
            Opcode::Lmul32Imm | Opcode::Udiv32Imm | Opcode::Urem32Imm | Opcode::Sdiv32Imm |
            Opcode::Srem32Imm | Opcode::Add64Imm | Opcode::Sub64Imm | Opcode::Mul64Imm |
            Opcode::Div64Imm | Opcode::Or64Imm | Opcode::And64Imm | Opcode::Lsh64Imm |
            Opcode::Rsh64Imm | Opcode::Mod64Imm | Opcode::Xor64Imm | Opcode::Mov64Imm |
            Opcode::Arsh64Imm | Opcode::Hor64Imm | Opcode::Lmul64Imm | Opcode::Uhmul64Imm |
            Opcode::Udiv64Imm | Opcode::Urem64Imm | Opcode::Shmul64Imm | Opcode::Sdiv64Imm |
            Opcode::Srem64Imm => {
                // Format: [opcode(1)] [dst_reg(1)] [0(1)] [0(1)] [imm32(4)]
                if bytes.len() < 8 {
                    return None;
                }
            
                let dst_reg = bytes[1];
                let imm32 = i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
            
                operands.push(Token::Register(dst_reg, 1..2));
                operands.push(Token::ImmediateValue(ImmediateValue::Int(imm32 as i64), 4..8));
            }
        
            // Arithmetic instructions with register operands
            Opcode::Add32Reg | Opcode::Sub32Reg | Opcode::Mul32Reg | Opcode::Div32Reg |
            Opcode::Or32Reg | Opcode::And32Reg | Opcode::Lsh32Reg | Opcode::Rsh32Reg |
            Opcode::Mod32Reg | Opcode::Xor32Reg | Opcode::Mov32Reg | Opcode::Arsh32Reg |
            Opcode::Lmul32Reg | Opcode::Udiv32Reg | Opcode::Urem32Reg | Opcode::Sdiv32Reg |
            Opcode::Srem32Reg | Opcode::Add64Reg | Opcode::Sub64Reg | Opcode::Mul64Reg |
            Opcode::Div64Reg | Opcode::Or64Reg | Opcode::And64Reg | Opcode::Lsh64Reg |
            Opcode::Rsh64Reg | Opcode::Mod64Reg | Opcode::Xor64Reg | Opcode::Mov64Reg |
            Opcode::Arsh64Reg | Opcode::Lmul64Reg | Opcode::Uhmul64Reg | Opcode::Udiv64Reg |
            Opcode::Urem64Reg | Opcode::Shmul64Reg | Opcode::Sdiv64Reg | Opcode::Srem64Reg => {
                // Format: [opcode(1)] [dst_reg(1)] [src_reg(1)] [0(1)] [0(4)]
                if bytes.len() < 8 {
                    return None;
                }
            
                let dst_reg = bytes[1];
                let src_reg = bytes[2];
            
                operands.push(Token::Register(dst_reg, 1..2));
                operands.push(Token::Register(src_reg, 2..3));
            }
        
            // Load/Store instructions with immediate offset
            Opcode::Ldxb | Opcode::Ldxh | Opcode::Ldxw | Opcode::Ldxdw |
            Opcode::Stb | Opcode::Sth | Opcode::Stw | Opcode::Stdw |
            Opcode::Stxb | Opcode::Stxh | Opcode::Stxw | Opcode::Stxdw => {
                // Format: [opcode(1)] [dst_reg(1)] [src_reg(1)] [offset16(2)] [imm32(4)]
                if bytes.len() < 8 {
                    return None;
                }
            
                let dst_reg = bytes[1];
                let offset16 = u16::from_le_bytes([bytes[3], bytes[4]]);
                let imm32 = i32::from_le_bytes([bytes[5], bytes[6], bytes[7], 0]);
            
                operands.push(Token::Register(dst_reg, 1..2));
                operands.push(Token::ImmediateValue(ImmediateValue::Int(imm32 as i64), 5..8));
                operands.push(Token::ImmediateValue(ImmediateValue::Int(offset16 as i64), 3..5));
            }
        
            // Unary operations
            Opcode::Neg32 | Opcode::Neg64 | Opcode::Exit => {
                // Format: [opcode(1)] [dst_reg(1)] [0(1)] [0(1)] [0(4)]
                if bytes.len() < 8 {
                    return None;
                }
            
                let dst_reg = bytes[1];
                operands.push(Token::Register(dst_reg, 1..2));
            }
        
            // Endianness operations
            Opcode::Le | Opcode::Be => {
                // Format: [opcode(1)] [dst_reg(1)] [0(1)] [0(1)] [imm32(4)]
                if bytes.len() < 8 {
                    return None;
                }
            
                let dst_reg = bytes[1];
                let imm32 = i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
            
                operands.push(Token::Register(dst_reg, 1..2));
                operands.push(Token::ImmediateValue(ImmediateValue::Int(imm32 as i64), 4..8));
            }
        
            _ => {
                return None;
            }
        }

        Some(Instruction {
            opcode,
            operands,
            span,
        })
    }
}

