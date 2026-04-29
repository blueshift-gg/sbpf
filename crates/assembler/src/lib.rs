use {anyhow::Result, codespan::Files};

// Parser
pub mod parser;

// Preprocessor (include + macro expansion)
pub mod preprocessor;

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
    preprocessor::{
        FileResolver, FsFileResolver, MockFileResolver, PreprocessResult, preprocess,
        source_map::{FileRegistry, SourceMap, SourceOrigin},
    },
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

/// An error enriched with source location information from preprocessing.
/// Wraps a `CompileError` with the resolved original source location.
#[derive(Debug)]
pub struct AssemblerError {
    pub error: CompileError,
    pub origin: Option<SourceOrigin>,
    /// Column offset (0-based) within the original line, if known.
    pub column: Option<usize>,
}

impl std::fmt::Display for AssemblerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error)
    }
}

impl std::error::Error for AssemblerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// Returned when assembly with preprocessing fails.
/// Contains the errors and the file registry so callers can render
/// diagnostics against the original source files.
#[derive(Debug)]
pub struct AssembleErrors {
    pub errors: Vec<AssemblerError>,
    pub file_registry: FileRegistry,
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

    /// Assemble source code directly (no preprocessing).
    /// This is the original API -- macros and includes are not supported.
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

    /// Assemble with preprocessing: resolves `.include` and expands `.macro` directives
    /// before parsing. Errors include source location information from the source map,
    /// and the file registry is returned so callers can render diagnostics against
    /// original source files.
    pub fn assemble_with_preprocess(
        &self,
        source: &str,
        source_path: &str,
        resolver: Option<&dyn FileResolver>,
    ) -> Result<Vec<u8>, AssembleErrors> {
        // Run preprocessor
        let preprocess_result =
            preprocess(source, source_path, resolver).map_err(|failure| AssembleErrors {
                errors: failure
                    .errors
                    .into_iter()
                    .map(|e| AssemblerError {
                        error: e.error,
                        origin: e.origin,
                        column: None,
                    })
                    .collect(),
                file_registry: failure.file_registry,
            })?;

        let expanded = &preprocess_result.expanded_source;
        let source_map = &preprocess_result.source_map;

        // Parse the expanded source
        let parse_result = match parse(expanded, self.options.arch) {
            Ok(result) => result,
            Err(errors) => {
                // Extract file registry from source map before moving errors
                let file_registry = source_map.file_registry.clone();
                return Err(AssembleErrors {
                    errors: errors
                        .into_iter()
                        .map(|e| {
                            let span = e.span();
                            let origin = source_map.resolve_span(span, expanded).clone();
                            // Compute column offset within the line from the
                            // expanded source so we can highlight the right token.
                            let col = expanded[..span.start]
                                .rfind('\n')
                                .map(|nl| span.start - nl - 1)
                                .unwrap_or(span.start);
                            AssemblerError {
                                error: e,
                                column: Some(col),
                                origin: Some(origin),
                            }
                        })
                        .collect(),
                    file_registry,
                });
            }
        };

        // Build debug data if debug mode is enabled
        let debug_data = if let Some(ref debug_mode) = self.options.debug_mode {
            let (lines, labels) = collect_line_and_label_entries(expanded, &parse_result);
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

    /// Convenience method: read a file from disk and assemble with full preprocessing.
    pub fn assemble_file(&self, path: &std::path::Path) -> Result<Vec<u8>, AssembleErrors> {
        let source = std::fs::read_to_string(path).map_err(|e| AssembleErrors {
            errors: vec![AssemblerError {
                error: CompileError::IncludeReadError {
                    path: path.display().to_string(),
                    reason: e.to_string(),
                    span: 0..0,
                    custom_label: Some("Failed to read source file".to_string()),
                },
                origin: None,
                column: None,
            }],
            file_registry: FileRegistry::new(),
        })?;

        let source_path = path.to_string_lossy();
        let resolver = FsFileResolver::new();
        self.assemble_with_preprocess(&source, &source_path, Some(&resolver))
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

    for node in parse_result.data_section.get_nodes() {
        if let ASTNode::ROData { rodata, offset } = node {
            let line_index = files.line_index(file_id, rodata.span.start as u32);
            let line_number = (line_index.to_usize() + 1) as u32;
            label_entries.push((rodata.name.clone(), *offset, line_number));
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
    fn test_parse_error_has_correct_span() {
        // Verify parse errors point to the actual error position,
        // not span 0..len (which would lose position info for source maps).
        let source = ".rodata\n    thing2: .bogus 5\n.text\nentrypoint:\n    exit\n";
        let result = assemble(source);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        let span = errors[0].span();
        // The error should point somewhere near ".bogus" (byte ~20), NOT 0..len
        assert!(
            span.start > 0,
            "Error span should not start at 0, got {:?}",
            span
        );
        assert!(
            span.end < source.len(),
            "Error span should not cover entire source, got {:?}",
            span
        );
        // Verify the span actually points at ".bogus", not at "thing2"
        let error_text = &source[span.start..span.end.min(source.len())];
        assert!(
            error_text.starts_with('.'),
            "Error should point at '.bogus', but points at: '{}'",
            &source[span.start..span.start + 10.min(source.len() - span.start)]
        );
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
    fn test_assemble_label_arithmetic_rodata_length() {
        // The primary use case: compute string length via label subtraction
        let source = r#"
        .globl entrypoint
        .rodata
        msg: .ascii "Hello"
        msg_end:
        .text
        entrypoint:
            lddw r1, msg
            mov64 r2, msg_end - msg
            exit
        "#;
        let result = assemble(source);
        assert!(result.is_ok(), "Failed: {:?}", result.err());
    }

    #[test]
    fn test_assemble_label_arithmetic_with_offset() {
        // Label arithmetic with additional constant offset
        let source = r#"
        .globl entrypoint
        .rodata
        msg: .ascii "Hello!"
        msg_end:
        .text
        entrypoint:
            lddw r1, msg
            mov64 r2, msg_end - msg - 1
            exit
        "#;
        let result = assemble(source);
        assert!(result.is_ok(), "Failed: {:?}", result.err());
    }

    #[test]
    fn test_assemble_label_arithmetic_text_section() {
        // Label arithmetic works in the text section too
        let source = r#"
        .globl entrypoint
        entrypoint:
            mov64 r1, 1
        middle:
            mov64 r2, 2
        end:
            mov64 r3, end - entrypoint
            exit
        "#;
        let result = assemble(source);
        assert!(result.is_ok(), "Failed: {:?}", result.err());
    }

    #[test]
    fn test_assemble_label_arithmetic_forward_reference() {
        // Text section before rodata — forward references to rodata labels
        let source = r#"
        .globl entrypoint
        entrypoint:
            lddw r1, message
            mov64 r2, message_end - message
            call sol_log_
            exit
            lddw r10, 1
        .rodata
            message: .ascii "Hello, Solana!"
            message_end:
        "#;
        let result = assemble(source);
        assert!(
            result.is_ok(),
            "Forward reference failed: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_assemble_label_arithmetic_multiline_rodata() {
        // Rodata label and directive on separate lines (as from macro expansion)
        let source = r#"
        .globl entrypoint
        entrypoint:
            lddw r1, message
            mov64 r2, message_end - message
            call sol_log_
            exit
        .rodata
        message:
            .ascii "Hello, Solana!"
        message_end:
        "#;
        let result = assemble(source);
        assert!(
            result.is_ok(),
            "Multi-line rodata failed: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_assemble_label_arithmetic_macro_e2e() {
        // Full end-to-end test with macro expansion + label arithmetic
        let source = r#"
.macro DEF_STR name, text
\name:
    .ascii \text
\name\()_end:
.endm

.macro SOL_LOG name
    lddw r1, \name
    mov64 r2, \name\()_end - \name
    call sol_log_
.endm

.globl entrypoint
entrypoint:
    SOL_LOG message
    exit
.rodata
    DEF_STR message, "Hello, Solana!"
"#;
        let assembler = Assembler::new(AssemblerOption::default());
        let result = assembler.assemble_with_preprocess(source, "test.s", None);
        assert!(result.is_ok(), "Macro e2e failed: {:?}", result.err());
    }

    #[test]
    fn test_assemble_label_arithmetic_cross_section_error() {
        // Cross-section arithmetic should fail
        let source = r#"
        .globl entrypoint
        .rodata
        msg: .ascii "Hello"
        .text
        entrypoint:
            mov64 r1, msg - entrypoint
            exit
        "#;
        let result = assemble(source);
        assert!(result.is_err(), "Cross-section arithmetic should fail");
    }

    #[test]
    fn test_assemble_label_arithmetic_complex_expression() {
        // More complex expression with multiple rodata entries
        let source = r#"
        .globl entrypoint
        .rodata
        str1: .ascii "Hello"
        str2: .ascii " World"
        str2_end:
        .text
        entrypoint:
            lddw r1, str2
            mov64 r2, str2_end - str2
            exit
        "#;
        let result = assemble(source);
        assert!(result.is_ok(), "Failed: {:?}", result.err());
    }

    #[test]
    fn test_parse_error_column_through_preprocess() {
        // Verify the column offset is correctly computed through the
        // preprocessing pipeline so errors point at the invalid token,
        // not at the label before it.
        let source = ".globl e\ne:\n    exit\n.rodata\n    thing1: .bogus 5\n";
        let assembler = Assembler::new(AssemblerOption::default());
        let result = assembler.assemble_with_preprocess(source, "test.s", None);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        let err = &errors.errors[0];
        let origin = err.origin.as_ref().expect("Expected origin");
        let registry = &errors.file_registry;

        // Verify the column points at ".bogus", not "thing1"
        let col = err.column.expect("Expected column info");
        let line_start = registry.line_byte_offset(origin.file_id, origin.line);
        let content = registry.content(origin.file_id);
        let at_col = &content[line_start + col..];
        assert!(
            at_col.starts_with(".bogus"),
            "Expected column to point at '.bogus', but points at: '{}'",
            &at_col[..at_col.len().min(20)]
        );
    }

    #[test]
    fn test_parse_error_column_with_include_and_macros() {
        // Simulate the user's actual scenario: .include with macros,
        // then an invalid directive on a later line.
        use std::collections::HashMap;

        struct TestResolver {
            files: HashMap<String, String>,
        }
        impl FileResolver for TestResolver {
            fn resolve(&self, path: &str, _base: &str) -> Result<String, std::io::Error> {
                self.files.get(path).cloned().ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        format!("not found: {}", path),
                    )
                })
            }
        }

        let mut files = HashMap::new();
        files.insert(
            "syscalls/sol_log.s".to_string(),
            r#".macro SOL_LOG name
    lddw r1, \name
    mov64 r2, \name\()_end - \name
    call sol_log_
.endm

.macro DEF_STR name, text
\name:
    .ascii \text
\name\()_end:
.endm
"#
            .to_string(),
        );

        let source = r#".include "syscalls/sol_log.s"
.globl e
e:
    SOL_LOG message
.rodata
    DEF_STR message, "TEST"
    thing2: .int 1
    thing1: .bogus "text"
"#;
        let resolver = TestResolver { files };
        let assembler = Assembler::new(AssemblerOption::default());
        let result = assembler.assemble_with_preprocess(source, "main.s", Some(&resolver));
        assert!(result.is_err());
        let errors = result.unwrap_err();
        let err = &errors.errors[0];
        let origin = err.origin.as_ref().expect("Expected origin");
        let registry = &errors.file_registry;

        // Verify origin points to main.s, not the included file
        assert_eq!(registry.path(origin.file_id), "main.s");

        // Verify the column points at ".bogus"
        let col = err.column.expect("Expected column info");
        let line_start = registry.line_byte_offset(origin.file_id, origin.line);
        let content = registry.content(origin.file_id);
        let at_col = &content[line_start + col..];
        assert!(
            at_col.starts_with(".bogus"),
            "Expected column to point at '.bogus', but at line {} col {}, points at: '{}'",
            origin.line,
            col,
            &at_col[..at_col.len().min(20)]
        );
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
