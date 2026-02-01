use std::io::{self, Write};

use sbpf_vm::syscalls::SyscallHandler;

use crate::debugger::{DebugEvent, DebugMode, Debugger};

pub struct Repl<H: SyscallHandler> {
    pub dbg: Debugger<H>,
}

impl<H: SyscallHandler> Repl<H> {
    pub fn new(dbg: Debugger<H>) -> Self {
        Self { dbg }
    }

    pub fn start(&mut self) {
        println!("\nsBPF Debugger REPL. Type 'help' for commands.");

        let stdin = io::stdin();
        loop {
            print!("dbg> ");
            io::stdout().flush().unwrap();
            let mut input = String::new();
            if stdin.read_line(&mut input).is_err() {
                break;
            }
            let cmd = input.trim();
            match cmd {
                "step" | "s" => self.run_and_display(DebugMode::Step),
                "continue" | "c" => self.run_and_display(DebugMode::Continue),
                cmd if cmd.starts_with("break ") => {
                    if let Some(arg) = cmd.split_whitespace().nth(1) {
                        if let Ok(line) = arg.parse::<usize>() {
                            match self.dbg.set_breakpoint_at_line(line) {
                                Ok(()) => println!("Breakpoint set at line {}", line),
                                Err(e) => println!("Error: {}", e),
                            }
                        } else {
                            println!("Error: Invalid line number.");
                        }
                    }
                }
                cmd if cmd.starts_with("delete ") => {
                    if let Some(arg) = cmd.split_whitespace().nth(1) {
                        if let Ok(line) = arg.parse::<usize>() {
                            match self.dbg.remove_breakpoint_at_line(line) {
                                Ok(()) => println!("Breakpoint removed from line: {}", line),
                                Err(e) => println!("Error: {}", e),
                            }
                        } else {
                            println!("Error: Invalid line number for delete command.");
                        }
                    }
                }
                "info breakpoints" | "info b" => {
                    println!("{}", self.dbg.get_breakpoints_info());
                }
                "info line" => {
                    if let Some(line) = self.dbg.get_current_line() {
                        println!("Current line: {}", line);
                    } else {
                        println!("No line information available for current PC");
                    }
                }
                "quit" => break,
                "regs" => {
                    let regs = self.dbg.get_registers();
                    println!("+------------+--------------------+");
                    println!("| Register   | Value              |");
                    println!("+------------+--------------------+");
                    for (i, val) in regs.iter().enumerate() {
                        println!(
                            "| {:<10} | {:<18} |",
                            format!("r{}", i),
                            format!("0x{:016x}", val)
                        );
                    }
                    println!("+------------+--------------------+");
                }
                cmd if cmd.starts_with("reg ") => {
                    if let Some(arg) = cmd.split_whitespace().nth(1) {
                        if let Ok(idx) = arg.parse::<usize>() {
                            if let Some(val) = self.dbg.get_register(idx) {
                                println!("+------------+--------------------+");
                                println!("| Register   | Value              |");
                                println!("+------------+--------------------+");
                                println!(
                                    "| {:<10} | {:<18} |",
                                    format!("r{}", idx),
                                    format!("0x{:016x}", val)
                                );
                                println!("+------------+--------------------+");
                            } else {
                                println!("Register index out of range");
                            }
                        } else {
                            println!("Invalid register index");
                        }
                    } else {
                        println!("Usage: reg <idx>");
                    }
                }
                cmd if cmd.starts_with("setreg ") => {
                    let mut parts = cmd.split_whitespace();
                    parts.next();
                    let idx_str = parts.next();
                    let val_str = parts.next();
                    if let (Some(idx_str), Some(val_str)) = (idx_str, val_str) {
                        if let Ok(idx) = idx_str.parse::<usize>() {
                            let value = if let Some(stripped) = val_str.strip_prefix("0x") {
                                u64::from_str_radix(stripped, 16)
                            } else {
                                val_str.parse::<u64>()
                            };
                            match value {
                                Ok(val) => match self.dbg.set_register_value(idx, val) {
                                    Ok(()) => println!("Set r{} to 0x{:08x} ({})", idx, val, val),
                                    Err(e) => println!("{}", e),
                                },
                                Err(_) => println!(
                                    "Invalid value: must be a number (decimal or 0x... hex)"
                                ),
                            }
                        } else {
                            println!("Invalid register index");
                        }
                    } else {
                        println!("Usage: setreg <idx> <value>");
                    }
                }
                "lines" => {
                    if let Some(ref dwarf_map) = self.dbg.dwarf_line_map {
                        println!("+----------+--------------------------+");
                        println!("| Line     | Instruction Addresses    |");
                        println!("+----------+--------------------------+");
                        let mut lines: Vec<_> = dwarf_map.get_line_to_pcs().into_iter().collect();
                        lines.sort_by_key(|(line, _)| *line);
                        for (line, pcs) in lines {
                            let pcs_str = pcs
                                .iter()
                                .map(|pc| format!("0x{:08x}", pc))
                                .collect::<Vec<_>>()
                                .join(", ");
                            println!("| {:<8} | {:<24} |", line, pcs_str);
                        }
                        println!("+----------+--------------------------+");
                    } else {
                        println!("No DWARF line mapping available.");
                    }
                }
                "compute" => {
                    let cu_used = self.dbg.get_compute_units();
                    let cu_total = self.dbg.initial_compute_budget;
                    println!("Program consumed {} of {} compute units", cu_used, cu_total);
                }
                "help" => {
                    println!("Commands:");
                    println!("  step (s)                     - Execute one instruction");
                    println!("  continue (c)                 - Continue execution");
                    println!("  break <line>                 - Set breakpoint at line number");
                    println!("  delete <line>                - Remove breakpoint at line");
                    println!("  info breakpoints (info b)    - Show all breakpoints");
                    println!("  info line                    - Show current line info");
                    println!("  regs                         - Show all registers");
                    println!("  reg <idx>                    - Show single register");
                    println!("  setreg <idx> <value>         - Set register value");
                    println!("  lines                        - Show line to PC mapping");
                    println!("  compute                      - Show compute unit information");
                    println!("  help                         - Show this help");
                    println!("  quit                         - Exit debugger");
                }
                _ => println!("Unknown command. Type 'help'."),
            }
        }
    }

    fn run_and_display(&mut self, mode: DebugMode) {
        self.dbg.set_debug_mode(mode);
        match self.dbg.run() {
            Ok(event) => match event {
                DebugEvent::Step(pc, line) => {
                    if let Some(line_num) = line {
                        println!("Step at PC 0x{:08x} (line {})", pc, line_num);
                    } else {
                        println!("Step at PC 0x{:08x}", pc);
                    }
                }
                DebugEvent::Breakpoint(pc, line) => {
                    if let Some(line_num) = line {
                        println!("Breakpoint hit at PC 0x{:08x} (line {})", pc, line_num);
                    } else {
                        println!("Breakpoint hit at PC 0x{:08x}", pc);
                    }
                }
                DebugEvent::Exit(code) => {
                    println!("Program exited with code: {}", code);
                }
                DebugEvent::Error(msg) => {
                    println!("Program error: {}", msg);
                }
            },
            Err(e) => println!("Debugger error: {:?}", e),
        }
    }
}
