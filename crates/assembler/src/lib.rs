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
pub mod debug;
pub mod debuginfo;

// WASM bindings
#[cfg(target_arch = "wasm32")]
pub mod wasm;

pub use self::{
    astnode::ASTNode,
    debug::DebugData,
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
    let program = Program::from_parse_result(parse_result, None);
    let bytecode = program.emit_bytecode();
    Ok(bytecode)
}

pub fn assemble_with_debug_data(
    source: &str,
    filename: &str,
    directory: &str,
) -> Result<Vec<u8>, Vec<CompileError>> {
    let parse_result = match parse(source) {
        Ok(result) => result,
        Err(errors) => {
            return Err(errors);
        }
    };

    // Collect line entries and labels from parse result
    let lines = collect_line_entries(source, &parse_result);
    let labels = collect_labels(source, &parse_result);
    let code_end = parse_result.code_section.get_size();

    let debug_data = DebugData {
        filename: filename.to_string(),
        directory: directory.to_string(),
        lines,
        labels,
        code_start: 0,
        code_end,
    };

    let program = Program::from_parse_result(parse_result, Some(debug_data));
    let bytecode = program.emit_bytecode();
    Ok(bytecode)
}

// Helper function to collect line entries
fn collect_line_entries(source: &str, parse_result: &ParseResult) -> Vec<(u64, u32)> {
    let mut line_starts = vec![0usize];
    for (i, c) in source.char_indices() {
        if c == '\n' {
            line_starts.push(i + 1);
        }
    }

    let mut entries = Vec::new(); // Vec<(address, line)>
    for node in parse_result.code_section.get_nodes() {
        if let ASTNode::Instruction {
            instruction,
            offset,
        } = node
        {
            let line = line_starts.partition_point(|&start| start <= instruction.span.start) as u32;
            entries.push((*offset, line));
        }
    }

    entries
}

// Helper function to collect labels
fn collect_labels(source: &str, parse_result: &ParseResult) -> Vec<(String, u64, u32)> {
    let mut line_starts = vec![0usize];
    for (i, c) in source.char_indices() {
        if c == '\n' {
            line_starts.push(i + 1);
        }
    }

    let mut labels = Vec::new(); // Vec<(name, address, line)>
    for node in parse_result.code_section.get_nodes() {
        if let ASTNode::Label { label, offset } = node {
            let line = line_starts.partition_point(|&start| start <= label.span.start) as u32;
            labels.push((label.name.clone(), *offset, line));
        }
    }

    labels
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

    #[test]
    fn test_assemble_with_debug_data() {
        let source = r#".equ MSG_LEN, 14

.globl entrypoint
entrypoint:
  lddw r1, message
  mov64 r2, MSG_LEN
  call sol_log_
  exit
.rodata
  message: .ascii "Hello, Solana!"
"#;
        let result = assemble_with_debug_data(source, "hello_solana.s", "/tmp");
        assert!(result.is_ok());
        let bytecode = result.unwrap();

        // Verify the ELF has all debug sections.
        let bytecode_str = String::from_utf8_lossy(&bytecode);
        assert!(
            bytecode_str.contains(".debug_abbrev"),
            "Missing .debug_abbrev section"
        );
        assert!(
            bytecode_str.contains(".debug_info"),
            "Missing .debug_info section"
        );
        assert!(
            bytecode_str.contains(".debug_line"),
            "Missing .debug_line section"
        );
        assert!(
            bytecode_str.contains(".debug_line_str"),
            "Missing .debug_line_str section"
        );
    }
}
