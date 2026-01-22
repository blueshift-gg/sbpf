use {anyhow::Result, codespan::Files};

// Parser
pub mod parser;

// Error handling and diagnostics
pub mod errors;
pub mod macros;

// Intermediate Representation
pub mod ast;
pub mod astnode;
pub mod dynsym;

// ELF header, program, section
pub mod header;
pub mod program;
pub mod section;

// Debug info
pub mod debug;

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

/// sBPF target architecture
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SbpfArch {
    #[default]
    V0,
    V3,
}

impl SbpfArch {
    pub fn is_v3(&self) -> bool {
        matches!(self, SbpfArch::V3)
    }

    pub fn e_flags(&self) -> u32 {
        match self {
            SbpfArch::V0 => 0,
            SbpfArch::V3 => 3,
        }
    }
}

/// Debug mode configuration for the assembler
#[derive(Debug, Clone)]
pub struct DebugMode {
    /// Source filename for debug info
    pub filename: String,
    /// Source directory for debug info
    pub directory: String,
}

/// Options for the assembler
#[derive(Debug, Clone, Default)]
pub struct AssemblerOption {
    /// sBPF target architecture
    pub arch: SbpfArch,
    /// Optional debug mode configuration
    pub debug_mode: Option<DebugMode>,
}

/// Assembler for SBPF assembly code
#[derive(Debug, Clone)]
pub struct Assembler {
    options: AssemblerOption,
}

impl Assembler {
    /// Create a new Assembler with the given options
    pub fn new(options: AssemblerOption) -> Self {
        Self { options }
    }

    pub fn assemble(&self, source: &str) -> Result<Vec<u8>, Vec<CompileError>> {
        let parse_result = match parse(source, self.options.arch) {
            Ok(result) => result,
            Err(errors) => {
                return Err(errors);
            }
        };

        // Build debug data if debug mode is enabled
        let debug_data = if let Some(ref debug_mode) = self.options.debug_mode {
            let (lines, labels) = collect_line_and_label_entries(source, &parse_result);
            let code_end = parse_result.code_section.get_size();

            Some(DebugData {
                filename: debug_mode.filename.clone(),
                directory: debug_mode.directory.clone(),
                lines,
                labels,
                code_start: 0,
                code_end,
            })
        } else {
            None
        };

        let program = Program::from_parse_result(parse_result, debug_data);
        let bytecode = program.emit_bytecode();
        Ok(bytecode)
    }
}

type LineEntry = (u64, u32); // (offset, line)
type LabelEntry = (String, u64, u32); // (label, offset, line)

/// Helper function to collect line and label entries
fn collect_line_and_label_entries(
    source: &str,
    parse_result: &ParseResult,
) -> (Vec<LineEntry>, Vec<LabelEntry>) {
    let mut files: Files<&str> = Files::new();
    let file_id = files.add("source", source);

    let mut line_entries = Vec::new();
    let mut label_entries = Vec::new();

    for node in parse_result.code_section.get_nodes() {
        match node {
            ASTNode::Instruction {
                instruction,
                offset,
            } => {
                let line_index = files.line_index(file_id, instruction.span.start as u32);
                let line_number = (line_index.to_usize() + 1) as u32;
                line_entries.push((*offset, line_number));
            }
            ASTNode::Label { label, offset } => {
                let line_index = files.line_index(file_id, label.span.start as u32);
                let line_number = (line_index.to_usize() + 1) as u32;
                label_entries.push((label.name.clone(), *offset, line_number));
            }
            _ => {}
        }
    }

    (line_entries, label_entries)
}

#[cfg(test)]
pub fn assemble(source: &str) -> Result<Vec<u8>, Vec<CompileError>> {
    let options = AssemblerOption::default();
    let assembler = Assembler::new(options);
    assembler.assemble(source)
}

#[cfg(test)]
pub fn assemble_with_debug_data(
    source: &str,
    filename: &str,
    directory: &str,
) -> Result<Vec<u8>, Vec<CompileError>> {
    let options = AssemblerOption {
        arch: SbpfArch::V0,
        debug_mode: Some(DebugMode {
            filename: filename.to_string(),
            directory: directory.to_string(),
        }),
    };
    let assembler = Assembler::new(options);
    assembler.assemble(source)
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
