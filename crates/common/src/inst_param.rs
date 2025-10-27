use {
    core::fmt,
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Register {
    pub n: u8,
}

impl fmt::Display for Register {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "r{}", self.n)
    }
}

pub enum OperandValue {
    Number(Number),
    Ident(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Number {
    Int(i64),
    Addr(i64),
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Number::Int(i) => write!(f, "{}", i),
            Number::Addr(a) => write!(f, "{}", a),
        }
    }
}

impl std::ops::Add for Number {
    type Output = Number;
    fn add(self, other: Self) -> Number {
        match (self, other) {
            (Number::Int(a), Number::Int(b)) => Number::Int(a + b),
            (Number::Addr(a), Number::Addr(b)) => Number::Addr(a + b),
            (Number::Int(a), Number::Addr(b)) => Number::Addr(a + b),
            (Number::Addr(a), Number::Int(b)) => Number::Addr(a + b),
        }
    }
}

impl std::ops::Sub for Number {
    type Output = Number;
    fn sub(self, other: Self) -> Number {
        match (self, other) {
            (Number::Int(a), Number::Int(b)) => Number::Int(a - b),
            (Number::Addr(a), Number::Addr(b)) => Number::Addr(a - b),
            (Number::Int(a), Number::Addr(b)) => Number::Addr(a - b),
            (Number::Addr(a), Number::Int(b)) => Number::Addr(a - b),
        }
    }
}

impl std::ops::Mul for Number {
    type Output = Number;
    fn mul(self, other: Self) -> Number {
        match (self, other) {
            (Number::Int(a), Number::Int(b)) => Number::Int(a * b),
            (Number::Addr(a), Number::Addr(b)) => Number::Addr(a * b),
            (Number::Int(a), Number::Addr(b)) => Number::Addr(a * b),
            (Number::Addr(a), Number::Int(b)) => Number::Addr(a * b),
        }
    }
}

impl std::ops::Div for Number {
    type Output = Number;
    fn div(self, other: Self) -> Number {
        match (self, other) {
            (Number::Int(a), Number::Int(b)) => Number::Int(a / b),
            (Number::Addr(a), Number::Addr(b)) => Number::Addr(a / b),
            (Number::Int(a), Number::Addr(b)) => Number::Addr(a / b),
            (Number::Addr(a), Number::Int(b)) => Number::Addr(a / b),
        }
    }
}
