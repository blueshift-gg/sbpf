use {
    crate::{
        adapter::DebuggerInterface,
        error::DebuggerResult,
        parser::{LineMap, RODataSymbol},
    },
    either::Either,
    sbpf_common::{inst_param::Number, instruction::Instruction, opcode::Opcode},
    sbpf_runtime::Runtime,
    serde_json::{Value, json},
    std::collections::HashSet,
};

pub struct StackFrame<'a> {
    pub index: usize,
    pub pc: u64,
    pub file: Option<&'a str>,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

#[derive(Debug)]
pub enum DebugMode {
    Next,
    Continue,
}

#[derive(Debug)]
pub enum DebugEvent {
    Breakpoint(u64, Option<usize>),
    Next(u64, Option<usize>),
    Exit(u64),
    Error(String),
}

pub struct Debugger {
    pub runtime: Runtime,
    pub breakpoints: HashSet<u64>,
    pub line_breakpoints: HashSet<usize>,
    pub dwarf_line_map: Option<LineMap>,
    pub rodata: Option<Vec<RODataSymbol>>,
    pub last_breakpoint: Option<u64>,
    pub debug_mode: DebugMode,
    pub stopped: bool,
    pub exit_code: u64,
    pub at_breakpoint: bool,
    pub last_breakpoint_pc: Option<u64>,
    pub initial_compute_budget: u64,
    pub instruction_offsets: Vec<u64>,
}

impl Debugger {
    pub fn new(runtime: Runtime) -> Self {
        let initial_compute_budget = runtime.config().compute_budget;

        let instruction_offsets: Vec<u64> = runtime
            .get_program()
            .iter()
            .scan(0u64, |offset, inst| {
                let current = *offset;
                *offset += inst.get_size();
                Some(current)
            })
            .collect();

        Self {
            runtime,
            breakpoints: HashSet::new(),
            line_breakpoints: HashSet::new(),
            dwarf_line_map: None,
            rodata: None,
            last_breakpoint: None,
            debug_mode: DebugMode::Continue,
            stopped: false,
            exit_code: 0,
            at_breakpoint: false,
            last_breakpoint_pc: None,
            initial_compute_budget,
            instruction_offsets,
        }
    }

    pub fn set_dwarf_line_map(&mut self, dwarf_map: LineMap) {
        self.dwarf_line_map = Some(dwarf_map);
    }

    pub fn set_rodata(&mut self, rodata: Vec<RODataSymbol>) {
        self.rodata = Some(rodata);
    }

    pub fn set_breakpoint(&mut self, pc: u64) {
        self.breakpoints.insert(pc);
    }

    pub fn set_breakpoint_at_line(&mut self, line: usize) -> Result<(), String> {
        if let Some(dwarf_map) = &self.dwarf_line_map {
            let pcs = dwarf_map.get_pcs_for_line(line);
            if pcs.is_empty() {
                return Err(format!("No code at line {}", line));
            }
            self.line_breakpoints.insert(line);
            for &pc in &pcs {
                self.breakpoints.insert(pc);
            }
            Ok(())
        } else {
            Err("No debug info available".to_string())
        }
    }

    pub fn remove_breakpoint_at_line(&mut self, line: usize) -> Result<(), String> {
        if let Some(dwarf_map) = &self.dwarf_line_map {
            let pcs = dwarf_map.get_pcs_for_line(line);
            if !pcs.is_empty() {
                self.line_breakpoints.remove(&line);
                for &pc in &pcs {
                    self.breakpoints.remove(&pc);
                }
            }
        }
        Ok(())
    }

    pub fn get_current_line(&self) -> Option<usize> {
        let pc = self.get_pc();
        self.get_line_for_pc(pc)
    }

    pub fn get_line_for_pc(&self, pc: u64) -> Option<usize> {
        if let Some(dwarf_map) = &self.dwarf_line_map {
            dwarf_map.get_line_for_pc(pc)
        } else {
            None
        }
    }

    pub fn get_pcs_for_line(&self, line: usize) -> Vec<u64> {
        if let Some(dwarf_map) = &self.dwarf_line_map {
            dwarf_map.get_pcs_for_line(line)
        } else {
            Vec::new()
        }
    }

