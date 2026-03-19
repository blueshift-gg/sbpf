use {
    crate::{
        debugger::{DebugEvent, DebugMode},
        runner::DebuggerSession,
    },
    std::io::{self, Write},
};

pub struct Repl {
    pub session: DebuggerSession,
}

impl Repl {
    pub fn new(session: DebuggerSession) -> Self {
        Self { session }
    }

    pub fn start(&mut self) {
        println!("\nsBPF Debugger REPL. Type 'help' for commands.");

        for log in self.session.debugger.runtime.drain_logs() {
            println!("{}", log);
        }

        // Print the first instruction.
        if let Some(line) = self.session.debugger.get_current_line() {
            let asm = self
                .session
                .debugger
                .get_instruction_asm()
                .unwrap_or_default();
            println!("{}\t{}", line, asm);
        }

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
                "next" | "n" => self.run_and_display(DebugMode::Next),
                "finish" | "f" => self.run_and_display(DebugMode::Finish),
                "continue" | "c" => self.run_and_display(DebugMode::Continue),
                cmd if cmd.starts_with("break ") || cmd.starts_with("b ") => {
                    if let Some(arg) = cmd.split_whitespace().nth(1) {
                        if let Ok(line) = arg.parse::<usize>() {
                            match self.session.debugger.set_breakpoint_at_line(line) {
                                Ok(()) => println!("Breakpoint set at line {}", line),
                                Err(e) => println!("Error: {}", e),
                            }
                        } else {
                            println!("Error: Invalid line number.");
                        }
                    }
                }
                cmd if cmd.starts_with("delete ") || cmd.starts_with("d ") => {
                    if let Some(arg) = cmd.split_whitespace().nth(1) {
                        if let Ok(line) = arg.parse::<usize>() {
                            match self.session.debugger.remove_breakpoint_at_line(line) {
                                Ok(()) => println!("Breakpoint removed from line {}", line),
                                Err(e) => println!("Error: {}", e),
                            }
                        } else {
                            println!("Error: Invalid line number for delete command.");
                        }
                    }
                }
                "info breakpoints" | "info b" => {
                    println!("{}", self.session.debugger.get_breakpoints_info());
                }
                "info line" => {
                    if let Some(line) = self.session.debugger.get_current_line() {
                        let asm = self
                            .session
                            .debugger
                            .get_instruction_asm()
                            .unwrap_or_default();
                        println!("{}\t{}", line, asm);
                    }
                }
                "quit" | "q" => break,
                "regs" => {
                    let regs = self.session.debugger.get_registers();
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
                            if let Some(val) = self.session.debugger.get_register(idx) {
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
                                Ok(val) => match self.session.debugger.set_register_value(idx, val)
                                {
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
                "compute" => {
                    let cu_used = self.session.debugger.get_compute_units();
                    let cu_total = self.session.debugger.initial_compute_budget;
                    println!("Program consumed {} of {} compute units", cu_used, cu_total);
                }
                "help" => {
                    println!("Commands:");
                    println!("  step (s)                     - Step into");
                    println!("  next (n)                     - Step over");
                    println!("  finish (f)                   - Step out");
                    println!("  continue (c)                 - Continue execution");
                    println!("  break (b) <line>             - Set breakpoint at line number");
                    println!("  delete (d) <line>            - Remove breakpoint at line");
                    println!("  info breakpoints (info b)    - Show all breakpoints");
                    println!("  info line                    - Show current line info");
                    println!("  regs                         - Show all registers");
                    println!("  reg <idx>                    - Show single register");
                    println!("  setreg <idx> <value>         - Set register value");
                    println!("  compute                      - Show compute unit information");
                    println!("  help                         - Show this help");
                    println!("  quit (q)                     - Exit debugger");
                }
                _ => println!("Unknown command. Type 'help'."),
            }
        }
    }

    fn run_and_display(&mut self, mode: DebugMode) {
        self.session.debugger.set_debug_mode(mode);
        match self.session.debugger.run() {
            Ok(event) => match event {
                DebugEvent::Stopped(_pc, line) => {
                    if let Some(line_num) = line {
                        let asm = self
                            .session
                            .debugger
                            .get_instruction_asm()
                            .unwrap_or_default();
                        println!("{}\t{}", line_num, asm);
                    }
                }
                DebugEvent::Breakpoint(_pc, line) => {
                    if let Some(line_num) = line {
                        let asm = self
                            .session
                            .debugger
                            .get_instruction_asm()
                            .unwrap_or_default();
                        println!("Breakpoint hit at line {}", line_num);
                        println!("{}\t{}", line_num, asm);
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
