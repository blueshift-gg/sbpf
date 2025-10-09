use crate::opcode::Opcode;
use std::ops::Range;

#[derive(Debug, Clone)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImmediateValue {
    Int(i64),
    Addr(i64),
}

impl std::ops::Add for ImmediateValue {
    type Output = ImmediateValue;
    fn add(self, other: Self) -> ImmediateValue {
        match (self, other) {
            (ImmediateValue::Int(a), ImmediateValue::Int(b)) => ImmediateValue::Int(a + b),
            (ImmediateValue::Addr(a), ImmediateValue::Addr(b)) => ImmediateValue::Addr(a + b),
            (ImmediateValue::Int(a), ImmediateValue::Addr(b)) => ImmediateValue::Addr(a + b),
            (ImmediateValue::Addr(a), ImmediateValue::Int(b)) => ImmediateValue::Addr(a + b),
        }
    }
}

impl std::ops::Sub for ImmediateValue {
    type Output = ImmediateValue;
    fn sub(self, other: Self) -> ImmediateValue {
        match (self, other) {
            (ImmediateValue::Int(a), ImmediateValue::Int(b)) => ImmediateValue::Int(a - b),
            (ImmediateValue::Addr(a), ImmediateValue::Addr(b)) => ImmediateValue::Addr(a - b),
            (ImmediateValue::Int(a), ImmediateValue::Addr(b)) => ImmediateValue::Addr(a - b),
            (ImmediateValue::Addr(a), ImmediateValue::Int(b)) => ImmediateValue::Addr(a - b),
        }
    }
}

impl std::ops::Mul for ImmediateValue {
    type Output = ImmediateValue;
    fn mul(self, other: Self) -> ImmediateValue {
        match (self, other) {
            (ImmediateValue::Int(a), ImmediateValue::Int(b)) => ImmediateValue::Int(a * b),
            (ImmediateValue::Addr(a), ImmediateValue::Addr(b)) => ImmediateValue::Addr(a * b),
            (ImmediateValue::Int(a), ImmediateValue::Addr(b)) => ImmediateValue::Addr(a * b),
            (ImmediateValue::Addr(a), ImmediateValue::Int(b)) => ImmediateValue::Addr(a * b),
        }
    }
}

impl std::ops::Div for ImmediateValue {
    type Output = ImmediateValue;
    fn div(self, other: Self) -> ImmediateValue {
        match (self, other) {
            (ImmediateValue::Int(a), ImmediateValue::Int(b)) => ImmediateValue::Int(a / b),
            (ImmediateValue::Addr(a), ImmediateValue::Addr(b)) => ImmediateValue::Addr(a / b),
            (ImmediateValue::Int(a), ImmediateValue::Addr(b)) => ImmediateValue::Addr(a / b),
            (ImmediateValue::Addr(a), ImmediateValue::Int(b)) => ImmediateValue::Addr(a / b),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Token {
    Directive(String, Range<usize>),
    Label(String, Range<usize>),
    Identifier(String, Range<usize>),
    Opcode(Opcode, Range<usize>),
    Register(u8, Range<usize>),
    ImmediateValue(ImmediateValue, Range<usize>),
    BinaryOp(Op, Range<usize>),
    StringLiteral(String, Range<usize>),
    VectorLiteral(Vec<ImmediateValue>, Range<usize>),

    LeftBracket(Range<usize>),
    RightBracket(Range<usize>),
    LeftParen(Range<usize>),
    RightParen(Range<usize>),
    Comma(Range<usize>),
    Colon(Range<usize>),

    Newline(Range<usize>),
}