    pub fn get_breakpoints_info(&self) -> String {
        if self.line_breakpoints.is_empty() {
            return "No breakpoints set".to_string();
        }
        let mut lines: Vec<_> = self.line_breakpoints.iter().copied().collect();
        lines.sort();
        let lines_str = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        format!("Breakpoints: {}", lines_str)
    }

    pub fn set_debug_mode(&mut self, debug_mode: DebugMode) {
        self.debug_mode = debug_mode;
    }

    pub fn run(&mut self) -> DebuggerResult<DebugEvent> {
        let event = self.execute()?;

        // Print collected logs.
        for log in self.runtime.drain_logs() {
            println!("{}", log);
        }

        Ok(event)
    }

    fn execute(&mut self) -> DebuggerResult<DebugEvent> {
        match self.debug_mode {
            DebugMode::Next => {
                let current_pc = self.get_pc();

                if self.at_breakpoint {
                    match self.runtime.step() {
                        Ok(()) => {
                            self.at_breakpoint = false;
                            self.last_breakpoint_pc = None;

                            if self.runtime.is_halted() {
                                let exit_code = self.runtime.exit_code().unwrap_or(0);
                                return Ok(DebugEvent::Exit(exit_code));
                            }

                            let new_pc = self.get_pc();
                            if self.breakpoints.contains(&new_pc) {
                                self.at_breakpoint = true;
                                self.last_breakpoint_pc = Some(new_pc);
                                let line_number = self.get_line_for_pc(new_pc);
                                return Ok(DebugEvent::Breakpoint(new_pc, line_number));
                            }
                            let line_number = self.get_line_for_pc(new_pc);
                            return Ok(DebugEvent::Next(new_pc, line_number));
                        }
                        Err(e) => return Ok(DebugEvent::Error(format!("{e}"))),
                    }
                }

                if self.breakpoints.contains(&current_pc)
                    && self.last_breakpoint_pc != Some(current_pc)
                {
                    self.at_breakpoint = true;
                    self.last_breakpoint_pc = Some(current_pc);
                    let line_number = self.get_line_for_pc(current_pc);
                    return Ok(DebugEvent::Breakpoint(current_pc, line_number));
                }

                let event = match self.runtime.step() {
                    Ok(()) => {
                        if self.runtime.is_halted() {
                            let exit_code = self.runtime.exit_code().unwrap_or(0);
                            DebugEvent::Exit(exit_code)
                        } else {
                            let new_pc = self.get_pc();
                            let line_number = self.get_line_for_pc(new_pc);
                            DebugEvent::Next(new_pc, line_number)
                        }
                    }
                    Err(e) => DebugEvent::Error(format!("{e}")),
                };
                Ok(event)
            }
            DebugMode::Continue => loop {
                let current_pc = self.get_pc();

                if self.at_breakpoint {
                    match self.runtime.step() {
                        Ok(()) => {
                            self.at_breakpoint = false;
                            self.last_breakpoint_pc = None;

                            if self.runtime.is_halted() {
                                let exit_code = self.runtime.exit_code().unwrap_or(0);
                                return Ok(DebugEvent::Exit(exit_code));
                            }
                        }
                        Err(e) => return Ok(DebugEvent::Error(format!("{e}"))),
                    }
                    continue;
                }

                if self.breakpoints.contains(&current_pc)
                    && self.last_breakpoint_pc != Some(current_pc)
                {
                    self.at_breakpoint = true;
                    self.last_breakpoint_pc = Some(current_pc);
                    let line_number = self.get_line_for_pc(current_pc);
                    return Ok(DebugEvent::Breakpoint(current_pc, line_number));
                }

                match self.runtime.step() {
                    Ok(()) => {
                        if self.runtime.is_halted() {
                            let exit_code = self.runtime.exit_code().unwrap_or(0);
                            return Ok(DebugEvent::Exit(exit_code));
                        }
                    }
                    Err(e) => return Ok(DebugEvent::Error(format!("{e}"))),
                }
            },
        }
    }

