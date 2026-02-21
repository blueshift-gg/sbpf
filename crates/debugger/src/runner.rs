use {
    crate::{
        debugger::Debugger,
        error::{DebuggerError, DebuggerResult},
        execution_cost::ExecutionCost,
        parser::{LineMap, rodata_from_section},
        syscalls::DebuggerSyscallHandler,
    },
    either::Either,
    sbpf_assembler::{Assembler, AssemblerOption, DebugMode, SbpfArch},
    sbpf_common::{inst_param::Number, opcode::Opcode},
    sbpf_disassembler::program::Program,
    sbpf_vm::{
        compute::ComputeMeter,
        memory::Memory,
        vm::{SbpfVm, SbpfVmConfig},
    },
    solana_address::Address,
    std::{
        fs::File,
        io::Read,
        path::{Path, PathBuf},
    },
};

pub struct DebuggerSession {
    pub debugger: Debugger<DebuggerSyscallHandler>,
    pub line_map: Option<LineMap>,
    pub elf_bytes: Vec<u8>,
    pub elf_path: PathBuf,
}

impl DebuggerSession {
    pub fn build_vm(
        instructions: Vec<sbpf_common::instruction::Instruction>,
        input: Vec<u8>,
        rodata_bytes: Vec<u8>,
        config: SbpfVmConfig,
        program_id: Address,
    ) -> SbpfVm<DebuggerSyscallHandler> {
        let compute_meter = ComputeMeter::new(config.compute_unit_limit);
        let handler = DebuggerSyscallHandler::new(ExecutionCost::default(), program_id);

        let mut vm = SbpfVm::new_with_config(instructions, input, rodata_bytes, handler, config);
        vm.compute_meter = compute_meter;
        vm
    }
}

pub fn load_session_from_asm(
    asm_path: &str,
    input: Vec<u8>,
    config: SbpfVmConfig,
    program_id: Address,
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
        arch: SbpfArch::V0,
        debug_mode: Some(DebugMode {
            filename,
            directory,
        }),
    };
    let assembler = Assembler::new(options);
    let bytecode = assembler
        .assemble(&source_code)
        .map_err(|errors| DebuggerError::Assembler(format!("{:?}", errors)))?;

    load_session_from_bytes(bytecode, input, config, None, program_id)
}

pub fn load_session_from_elf(
    elf_path: &str,
    input: Vec<u8>,
    config: SbpfVmConfig,
    program_id: Address,
) -> DebuggerResult<DebuggerSession> {
    let mut file = File::open(elf_path)?;
    let mut elf_bytes = Vec::new();
    file.read_to_end(&mut elf_bytes)?;
    load_session_from_bytes(elf_bytes, input, config, Some(elf_path.into()), program_id)
}

pub fn load_session_from_bytes(
    elf_bytes: Vec<u8>,
    input: Vec<u8>,
    config: SbpfVmConfig,
    elf_path: Option<PathBuf>,
    program_id: Address,
) -> DebuggerResult<DebuggerSession> {
    let program = Program::from_bytes(&elf_bytes)?;
    let entrypoint = program.get_entrypoint_offset().unwrap_or(0);
    let (mut instructions, rodata_section) = program.to_ixs()?;
    let rodata_bytes = rodata_section
        .as_ref()
        .map(|section| section.data.clone())
        .unwrap_or_default();

    // Remap rodata addresses from ELF addresses to VM addresses
    if let Some(ref section) = rodata_section {
        let elf_rodata_base = section.base_address;
        let elf_rodata_end = elf_rodata_base + section.data.len() as u64;

        for ix in &mut instructions {
            if ix.opcode == Opcode::Lddw
                && let Some(Either::Right(Number::Int(imm))) = &ix.imm
            {
                let addr = *imm as u64;
                if addr >= elf_rodata_base && addr < elf_rodata_end {
                    let offset = addr - elf_rodata_base;
                    let vm_addr = Memory::RODATA_START + offset;
                    ix.imm = Some(Either::Right(Number::Int(vm_addr as i64)));
                }
            }
        }
    }

    let mut vm = DebuggerSession::build_vm(instructions, input, rodata_bytes, config, program_id);
    vm.set_entrypoint(entrypoint as usize);

    let mut debugger = Debugger::new(vm);
    if let Ok(line_map) = LineMap::from_elf_data(&elf_bytes) {
        debugger.set_dwarf_line_map(line_map.clone());
    }

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

    Ok(DebuggerSession {
        line_map: debugger.dwarf_line_map.clone(),
        debugger,
        elf_bytes,
        elf_path: elf_path.unwrap_or_else(|| PathBuf::from("<memory>")),
    })
}
