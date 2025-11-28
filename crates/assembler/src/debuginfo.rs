use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterType {
    Int,
    Addr,
    Null,
}

impl RegisterType {
    pub fn to_string(&self) -> &'static str {
        match self {
            RegisterType::Int => "int",
            RegisterType::Addr => "addr",
            RegisterType::Null => "null",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RegisterHint {
    pub register: usize,
    pub register_type: RegisterType,
}

impl Default for RegisterHint {
    fn default() -> Self {
        Self {
            register: 0,
            register_type: RegisterType::Null,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DebugInfo {
    pub code_span: Range<usize>,
    pub register_hint: RegisterHint,
}

impl DebugInfo {
    pub fn new(code_span: Range<usize>) -> Self {
        Self {
            code_span,
            register_hint: RegisterHint::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_type_to_string() {
        assert_eq!(RegisterType::Int.to_string(), "int");
        assert_eq!(RegisterType::Addr.to_string(), "addr");
        assert_eq!(RegisterType::Null.to_string(), "null");
    }

    #[test]
    fn test_register_hint_default() {
        let hint = RegisterHint::default();
        assert_eq!(hint.register, 0);
        assert_eq!(hint.register_type, RegisterType::Null);
    }

    #[test]
    fn test_debug_info_new() {
        let debug_info = DebugInfo::new(10..20);
        assert_eq!(debug_info.code_span, 10..20);
        assert_eq!(debug_info.register_hint.register, 0);
        assert_eq!(debug_info.register_hint.register_type, RegisterType::Null);
    }

    #[test]
    fn test_register_hint_custom() {
        let hint = RegisterHint {
            register: 5,
            register_type: RegisterType::Addr,
        };
        assert_eq!(hint.register, 5);
        assert_eq!(hint.register_type, RegisterType::Addr);
    }
}