    pub fn get_pc(&self) -> u64 {
        let idx = self.runtime.get_pc();
        self.instruction_offsets
            .get(idx)
            .copied()
            .unwrap_or(idx as u64)
    }

    fn instruction_index_to_byte_offset(&self, idx: usize) -> u64 {
        self.instruction_offsets
            .get(idx)
            .copied()
            .unwrap_or(idx as u64)
    }

    pub fn get_registers(&self) -> &[u64] {
        self.runtime
            .get_registers()
            .map(|r| r.as_slice())
            .unwrap_or(&[])
    }

    pub fn get_register(&self, idx: usize) -> Option<u64> {
        self.runtime.get_register(idx)
    }

    pub fn set_register_value(&mut self, idx: usize, value: u64) -> Result<(), String> {
        self.runtime
            .set_register(idx, value)
            .map_err(|e| e.to_string())
    }

    pub fn get_rodata(&self) -> Option<&Vec<RODataSymbol>> {
        self.rodata.as_ref()
    }

    pub fn get_compute_units(&self) -> u64 {
        self.runtime.compute_units_consumed()
    }

    pub fn get_instruction(&self) -> Option<&Instruction> {
        self.runtime.get_instruction()
    }

    pub fn get_instruction_asm(&self) -> Option<String> {
        let inst = self.runtime.get_instruction()?;
        let mut asm = inst.to_asm().ok()?;
        // Resolve rodata label.
        if inst.opcode == Opcode::Lddw
            && let Some(Either::Right(Number::Int(imm))) = &inst.imm
            && let Some(ref rodata_symbols) = self.rodata
        {
            let addr = *imm as u64;
            for sym in rodata_symbols {
                if sym.address == addr {
                    asm = asm.replace(&imm.to_string(), &sym.name);
                    break;
                }
            }
        }

        Some(asm)
    }

    pub fn get_source_location(&self, pc: u64) -> Option<(&str, usize, usize)> {
        if let Some(dwarf_map) = &self.dwarf_line_map
            && let Some(loc) = dwarf_map.get_source_location(pc)
        {
            return Some((&loc.file, loc.line as usize, loc.column as usize));
        }
        None
    }

    pub fn clear_breakpoints(&mut self) {
        if let Some(dwarf_map) = &self.dwarf_line_map {
            let lines: Vec<usize> = self.line_breakpoints.iter().copied().collect();
            for line in lines {
                let pcs = dwarf_map.get_pcs_for_line(line);
                for pc in pcs {
                    self.breakpoints.remove(&pc);
                }
                self.line_breakpoints.remove(&line);
            }
        } else {
            self.breakpoints.clear();
            self.line_breakpoints.clear();
        }
    }

    pub fn get_memory(&self, address: u64, size: usize) -> Option<Vec<u8>> {
        self.runtime.read_memory(address, size)
    }

    fn make_stack_frame(&self, index: usize, pc: u64) -> StackFrame<'_> {
        let loc = self.get_source_location(pc);
        StackFrame {
            index,
            pc,
            file: loc.map(|(f, _, _)| f),
            line: loc.map(|(_, l, _)| l),
            column: loc.map(|(_, _, c)| c),
        }
    }

    pub fn get_stack_frames(&self) -> Vec<StackFrame<'_>> {
        let mut frames = Vec::new();
        // Current frame
        let current_pc = self.get_pc();
        frames.push(self.make_stack_frame(0, current_pc));

        // Call stack frames
        if let Some(call_stack) = self.runtime.get_call_stack() {
            for (i, frame) in call_stack.iter().rev().enumerate() {
                let pc = self.instruction_index_to_byte_offset(frame.return_pc);
                frames.push(self.make_stack_frame(i + 1, pc));
            }
        }

        frames
    }
}

impl DebuggerInterface for Debugger {
    fn next(&mut self) -> Value {
        self.set_debug_mode(DebugMode::Next);
        self.run_to_json()
    }

