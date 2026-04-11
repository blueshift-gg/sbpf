use {
    anyhow::Result,
    codespan::Files,
    std::{collections::HashMap, path::Path},
};

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
    parser::{ParseResult, Token, parse, parse_with_base_path},
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

/// Result of `Assembler::assemble_with_base_path`. Carries the source
/// registry populated by `.include` handling alongside the bytecode
/// result, so callers can render multi-file diagnostics on error.
pub struct AssembleResult {
    /// Every source file read during parsing, keyed by the identifier
    /// used in diagnostics (main file name or the relative `.include`
    /// path). Populated on both success and failure.
    pub sources: HashMap<String, String>,
    /// The assembled bytecode, or a list of compile errors.
    pub bytecode: Result<Vec<u8>, Vec<CompileError>>,
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

    /// Assemble a single source string. Does not support `.include`;
    /// callers that need multi-file assembly should use
    /// `assemble_with_base_path`.
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
                lines_multi: Vec::new(),
                labels_multi: Vec::new(),
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

    /// Assemble `source` with `.include` support.
    ///
    /// * `main_file_name` — identifier used for the main file in
    ///   diagnostics and DWARF debug info (typically the file's basename).
    /// * `source` — contents of the main file.
    /// * `base_path` — directory used to resolve `.include` paths in the
    ///   main file (nested includes are resolved relative to the
    ///   including file's directory).
    ///
    /// Returns `AssembleResult` with the full source registry (main plus
    /// every file pulled in via `.include`) so callers can produce
    /// multi-file diagnostics even when assembly fails. The registry is
    /// populated in both the success and failure paths.
    pub fn assemble_with_base_path(
        &self,
        main_file_name: &str,
        source: &str,
        base_path: &Path,
    ) -> AssembleResult {
        let mut sources: HashMap<String, String> = HashMap::new();
        let parse_result = match parse_with_base_path(
            source,
            self.options.arch,
            Some(base_path),
            main_file_name,
            &mut sources,
        ) {
            Ok(result) => result,
            Err(errors) => {
                // Parsing failed. `sources` has every file read during
                // parsing (main + any included files that were read
                // before the error), so multi-file diagnostics can still
                // be rendered by the caller.
                return AssembleResult {
                    sources,
                    bytecode: Err(errors),
                };
            }
        };

        let debug_data = if let Some(ref debug_mode) = self.options.debug_mode {
            let (lines, labels, lines_multi, labels_multi) =
                collect_multi_file_line_entries(&parse_result, debug_mode);
            let code_end = parse_result.code_section.get_size();

            Some(DebugData {
                filename: debug_mode.filename.clone(),
                directory: debug_mode.directory.clone(),
                lines,
                labels,
                lines_multi,
                labels_multi,
                code_start: 0,
                code_end,
            })
        } else {
            None
        };

        let program = Program::from_parse_result(parse_result, debug_data);
        let bytecode = program.emit_bytecode();
        AssembleResult {
            sources,
            bytecode: Ok(bytecode),
        }
    }
}

type LineEntry = (u64, u32); // (offset, line)
type LabelEntry = (String, u64, u32); // (label, offset, line)
type LineMultiEntry = (u64, String, String, u32); // (offset, file, dir, line)
type LabelMultiEntry = (String, u64, String, String, u32); // (name, offset, file, dir, line)

/// Helper function to collect line and label entries for single-file
/// sources (no `.include`).
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

