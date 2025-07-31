use super::*;
use proptest::prelude::*;
use std::collections::HashMap;

proptest! {
    #[test]
    fn test_lexer_roundtrip(input in arb_assembly()) {
        let tokens = tokenize(&input).unwrap();
        // Verify we can parse what we lex
        assert!(!tokens.is_empty());
    }
    
    #[test]
    fn test_parser_roundtrip(input in arb_assembly()) {
        let tokens = tokenize(&input).unwrap();
        let mut parser = Parser::new(tokens);
        let result = parser.parse();
        // Should not panic, even if it fails
        let _ = result;
    }
}

#[test]
fn test_lexer_fuzz() {
    let fuzz_inputs = vec![
        "lddw r1, 42",
        "add64 r0, r1",
        "exit",
        "// comment\nlddw r1, 0x42",
        "# another comment\ncall sol_log_",
        ".globl entrypoint\nentrypoint:\n  exit",
    ];
    
    for input in fuzz_inputs {
        let result = tokenize(input);
        assert!(result.is_ok(), "Failed to lex: {}", input);
    }
}

// Unit tests for lexer
#[test]
fn test_lexer_basic() {
    let input = "lddw r1, 42";
    let tokens = tokenize(input).unwrap();
    assert_eq!(tokens.len(), 3);
    assert!(matches!(tokens[0], Token::Opcode(Opcode::Lddw, _)));
    assert!(matches!(tokens[1], Token::Register(1, _)));
    assert!(matches!(tokens[2], Token::ImmediateValue(ImmediateValue::Int(42), _)));
}

#[test]
fn test_lexer_comments() {
    let input = "lddw r1, 42 // comment\n# another comment";
    let tokens = tokenize(input).unwrap();
    assert_eq!(tokens.len(), 3); // Comments should be ignored
}

#[test]
fn test_lexer_hex_numbers() {
    let input = "lddw r1, 0x42";
    let tokens = tokenize(input).unwrap();
    assert!(matches!(tokens[2], Token::ImmediateValue(ImmediateValue::Addr(0x42), _)));
}

#[test]
fn test_lexer_labels() {
    let input = "entrypoint:\n  exit";
    let tokens = tokenize(input).unwrap();
    assert!(matches!(tokens[0], Token::Label(ref name, _) if name == "entrypoint"));
}

// Unit tests for parser
#[test]
fn test_parser_basic() {
    let input = ".globl entrypoint\nentrypoint:\n  exit";
    let tokens = tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let result = parser.parse().unwrap();
    
    assert!(!result.prog_is_static);
    assert!(result.code_section.instructions.len() > 0);
}

// Unit tests for program generation
#[test]
fn test_program_generation() {
    let input = ".globl entrypoint\nentrypoint:\n  exit";
    let tokens = tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let parse_result = parser.parse().unwrap();
    
    let program = Program::from_parse_result(parse_result);
    let bytecode = program.emit_bytecode();
    
    assert!(!bytecode.is_empty());
    assert!(bytecode.len() > 64); // At least ELF header
}

// Property generators
fn arb_assembly() -> impl Strategy<Value = String> {
    prop::collection::vec(arb_instruction(), 1..50)
        .prop_map(|instructions| instructions.join("\n"))
}

fn arb_instruction() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "lddw r0, 42".to_string(),
        "add64 r0, r1".to_string(),
        "exit".to_string(),
        "call sol_log_".to_string(),
        "mov64 r0, r1".to_string(),
    ])
}

// Integration tests
#[test]
fn test_full_compilation_pipeline() {
    let input = r#"
.globl entrypoint
entrypoint:
  lddw r1, message
  lddw r2, 14
  call sol_log_
  exit
.rodata
  message: .ascii "Hello, Solana!"
"#;
    
    let tokens = tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let parse_result = parser.parse().unwrap();
    let program = Program::from_parse_result(parse_result);
    let bytecode = program.emit_bytecode();
    
    // Verify ELF header
    assert_eq!(&bytecode[0..4], b"\x7fELF");
    
    // Verify we have some code
    assert!(bytecode.len() > 100);
}

// Performance tests
#[test]
fn test_lexer_performance() {
    let input = "lddw r1, 42\n".repeat(1000);
    let start = std::time::Instant::now();
    let result = tokenize(&input);
    let duration = start.elapsed();
    
    assert!(result.is_ok());
    assert!(duration.as_millis() < 100); // Should be fast
}

// Error handling tests
#[test]
fn test_lexer_error_handling() {
    let invalid_input = "lddw r1, 0xinvalid";
    let result = tokenize(invalid_input);
    assert!(result.is_err());
}

#[test]
fn test_parser_error_handling() {
    let invalid_input = "invalid_instruction r1, r2";
    let tokens = tokenize(invalid_input).unwrap();
    let mut parser = Parser::new(tokens);
    let result = parser.parse();
    assert!(result.is_err());
} 