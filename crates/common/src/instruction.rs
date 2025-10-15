use crate::opcode::Opcode;

use core::fmt;
use core::ops::Range;

#[derive(Debug, Clone)]
pub struct Register {
    pub n: u8,
}

impl fmt::Display for Register {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "r{}", self.n)
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
