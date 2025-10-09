use std::ops::Range;

use crate::dynsym::RelocationType;
use crate::opcode::Opcode;
use crate::token::Token;

#[derive(Debug, Clone)]
pub struct Instruction {
    pub opcode: Opcode,
    pub dst: Option<Token>,
    pub src: Option<Token>,
    pub off: Option<Token>,
    pub imm: Option<Token>,
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
            Opcode::Call | Opcode::Lddw => match &self.imm {
                Some(Token::Identifier(_, _)) => true,
                _ => false,
            },
            _ => false,
        }
    }

    pub fn is_jump(&self) -> bool {
        match self.opcode {
            Opcode::Ja
            | Opcode::JeqImm
            | Opcode::JgtImm
            | Opcode::JgeImm
            | Opcode::JltImm
            | Opcode::JleImm
            | Opcode::JsetImm
            | Opcode::JneImm
            | Opcode::JsgtImm
            | Opcode::JsgeImm
            | Opcode::JsltImm
            | Opcode::JsleImm
            | Opcode::JeqReg
            | Opcode::JgtReg
            | Opcode::JgeReg
            | Opcode::JltReg
            | Opcode::JleReg
            | Opcode::JsetReg
            | Opcode::JneReg
            | Opcode::JsgtReg
            | Opcode::JsgeReg
            | Opcode::JsltReg
            | Opcode::JsleReg => true,
            _ => false,
        }
    }

    pub fn get_relocation_info(&self) -> (RelocationType, String) {
        match self.opcode {
            Opcode::Lddw => match &self.imm {
                Some(Token::Identifier(name, _)) => (RelocationType::RSbf64Relative, name.clone()),
                _ => panic!("Expected label operand"),
            },
            _ => {
                if let Some(Token::Identifier(name, _)) = &self.imm {
                    (RelocationType::RSbfSyscall, name.clone())
                } else {
                    panic!("Expected label operand")
                }
            }
        }
    }
}
