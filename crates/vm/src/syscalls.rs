use crate::{errors::VmResult, memory::Memory};

pub struct SyscallContext<'a> {
    pub name: &'a str,
    pub registers: [u64; 5], // r1-r5
    pub memory: &'a mut Memory,
}

/// Trait for handling syscalls
pub trait SyscallHandler {
    fn handle(&mut self, ctx: SyscallContext<'_>) -> VmResult<u64>;
}

/// Mock syscall handler for testing
#[derive(Debug, Default)]
pub struct MockSyscallHandler {
    pub logs: Vec<String>,
}

impl SyscallHandler for MockSyscallHandler {
    fn handle(&mut self, ctx: SyscallContext<'_>) -> VmResult<u64> {
        self.logs.push(format!("syscall: {}", ctx.name));
        Ok(0)
    }
}
