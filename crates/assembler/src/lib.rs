use anyhow::Result;

// Parser
pub mod parser;

// Error handling and diagnostics
pub mod errors;
pub mod macros;
pub mod messages;

// Intermediate Representation
pub mod ast;
pub mod astnode;
pub mod dynsym;
pub mod syscall;

// ELF header, program, section
pub mod header;
pub mod program;
pub mod section;

// Debug info
pub mod debuginfo;

// WASM bindings
#[cfg(target_arch = "wasm32")]
pub mod wasm;

pub use self::{
    errors::CompileError,
    parser::{ParseResult, Token, parse},
    program::Program,
};

pub fn assemble(source: &str) -> Result<Vec<u8>, Vec<CompileError>> {
    let parse_result = match parse(source) {
        Ok(result) => result,
        Err(errors) => {
            return Err(errors);
        }
    };
    let program = Program::from_parse_result(parse_result);
    let bytecode = program.emit_bytecode();
    Ok(bytecode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assemble_success() {
        let source = "exit";
        let result = assemble(source);
        assert!(result.is_ok());
        let bytecode = result.unwrap();
        assert!(!bytecode.is_empty());
    }

    #[test]
    fn test_assemble_parse_error() {
        let source = "invalid_xyz";
        let result = assemble(source);
        assert!(result.is_err());
    }

    #[test]
    fn test_assemble_with_equ_directive() {
        let source = r#"
        .globl entrypoint
        .equ MY_CONST, 42
        entrypoint:
            mov64 r1, MY_CONST
            exit
        "#;
        let result = assemble(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_duplicate_label_error() {
        let source = r#"
        .globl entrypoint
        entrypoint:
            mov64 r1, 1
        entrypoint:
            exit
        "#;
        let result = assemble(source);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_assemble_extern_directive() {
        let source = r#"
        .globl entrypoint
        .extern my_extern_symbol
        entrypoint:
            exit
        "#;
        let result = assemble(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_rodata_section() {
        let source = r#"
        .globl entrypoint
        .rodata
        my_data: .ascii "hello"
        .text
        entrypoint:
            exit
        "#;
        let result = assemble(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_rodata_byte() {
        let source = r#"
        .globl entrypoint
        .rodata
        my_byte: .byte 0x42
        .text
        entrypoint:
            exit
        "#;
        let result = assemble(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_rodata_multiple_bytes() {
        let source = r#"
        .globl entrypoint
        .rodata
        my_bytes: .byte 0x01, 0x02, 0x03, 0x04
        .text
        entrypoint:
            exit
        "#;
        let result = assemble(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_rodata_mixed() {
        let source = r#"
        .globl entrypoint
        .rodata
        data1: .byte 0x42
        data2: .ascii "test"
        .text
        entrypoint:
            exit
        "#;
        let result = assemble(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_jump_operations() {
        let source = r#"
        .globl entrypoint
        entrypoint:
            jeq r1, 0, +1
            ja +2
        target:
            jne r1, r2, target
            exit
        "#;
        let result = assemble(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_offset_expression() {
        let source = r#"
        .globl entrypoint
        .equ BASE, 100
        entrypoint:
            mov64 r1, BASE+10
            exit
        "#;
        let result = assemble(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_equ_expression() {
        let source = r#"
        .globl entrypoint
        .equ BASE, 100
        .equ OFFSET, 20
        .equ COMPUTED, BASE
        entrypoint:
            mov64 r1, BASE
            mov64 r2, OFFSET
            mov64 r3, COMPUTED
            exit
        "#;
        let result = assemble(source);
        assert!(result.is_ok());
    }
}
