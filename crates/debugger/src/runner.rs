use {
    crate::{
        debugger::Debugger,
        error::DebuggerResult,
        input::ParsedInput,
        parser::{LineMap, rodata_from_section},
    },
    sbpf_assembler::{Assembler, AssemblerOption, DebugMode, SbpfArch},
    sbpf_disassembler::program::Program,
    sbpf_runtime::{Runtime, config::RuntimeConfig},
    sbpf_vm::memory::Memory,
    std::path::{Path, PathBuf},
};

pub struct DebuggerSession {
    pub debugger: Debugger,
    pub line_map: Option<LineMap>,
    pub elf_bytes: Vec<u8>,
    pub elf_path: PathBuf,
}

pub fn load_session_from_asm(
    asm_path: &str,
    parsed: ParsedInput,
    config: RuntimeConfig,
) -> DebuggerResult<DebuggerSession> {
    let asm_path = Path::new(asm_path);
    if !asm_path.exists() {
        return Err(crate::error::DebuggerError::InvalidInput(format!(
            "Assembly file not found: {}",
            asm_path.display()
        )));
    }

    let source_code = std::fs::read_to_string(asm_path)?;
    let filename = asm_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown.s")
        .to_string();
    let directory = asm_path
        .parent()
        .and_then(|p| p.canonicalize().ok())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string());

    let options = AssemblerOption {
        arch: SbpfArch::V0,
        debug_mode: Some(DebugMode {
            filename,
            directory,
        }),
    };
    let assembler = Assembler::new(options);
    let bytecode = assembler
        .assemble(&source_code)
        .map_err(|errors| crate::error::DebuggerError::Assembler(format!("{:?}", errors)))?;

    load_session_from_bytes(bytecode, parsed, config, None)
}

pub fn load_session_from_elf(
    elf_path: &str,
    parsed: ParsedInput,
    config: RuntimeConfig,
) -> DebuggerResult<DebuggerSession> {
    let elf_bytes = std::fs::read(elf_path)?;
    load_session_from_bytes(elf_bytes, parsed, config, Some(elf_path.into()))
}

fn load_session_from_bytes(
    elf_bytes: Vec<u8>,
    parsed: ParsedInput,
    config: RuntimeConfig,
    elf_path: Option<PathBuf>,
) -> DebuggerResult<DebuggerSession> {
    let mut runtime = Runtime::new(parsed.instruction.program_id, elf_bytes.clone(), config)?;
    runtime.prepare(&parsed.instruction, &parsed.accounts)?;

    let mut debugger = Debugger::new(runtime);
    if let Ok(line_map) = LineMap::from_elf_data(&elf_bytes) {
        debugger.set_dwarf_line_map(line_map);
    }

    if let Ok(program) = Program::from_bytes(&elf_bytes) {
        if let Ok((_, rodata_section)) = program.to_ixs() {
            // Set rodata symbols from the disassembler's parsed section
            if let Some(ref section) = rodata_section {
                let mut rodata_symbols = rodata_from_section(section);
                // Replace generated labels with actual labels from DWARF info (if available).
                if let Some(ref line_map) = debugger.dwarf_line_map {
                    let text_offset = line_map.get_text_offset();
                    for sym in &mut rodata_symbols {
                        let rodata_offset = sym.address - Memory::RODATA_START;
                        let addr = rodata_offset + text_offset;
                        if let Some(name) = line_map.get_label_for_address(addr) {
                            sym.name = name.to_string();
                        }
                    }
                }
                if !rodata_symbols.is_empty() {
                    debugger.set_rodata(rodata_symbols);
                }
            }
        }
    }

    Ok(DebuggerSession {
        line_map: debugger.dwarf_line_map.clone(),
        debugger,
        elf_bytes,
        elf_path: elf_path.unwrap_or_else(|| PathBuf::from("<memory>")),
    })
}
