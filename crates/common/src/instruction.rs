use std::ops::Range;

use crate::opcode::Opcode;

#[derive(Debug, Clone)]
pub struct Register {
    pub n: u8,
}

impl Register {
    pub fn to_string(&self) -> String {
        format!("r{}", self.n)
    }
}

#[derive(Debug, Clone)]
pub enum Number {
    Int(i64),
    Addr(i64),
}

#[derive(Debug, Clone)]
pub struct Instruction {
    pub opcode: Opcode,
    pub dst: Option<Register>,
    pub src: Option<Register>,
    pub off: Option<i16>,
    pub imm: Option<Number>,
    pub span: Range<usize>,
}
