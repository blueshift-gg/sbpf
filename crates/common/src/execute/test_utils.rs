use {
    super::*,
    crate::{
        inst_param::{Number, Register},
        opcode::Opcode,
    },
    either::Either,
};

/// Mock implementation of Vm for testing
pub struct MockVm {
    pub registers: [u64; 11],
    pub pc: usize,
    memory: Vec<u8>,
    pub call_stack: Vec<(usize, [u64; 4], u64)>,
    pub halted: bool,
    pub exit_code: Option<u64>,
    pub syscall_logs: Vec<String>,
    pub call_depth_limit: usize,
    stack_frame_size: u64,
}

impl Default for MockVm {
    fn default() -> Self {
        Self::new()
    }
}

impl MockVm {
    pub fn new() -> Self {
        Self {
            registers: [0; 11],
            pc: 0,
            memory: vec![0; 0x10000],
            call_stack: Vec::new(),
            halted: false,
            exit_code: None,
            syscall_logs: Vec::new(),
            call_depth_limit: 64,
            stack_frame_size: 4096,
        }
    }

    pub fn write_memory(&mut self, addr: u64, data: &[u8]) {
        let start = addr as usize;
        let end = start + data.len();
        if end <= self.memory.len() {
            self.memory[start..end].copy_from_slice(data);
        }
    }

    pub fn read_memory(&self, addr: u64, len: usize) -> Vec<u8> {
        let start = addr as usize;
        let end = start + len;
        if end <= self.memory.len() {
            self.memory[start..end].to_vec()
        } else {
            vec![]
        }
    }
}

impl super::Vm for MockVm {
    fn get_register(&self, reg: usize) -> u64 {
        self.registers[reg]
    }

    fn set_register(&mut self, reg: usize, value: u64) {
        self.registers[reg] = value;
    }

    fn get_pc(&self) -> usize {
        self.pc
    }

    fn set_pc(&mut self, pc: usize) {
        self.pc = pc;
    }

    fn read_u8(&self, addr: u64) -> ExecutionResult<u8> {
        let idx = addr as usize;
        if idx < self.memory.len() {
            Ok(self.memory[idx])
        } else {
            Err(ExecutionError::InvalidMemoryAccess(addr))
        }
    }

    fn read_u16(&self, addr: u64) -> ExecutionResult<u16> {
        let idx = addr as usize;
        if idx + 2 <= self.memory.len() {
            Ok(u16::from_le_bytes([self.memory[idx], self.memory[idx + 1]]))
        } else {
            Err(ExecutionError::InvalidMemoryAccess(addr))
        }
    }

    fn read_u32(&self, addr: u64) -> ExecutionResult<u32> {
        let idx = addr as usize;
        if idx + 4 <= self.memory.len() {
            Ok(u32::from_le_bytes([
                self.memory[idx],
                self.memory[idx + 1],
                self.memory[idx + 2],
                self.memory[idx + 3],
            ]))
        } else {
            Err(ExecutionError::InvalidMemoryAccess(addr))
        }
    }

    fn read_u64(&self, addr: u64) -> ExecutionResult<u64> {
        let idx = addr as usize;
        if idx + 8 <= self.memory.len() {
            Ok(u64::from_le_bytes([
                self.memory[idx],
                self.memory[idx + 1],
                self.memory[idx + 2],
                self.memory[idx + 3],
                self.memory[idx + 4],
                self.memory[idx + 5],
                self.memory[idx + 6],
                self.memory[idx + 7],
            ]))
        } else {
            Err(ExecutionError::InvalidMemoryAccess(addr))
        }
    }

    fn write_u8(&mut self, addr: u64, value: u8) -> ExecutionResult<()> {
        let idx = addr as usize;
        if idx < self.memory.len() {
            self.memory[idx] = value;
            Ok(())
        } else {
            Err(ExecutionError::InvalidMemoryAccess(addr))
        }
    }

    fn write_u16(&mut self, addr: u64, value: u16) -> ExecutionResult<()> {
        let idx = addr as usize;
        if idx + 2 <= self.memory.len() {
            let bytes = value.to_le_bytes();
            self.memory[idx] = bytes[0];
            self.memory[idx + 1] = bytes[1];
            Ok(())
        } else {
            Err(ExecutionError::InvalidMemoryAccess(addr))
        }
    }

    fn write_u32(&mut self, addr: u64, value: u32) -> ExecutionResult<()> {
        let idx = addr as usize;
        if idx + 4 <= self.memory.len() {
            let bytes = value.to_le_bytes();
            self.memory[idx..idx + 4].copy_from_slice(&bytes);
            Ok(())
        } else {
            Err(ExecutionError::InvalidMemoryAccess(addr))
        }
    }

    fn write_u64(&mut self, addr: u64, value: u64) -> ExecutionResult<()> {
        let idx = addr as usize;
        if idx + 8 <= self.memory.len() {
            let bytes = value.to_le_bytes();
            self.memory[idx..idx + 8].copy_from_slice(&bytes);
            Ok(())
        } else {
            Err(ExecutionError::InvalidMemoryAccess(addr))
        }
    }

    fn get_call_depth(&self) -> usize {
        self.call_stack.len()
    }

    fn max_call_depth(&self) -> usize {
        self.call_depth_limit
    }

    fn push_frame(
        &mut self,
        return_pc: usize,
        saved_registers: [u64; 4],
        saved_frame_pointer: u64,
    ) -> ExecutionResult<()> {
        self.call_stack
            .push((return_pc, saved_registers, saved_frame_pointer));
        Ok(())
    }

    fn pop_frame(&mut self) -> Option<(usize, [u64; 4], u64)> {
        self.call_stack.pop()
    }

    fn halt(&mut self, exit_code: u64) {
        self.halted = true;
        self.exit_code = Some(exit_code);
    }

    fn get_stack_frame_size(&self) -> u64 {
        self.stack_frame_size
    }

    fn handle_syscall(&mut self, name: &str) -> ExecutionResult<u64> {
        self.syscall_logs.push(name.to_string());
        Ok(0)
    }
}

pub fn make_test_instruction(
    opcode: Opcode,
    dst: Option<Register>,
    src: Option<Register>,
    off: Option<Either<String, i16>>,
    imm: Option<Either<String, Number>>,
) -> crate::instruction::Instruction {
    crate::instruction::Instruction {
        opcode,
        dst,
        src,
        off,
        imm,
        span: 0..0,
    }
}