/// Collect line and label entries for a multi-file parse result.
///
/// Returns four vectors: the single-file `lines`/`labels` (always based
/// on the main source for backwards compatibility with consumers that
/// only read those), plus `lines_multi`/`labels_multi` which carry per-
/// instruction file/line info drawn from every included file.
///
/// `parse_result.code_file_info` / `rodata_file_info` / `label_file_info`
/// are the parallel maps populated by the parser; this function builds
/// `codespan::Files` over each distinct source so line numbers are
/// relative to the owning file (not a merged view).
fn collect_multi_file_line_entries(
    parse_result: &ParseResult,
    debug_mode: &DebugMode,
) -> (
    Vec<LineEntry>,
    Vec<LabelEntry>,
    Vec<LineMultiEntry>,
    Vec<LabelMultiEntry>,
) {
    use std::collections::HashMap;

    // Build a Files index over every source we read.
    let mut files: Files<String> = Files::new();
    let mut file_ids: HashMap<String, codespan::FileId> = HashMap::new();
    for (name, content) in &parse_result.sources {
        let id = files.add(name.clone(), content.clone());
        file_ids.insert(name.clone(), id);
    }

    let mut offset_to_file: HashMap<u64, String> = HashMap::new();
    for (offset, file) in &parse_result.code_file_info {
        offset_to_file.insert(*offset, file.clone());
    }
    let mut rodata_offset_to_file: HashMap<u64, String> = HashMap::new();
    for (offset, file) in &parse_result.rodata_file_info {
        rodata_offset_to_file.insert(*offset, file.clone());
    }

    let main_name = debug_mode.filename.clone();
    let main_dir = debug_mode.directory.clone();

    let mut lines = Vec::new();
    let mut labels = Vec::new();
    let mut lines_multi = Vec::new();
    let mut labels_multi = Vec::new();

    // Code section: instructions and labels.
    for node in parse_result.code_section.get_nodes() {
        match node {
            ASTNode::Instruction {
                instruction,
                offset,
            } => {
                let file = offset_to_file
                    .get(offset)
                    .cloned()
                    .unwrap_or_else(|| main_name.clone());
                if let Some(file_id) = file_ids.get(&file) {
                    let line_index = files.line_index(*file_id, instruction.span.start as u32);
                    let line_number = (line_index.to_usize() + 1) as u32;
                    // Always populate single-file view (treats everything
                    // as main file) for backwards compat.
                    lines.push((*offset, line_number));
                    // Multi-file view with real file name.
                    let (filename, directory) = split_file_and_dir(&file, &main_name, &main_dir);
                    lines_multi.push((*offset, filename, directory, line_number));
                }
            }
            ASTNode::Label { label, offset } => {
                let file = parse_result
                    .label_file_info
                    .get(&label.name)
                    .cloned()
                    .unwrap_or_else(|| main_name.clone());
                if let Some(file_id) = file_ids.get(&file) {
                    let line_index = files.line_index(*file_id, label.span.start as u32);
                    let line_number = (line_index.to_usize() + 1) as u32;
                    labels.push((label.name.clone(), *offset, line_number));
                    let (filename, directory) = split_file_and_dir(&file, &main_name, &main_dir);
                    labels_multi.push((
                        label.name.clone(),
                        *offset,
                        filename,
                        directory,
                        line_number,
                    ));
                }
            }
            _ => {}
        }
    }

    // ROData: also contribute label entries keyed by their own offset.
    for node in parse_result.data_section.get_nodes() {
        if let ASTNode::ROData { rodata, offset } = node {
            let file = parse_result
                .label_file_info
                .get(&rodata.name)
                .cloned()
                .or_else(|| rodata_offset_to_file.get(offset).cloned())
                .unwrap_or_else(|| main_name.clone());
            if let Some(file_id) = file_ids.get(&file) {
                let line_index = files.line_index(*file_id, rodata.span.start as u32);
                let line_number = (line_index.to_usize() + 1) as u32;
                labels.push((rodata.name.clone(), *offset, line_number));
                let (filename, directory) = split_file_and_dir(&file, &main_name, &main_dir);
                labels_multi.push((
                    rodata.name.clone(),
                    *offset,
                    filename,
                    directory,
                    line_number,
                ));
            }
        }
    }

    (lines, labels, lines_multi, labels_multi)
}

/// Split a parser file identifier into `(filename, directory)` for DWARF.
///
/// The main file uses the directory passed in `DebugMode`. Included files
/// are reported as `(relative_path, main_dir)` — the relative path acts as
/// the filename since that is how the user referenced the file. This
/// keeps the DWARF line table self-consistent without needing to canonicalize
/// absolute paths.
fn split_file_and_dir(file: &str, main_name: &str, main_dir: &str) -> (String, String) {
    if file == main_name {
        (main_name.to_string(), main_dir.to_string())
    } else {
        (file.to_string(), main_dir.to_string())
    }
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