    fn r#continue(&mut self) -> Value {
        self.set_debug_mode(DebugMode::Continue);
        self.run_to_json()
    }

    fn set_breakpoint(&mut self, file: String, line: usize) -> Value {
        match self.set_breakpoint_at_line(line) {
            Ok(()) => json!({
                "type": "setBreakpoint",
                "file": file,
                "line": line,
                "verified": true
            }),
            Err(e) => json!({
                "type": "setBreakpoint",
                "file": file,
                "line": line,
                "verified": false,
                "error": e
            }),
        }
    }

    fn remove_breakpoint(&mut self, file: String, line: usize) -> Value {
        match self.remove_breakpoint_at_line(line) {
            Ok(()) => json!({
                "type": "removeBreakpoint",
                "file": file,
                "line": line,
                "success": true
            }),
            Err(e) => json!({
                "type": "removeBreakpoint",
                "file": file,
                "line": line,
                "success": false,
                "error": e
            }),
        }
    }

    fn get_stack_frames(&self) -> Value {
        let frames: Vec<Value> = self
            .get_stack_frames()
            .iter()
            .map(|frame| {
                let name = frame.file.unwrap_or("?").to_string();
                let file = frame.file.unwrap_or("?").to_string();
                let line = frame.line.unwrap_or(0);
                let column = frame.column.unwrap_or(0);
                json!({
                    "index": frame.index,
                    "name": name,
                    "file": file,
                    "line": line,
                    "column": column,
                    "instruction": frame.pc
                })
            })
            .collect();
        json!({ "frames": frames })
    }

    fn get_registers(&self) -> Value {
        let regs: Vec<Value> = self
            .get_registers()
            .iter()
            .enumerate()
            .map(|(i, &value)| {
                json!({
                    "name": format!("r{}", i),
                    "value": format!("0x{:016x}", value),
                    "type": "u64"
                })
            })
            .collect();
        json!({ "registers": regs })
    }

    fn get_memory(&self, address: u64, size: usize) -> Value {
        match self.get_memory(address, size) {
            Some(data) => json!({
                "address": address,
                "size": size,
                "data": data
            }),
            None => json!({
                "address": address,
                "size": size,
                "data": []
            }),
        }
    }

    fn set_register(&mut self, index: usize, value: u64) -> Value {
        match self.set_register_value(index, value) {
            Ok(()) => json!({
                "type": "setRegister",
                "index": index,
                "value": value,
                "success": true
            }),
            Err(e) => json!({
                "type": "setRegister",
                "index": index,
                "value": value,
                "success": false,
                "error": e
            }),
        }
    }

    fn get_rodata(&self) -> Value {
        match self.get_rodata() {
            Some(symbols) => {
                let arr: Vec<Value> = symbols
                    .iter()
                    .map(|sym| {
                        json!({
                            "name": sym.name,
                            "address": format!("0x{:016x}", sym.address),
                            "value": sym.content
                        })
                    })
                    .collect();
                json!({ "rodata": arr })
            }
            None => json!({ "rodata": [] }),
        }
    }

    fn clear_breakpoints(&mut self, _file: String) -> Value {
        self.clear_breakpoints();
        json!({"result": "ok"})
    }

    fn quit(&mut self) -> Value {
        json!({ "type": "quit" })
    }

    fn get_compute_units(&self) -> Value {
        let used = self.get_compute_units();
        let total = self.initial_compute_budget;
        let remaining = total.saturating_sub(used);
        json!({
            "total": total,
            "used": used,
            "remaining": remaining
        })
    }

    fn run_to_json(&mut self) -> Value {
        match self.run() {
            Ok(event) => match event {
                DebugEvent::Next(pc, line) => json!({
                    "type": "next",
                    "pc": pc,
                    "line": line
                }),
                DebugEvent::Breakpoint(pc, line) => json!({
                    "type": "breakpoint",
                    "pc": pc,
                    "line": line
                }),
                DebugEvent::Exit(code) => {
                    let cu = DebuggerInterface::get_compute_units(self);
                    json!({
                        "type": "exit",
                        "code": code,
                        "compute_units": cu
                    })
                }
                DebugEvent::Error(msg) => json!({
                    "type": "error",
                    "message": msg
                }),
            },
            Err(e) => json!({
                "type": "error",
                "message": format!("{:?}", e)
            }),
        }
    }
}
