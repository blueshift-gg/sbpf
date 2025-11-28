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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Number {
    Int(i64),
    Addr(i64),
}

impl Number {
    pub fn to_i16(&self) -> i16 {
        match self {
            Number::Int(v) => *v as i16,
            Number::Addr(a) => *a as i16,
        }
    }

    pub fn to_i64(&self) -> i64 {
        match self {
            Number::Int(v) => *v,
            Number::Addr(a) => *a,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_display() {
        let reg = Register { n: 5 };
        assert_eq!(reg.to_string(), "r5");

        let reg0 = Register { n: 0 };
        assert_eq!(reg0.to_string(), "r0");

        let reg10 = Register { n: 10 };
        assert_eq!(reg10.to_string(), "r10");
    }

    #[test]
    fn test_number_to_i16() {
        assert_eq!(Number::Int(42).to_i16(), 42i16);
        assert_eq!(Number::Addr(100).to_i16(), 100i16);
        assert_eq!(Number::Int(-5).to_i16(), -5i16);
    }

    #[test]
    fn test_number_to_i64() {
        assert_eq!(Number::Int(42).to_i64(), 42i64);
        assert_eq!(Number::Addr(100).to_i64(), 100i64);
        assert_eq!(Number::Int(-5).to_i64(), -5i64);
    }

    #[test]
    fn test_number_display() {
        assert_eq!(Number::Int(42).to_string(), "42");
        assert_eq!(Number::Addr(100).to_string(), "100");
        assert_eq!(Number::Int(-5).to_string(), "-5");
    }

    #[test]
    fn test_number_add() {
        // Int + Int
        let result = Number::Int(10) + Number::Int(20);
        assert_eq!(result, Number::Int(30));

        // Addr + Addr
        let result = Number::Addr(10) + Number::Addr(20);
        assert_eq!(result, Number::Addr(30));

        // Int + Addr
        let result = Number::Int(10) + Number::Addr(20);
        assert_eq!(result, Number::Addr(30));

        // Addr + Int
        let result = Number::Addr(10) + Number::Int(20);
        assert_eq!(result, Number::Addr(30));
    }

    #[test]
    fn test_number_sub() {
        // Int - Int
        let result = Number::Int(30) - Number::Int(10);
        assert_eq!(result, Number::Int(20));

        // Addr - Addr
        let result = Number::Addr(30) - Number::Addr(10);
        assert_eq!(result, Number::Addr(20));

        // Int - Addr
        let result = Number::Int(30) - Number::Addr(10);
        assert_eq!(result, Number::Addr(20));

        // Addr - Int
        let result = Number::Addr(30) - Number::Int(10);
        assert_eq!(result, Number::Addr(20));
    }

    #[test]
    fn test_number_mul() {
        // Int * Int
        let result = Number::Int(5) * Number::Int(4);
        assert_eq!(result, Number::Int(20));

        // Addr * Addr
        let result = Number::Addr(5) * Number::Addr(4);
        assert_eq!(result, Number::Addr(20));

        // Int * Addr
        let result = Number::Int(5) * Number::Addr(4);
        assert_eq!(result, Number::Addr(20));

        // Addr * Int
        let result = Number::Addr(5) * Number::Int(4);
        assert_eq!(result, Number::Addr(20));
    }

    #[test]
    fn test_number_div() {
        // Int / Int
        let result = Number::Int(20) / Number::Int(4);
        assert_eq!(result, Number::Int(5));

        // Addr / Addr
        let result = Number::Addr(20) / Number::Addr(4);
        assert_eq!(result, Number::Addr(5));

        // Int / Addr
        let result = Number::Int(20) / Number::Addr(4);
        assert_eq!(result, Number::Addr(5));

        // Addr / Int
        let result = Number::Addr(20) / Number::Int(4);
        assert_eq!(result, Number::Addr(5));
    }
}
