use std::ops::Range;

use crate::opcode::Opcode;

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum Register {
    R0,
    R1,
    R2,
    R3,
    R4,
    R5,
    R6,
    R7,
    R8,
    R9,
    R10,
}

impl Register {
    pub fn to_string(&self) -> String {
        format!("r{}", *self as u8)
    }
}

#[derive(Debug, Clone, PartialEq)]
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
