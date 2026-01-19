use crate::{errors::SbpfVmResult, memory::Memory};

/// Trait for handling syscalls
pub trait SyscallHandler {
    fn handle(&mut self, name: &str, registers: [u64; 5], memory: &mut Memory)
    -> SbpfVmResult<u64>;
}

/// Mock syscall handler for testing
#[derive(Debug, Default)]
pub struct MockSyscallHandler {
    pub logs: Vec<String>,
}

impl SyscallHandler for MockSyscallHandler {
    fn handle(
        &mut self,
        name: &str,
        _registers: [u64; 5],
        _memory: &mut Memory,
    ) -> SbpfVmResult<u64> {
        self.logs.push(format!("syscall: {}", name));
        Ok(0)
    }
}
