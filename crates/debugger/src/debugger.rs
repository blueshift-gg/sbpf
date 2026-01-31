use std::collections::HashSet;

use sbpf_common::instruction::Instruction;
use sbpf_vm::{syscalls::SyscallHandler, vm::SbpfVm};

use crate::{
    error::DebuggerResult,
    parser::{LineMap, RODataSymbol},
};

#[derive(Debug)]
pub enum DebugMode {
    Step,
    Continue,
}

#[derive(Debug)]
pub enum DebugEvent {
    Breakpoint(u64, Option<usize>),
    Step(u64, Option<usize>),
    Exit(u64),
    Error(String),
}

pub struct Debugger<H: SyscallHandler> {
    pub vm: SbpfVm<H>,
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
}

impl<H: SyscallHandler> Debugger<H> {
    pub fn new(vm: SbpfVm<H>) -> Self {
        let initial_compute_budget = vm.config.max_steps;
        Self {
            vm,
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
        match self.debug_mode {
            DebugMode::Step => {
                let current_pc = self.get_pc();

                if self.at_breakpoint {
                    match self.vm.step() {
                        Ok(()) => {
                            self.at_breakpoint = false;
                            self.last_breakpoint_pc = None;
                            let new_pc = self.get_pc();
                            if self.breakpoints.contains(&new_pc) {
                                self.at_breakpoint = true;
                                self.last_breakpoint_pc = Some(new_pc);
                                let line_number = self.get_line_for_pc(new_pc);
                                return Ok(DebugEvent::Breakpoint(new_pc, line_number));
                            }
                            let line_number = self.get_line_for_pc(new_pc);
                            return Ok(DebugEvent::Step(new_pc, line_number));
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

                let event = match self.vm.step() {
                    Ok(()) => {
                        let line_number = self.get_line_for_pc(current_pc);
                        DebugEvent::Step(current_pc, line_number)
                    }
                    Err(e) => DebugEvent::Error(format!("{e}")),
                };
                Ok(event)
            }
            DebugMode::Continue => loop {
                let current_pc = self.get_pc();

                if self.at_breakpoint {
                    match self.vm.step() {
                        Ok(()) => {
                            self.at_breakpoint = false;
                            self.last_breakpoint_pc = None;
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

                match self.vm.step() {
                    Ok(()) => {
                        if self.vm.halted {
                            let exit_code = self.vm.exit_code.unwrap_or(0);
                            return Ok(DebugEvent::Exit(exit_code));
                        }
                    }
                    Err(e) => return Ok(DebugEvent::Error(format!("{e}"))),
                }
            },
        }
    }

    pub fn get_pc(&self) -> u64 {
        let mut offset = 0u64;
        for (idx, inst) in self.vm.program.iter().enumerate() {
            if idx == self.vm.pc {
                return offset;
            }
            offset += inst.get_size();
        }
        self.vm.pc as u64
    }

    pub fn get_registers(&self) -> &[u64] {
        &self.vm.registers
    }

    pub fn get_register(&self, idx: usize) -> Option<u64> {
        self.vm.registers.get(idx).copied()
    }

    pub fn set_register_value(&mut self, idx: usize, value: u64) -> Result<(), String> {
        if let Some(reg) = self.vm.registers.get_mut(idx) {
            *reg = value;
            Ok(())
        } else {
            Err(format!("Register index {} out of range", idx))
        }
    }

    pub fn get_rodata(&self) -> Option<&Vec<RODataSymbol>> {
        self.rodata.as_ref()
    }

    pub fn get_compute_units(&self) -> u64 {
        self.vm.compute_units_consumed
    }

    pub fn get_instruction(&self) -> Option<&Instruction> {
        self.vm.program.get(self.vm.pc)
    }

    pub fn get_source_location(&self, pc: u64) -> Option<(&str, usize, usize)> {
        if let Some(dwarf_map) = &self.dwarf_line_map {
            if let Some(loc) = dwarf_map.get_source_location(pc) {
                return Some((&loc.file, loc.line as usize, loc.column as usize));
            }
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
        self.vm
            .memory
            .read_bytes(address, size)
            .map(|slice| slice.to_vec())
            .ok()
    }

    pub fn get_stack_frames(&self) -> Vec<(usize, u64, Option<(&str, usize, usize)>)> {
        let mut frames = Vec::new();

        // Current frame
        let current_pc = self.get_pc();
        frames.push((0, current_pc, self.get_source_location(current_pc)));

        // Call stack frames
        for (i, frame) in self.vm.call_stack.iter().rev().enumerate() {
            let pc = frame.return_pc as u64;
            frames.push((i + 1, pc, self.get_source_location(pc)));
        }

        frames
    }
}
