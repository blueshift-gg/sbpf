use std::{fs::File, io::Read, path::Path, path::PathBuf};

use either::Either;
use sbpf_assembler::{Assembler, AssemblerOption, DebugMode};
use sbpf_common::{inst_param::Number, opcode::Opcode};
use sbpf_disassembler::program::Program;
use sbpf_vm::{
    memory::Memory,
    vm::{SbpfVm, SbpfVmConfig},
};

use crate::{
    debugger::Debugger,
    error::{DebuggerError, DebuggerResult},
    parser::{LineMap, rodata_from_section},
    syscalls::DebuggerSyscallHandler,
};

pub struct DebuggerSession {
    pub debugger: Debugger<DebuggerSyscallHandler>,
    pub line_map: Option<LineMap>,
    pub elf_bytes: Vec<u8>,
    pub elf_path: PathBuf,
}

pub fn load_session_from_asm(
    asm_path: &str,
    input: Vec<u8>,
    config: SbpfVmConfig,
) -> DebuggerResult<DebuggerSession> {
    let asm_path = Path::new(asm_path);
    if !asm_path.exists() {
        return Err(DebuggerError::InvalidInput(format!(
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
        use_static_syscalls: false,
        debug_mode: Some(DebugMode {
            filename,
            directory,
        }),
    };
    let assembler = Assembler::new(options);
    let bytecode = assembler
        .assemble(&source_code)
        .map_err(|errors| DebuggerError::Assembler(format!("{:?}", errors)))?;

    load_session_from_bytes(bytecode, input, config, None)
}

pub fn load_session_from_elf(
    elf_path: &str,
    input: Vec<u8>,
    config: SbpfVmConfig,
) -> DebuggerResult<DebuggerSession> {
    let mut file = File::open(elf_path)?;
    let mut elf_bytes = Vec::new();
    file.read_to_end(&mut elf_bytes)?;
    load_session_from_bytes(elf_bytes, input, config, Some(elf_path.into()))
}

pub fn load_session_from_bytes(
    elf_bytes: Vec<u8>,
    input: Vec<u8>,
    config: SbpfVmConfig,
    elf_path: Option<PathBuf>,
) -> DebuggerResult<DebuggerSession> {
    let program = Program::from_bytes(&elf_bytes)?;
    let entrypoint = program.get_entrypoint_offset().unwrap_or(0);
    let (mut instructions, rodata_section) = program.to_ixs(false)?;
    let rodata_bytes = rodata_section
        .as_ref()
        .map(|section| section.data.clone())
        .unwrap_or_default();

    // Remap rodata addresses from ELF addresses to VM addresses
    if let Some(ref section) = rodata_section {
        let elf_rodata_base = section.base_address;
        let elf_rodata_end = elf_rodata_base + section.data.len() as u64;

        for ix in &mut instructions {
            if ix.opcode == Opcode::Lddw {
                if let Some(Either::Right(Number::Int(imm))) = &ix.imm {
                    let addr = *imm as u64;
                    if addr >= elf_rodata_base && addr < elf_rodata_end {
                        let offset = addr - elf_rodata_base;
                        let vm_addr = Memory::RODATA_START + offset;
                        ix.imm = Some(Either::Right(Number::Int(vm_addr as i64)));
                    }
                }
            }
        }
    }

    let mut vm = SbpfVm::new_with_config(
        instructions,
        input,
        rodata_bytes,
        DebuggerSyscallHandler::default(),
        config,
    );
    vm.set_entrypoint(entrypoint as usize);

    let mut debugger = Debugger::new(vm);
    if let Ok(line_map) = LineMap::from_elf_data(&elf_bytes) {
        debugger.set_dwarf_line_map(line_map.clone());
    }

    // Set rodata symbols from the disassembler's parsed section
    if let Some(ref section) = rodata_section {
        let rodata_symbols = rodata_from_section(section);
        if !rodata_symbols.is_empty() {
            debugger.set_rodata(rodata_symbols);
        }
    }

    Ok(DebuggerSession {
        line_map: debugger.dwarf_line_map.clone(),
        debugger,
        elf_bytes,
        elf_path: elf_path.unwrap_or_else(|| PathBuf::from("<memory>")),
    })
}

pub fn parse_input(input: &str) -> DebuggerResult<Vec<u8>> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(Vec::new());
    }

    if input.contains('/') || input.contains('\\') || input.ends_with(".hex") {
        let path = Path::new(input);
        if !path.exists() {
            return Err(DebuggerError::InvalidInput(format!(
                "File not found: {}",
                input
            )));
        }

        let mut file = File::open(path).map_err(|e| DebuggerError::InvalidInput(e.to_string()))?;
        let mut content = String::new();
        file.read_to_string(&mut content)
            .map_err(|e| DebuggerError::InvalidInput(e.to_string()))?;
        parse_hex(&content)
    } else {
        parse_hex(input)
    }
}

fn parse_hex(hex: &str) -> DebuggerResult<Vec<u8>> {
    let hex = hex.trim();
    let hex = if hex.starts_with("0x") || hex.starts_with("0X") {
        &hex[2..]
    } else {
        hex
    };

    if hex.len() % 2 != 0 {
        return Err(DebuggerError::InvalidInput(
            "Hex string must have even length".to_string(),
        ));
    }

    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        let byte_str = &hex[i..i + 2];
        let byte = u8::from_str_radix(byte_str, 16)
            .map_err(|_| DebuggerError::InvalidInput(format!("Invalid hex: {}", byte_str)))?;
        bytes.push(byte);
    }

    Ok(bytes)
}
