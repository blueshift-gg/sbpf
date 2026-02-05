use {
    serde::{Deserialize, Serialize},
    serde_json::{Value, json},
    std::io::{self, BufRead, Write},
};

pub trait DebuggerInterface {
    fn step(&mut self) -> Value;
    fn r#continue(&mut self) -> Value;
    fn set_breakpoint(&mut self, file: String, line: usize) -> Value;
    fn remove_breakpoint(&mut self, file: String, line: usize) -> Value;
    fn get_stack_frames(&self) -> Value;
    fn get_registers(&self) -> Value;
    fn get_memory(&self, address: u64, size: usize) -> Value;
    fn set_register(&mut self, index: usize, value: u64) -> Value;
    fn get_rodata(&self) -> Value;
    fn clear_breakpoints(&mut self, file: String) -> Value;
    fn quit(&mut self) -> Value;
    fn get_compute_units(&self) -> Value;
    fn run_to_json(&mut self) -> Value;
}

#[derive(Deserialize)]
struct AdapterCommand {
    command: String,
    args: Option<Value>,
    #[serde(rename = "requestId")]
    request_id: Option<Value>,
}

#[derive(Serialize)]
struct AdapterResponse {
    success: bool,
    data: Option<Value>,
    error: Option<String>,
    #[serde(rename = "requestId")]
    request_id: Option<Value>,
}

pub fn run_adapter_loop<T: DebuggerInterface>(debugger: &mut T) {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let cmd: Result<AdapterCommand, _> = serde_json::from_str(&line);
        let mut response = AdapterResponse {
            success: true,
            data: None,
            error: None,
            request_id: None,
        };
        match cmd {
            Ok(cmd) => {
                response.request_id = cmd.request_id.clone();
                let result = match cmd.command.as_str() {
                    "step" => debugger.step(),
                    "continue" => debugger.r#continue(),
                    "setBreakpoint" => {
                        if let Some(args) = cmd.args {
                            let file = args
                                .get(0)
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string();
                            let line = args.get(1).and_then(Value::as_u64).unwrap_or(0) as usize;
                            debugger.set_breakpoint(file, line)
                        } else {
                            json!({"type": "error", "message": "Missing args"})
                        }
                    }
                    "removeBreakpoint" => {
                        if let Some(args) = cmd.args {
                            let file = args
                                .get(0)
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string();
                            let line = args.get(1).and_then(Value::as_u64).unwrap_or(0) as usize;
                            debugger.remove_breakpoint(file, line)
                        } else {
                            json!({"type": "error", "message": "Missing args"})
                        }
                    }
                    "getStackFrames" => debugger.get_stack_frames(),
                    "getRegisters" => debugger.get_registers(),
                    "getRodata" => debugger.get_rodata(),
                    "clearBreakpoints" => {
                        if let Some(args) = cmd.args {
                            let file = args
                                .get(0)
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string();
                            debugger.clear_breakpoints(file)
                        } else {
                            json!({"type": "error", "message": "Missing args"})
                        }
                    }
                    "getMemory" => {
                        if let Some(args) = cmd.args {
                            let address = args.get(0).and_then(Value::as_u64).unwrap_or(0);
                            let size = args.get(1).and_then(Value::as_u64).unwrap_or(0) as usize;
                            debugger.get_memory(address, size)
                        } else {
                            json!({"type": "error", "message": "Missing args"})
                        }
                    }
                    "getComputeUnits" => debugger.get_compute_units(),
                    "setRegister" => {
                        if let Some(args) = cmd.args {
                            let index = args.get(0).and_then(Value::as_u64).unwrap_or(0) as usize;
                            let value = args.get(1).and_then(Value::as_u64).unwrap_or(0);
                            debugger.set_register(index, value)
                        } else {
                            json!({"type": "error", "message": "Missing args"})
                        }
                    }
                    "quit" => debugger.quit(),
                    _ => json!({"type": "error", "message": "Unknown command"}),
                };
                // Check if the result contains an error
                if let Some(result_obj) = result.as_object() {
                    if result_obj.contains_key("error") {
                        response.success = false;
                        response.error = result_obj
                            .get("error")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                    } else if result_obj.get("type").and_then(|v| v.as_str()) == Some("error") {
                        response.success = false;
                        response.error = result_obj
                            .get("message")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                    }
                }
                response.data = Some(result);
            }
            Err(e) => {
                response.success = false;
                response.error = Some(format!("Invalid command: {}", e));
            }
        }
        let resp_str = serde_json::to_string(&response).unwrap();
        writeln!(stdout, "{}", resp_str).unwrap();
        stdout.flush().unwrap();
    }
}
