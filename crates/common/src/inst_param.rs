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

    fn retag(&self, other: &Number, value: i64) -> Number {
        match (self, other) {
            (Number::Int(_), Number::Int(_)) => Number::Int(value),
            _ => Number::Addr(value),
        }
    }

    pub fn checked_add(&self, other: &Number) -> Option<Number> {
        Some(self.retag(other, self.to_i64().checked_add(other.to_i64())?))
    }

    pub fn checked_sub(&self, other: &Number) -> Option<Number> {
        Some(self.retag(other, self.to_i64().checked_sub(other.to_i64())?))
    }

    pub fn checked_mul(&self, other: &Number) -> Option<Number> {
        Some(self.retag(other, self.to_i64().checked_mul(other.to_i64())?))
    }

    pub fn checked_div(&self, other: &Number) -> Option<Number> {
        Some(self.retag(other, self.to_i64().checked_div(other.to_i64())?))
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
    fn test_number_checked_add() {
        // Int + Int
        assert_eq!(
            Number::Int(10).checked_add(&Number::Int(20)),
            Some(Number::Int(30))
        );
        // Addr + Addr
        assert_eq!(
            Number::Addr(10).checked_add(&Number::Addr(20)),
            Some(Number::Addr(30))
        );
        // Int + Addr / Addr + Int both tag the result as Addr
        assert_eq!(
            Number::Int(10).checked_add(&Number::Addr(20)),
            Some(Number::Addr(30))
        );
        assert_eq!(
            Number::Addr(10).checked_add(&Number::Int(20)),
            Some(Number::Addr(30))
        );
    }

    #[test]
    fn test_number_checked_sub() {
        assert_eq!(
            Number::Int(30).checked_sub(&Number::Int(10)),
            Some(Number::Int(20))
        );
        assert_eq!(
            Number::Addr(30).checked_sub(&Number::Int(10)),
            Some(Number::Addr(20))
        );
    }

    #[test]
    fn test_number_checked_mul() {
        assert_eq!(
            Number::Int(5).checked_mul(&Number::Int(4)),
            Some(Number::Int(20))
        );
        assert_eq!(
            Number::Addr(5).checked_mul(&Number::Int(4)),
            Some(Number::Addr(20))
        );
    }

    #[test]
    fn test_number_checked_div() {
        assert_eq!(
            Number::Int(20).checked_div(&Number::Int(4)),
            Some(Number::Int(5))
        );
        assert_eq!(
            Number::Addr(20).checked_div(&Number::Int(4)),
            Some(Number::Addr(5))
        );
    }

    #[test]
    fn test_number_checked_arithmetic_reports_overflow() {
        // Overflow returns None instead of panicking / wrapping.
        assert_eq!(Number::Int(i64::MAX).checked_add(&Number::Int(1)), None);
        assert_eq!(Number::Int(i64::MIN).checked_sub(&Number::Int(1)), None);
        assert_eq!(Number::Int(i64::MAX).checked_mul(&Number::Int(2)), None);
        // i64::MIN / -1 overflows two's-complement division.
        assert_eq!(Number::Int(i64::MIN).checked_div(&Number::Int(-1)), None);
    }

    #[test]
    fn test_number_checked_div_by_zero() {
        assert_eq!(Number::Int(8).checked_div(&Number::Int(0)), None);
        assert_eq!(Number::Addr(8).checked_div(&Number::Int(0)), None);
    }
}
