use {
    crate::{
        errors::{SbpfVmError, SbpfVmResult},
        memory::Memory,
        syscalls::SyscallHandler,
    },
    sbpf_common::{
        errors::ExecutionError,
        execute::{self, Vm},
        instruction::Instruction,
        opcode::Opcode,
    },
    serde::{Deserialize, Serialize},
};

/// VM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SbpfVmConfig {
    pub max_call_depth: usize,
    pub max_steps: u64,
    pub stack_size: usize,
    pub heap_size: usize,
}

impl Default for SbpfVmConfig {
    fn default() -> Self {
        Self {
            max_call_depth: 64,
            max_steps: 1_000_000,
            stack_size: Memory::DEFAULT_STACK_SIZE,
            heap_size: Memory::DEFAULT_HEAP_SIZE,
        }
    }
}

/// Call frame for internal function calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallFrame {
    pub return_pc: usize,
    pub saved_registers: [u64; 4], // callee-saved registers (r6-r9)
    pub saved_frame_pointer: u64,
}

/// sBPF Virtual Machine
pub struct SbpfVm<H: SyscallHandler> {
    pub config: SbpfVmConfig,
    pub registers: [u64; 11],
    pub pc: usize,
    pub call_stack: Vec<CallFrame>,
    pub memory: Memory,
    pub program: Vec<Instruction>,
    pub halted: bool,
    pub exit_code: Option<u64>,
    pub compute_units_consumed: u64,
    pub syscall_handler: H,
}

impl<H: SyscallHandler> SbpfVm<H> {
    pub fn new(
        program: Vec<Instruction>,
        input: Vec<u8>,
        rodata: Vec<u8>,
        syscall_handler: H,
    ) -> Self {
        Self::new_with_config(
            program,
            input,
            rodata,
            syscall_handler,
            SbpfVmConfig::default(),
        )
    }

    pub fn new_with_config(
        program: Vec<Instruction>,
        input: Vec<u8>,
        rodata: Vec<u8>,
        syscall_handler: H,
        config: SbpfVmConfig,
    ) -> Self {
        let memory = Memory::new(input, rodata, config.stack_size, config.heap_size);

        let mut registers = [0u64; 11];
        registers[1] = Memory::INPUT_START;
        registers[10] = memory.initial_frame_pointer();

        Self {
            config,
            registers,
            pc: 0,
            call_stack: Vec::new(),
            memory,
            program,
            halted: false,
            exit_code: None,
            compute_units_consumed: 0,
            syscall_handler,
        }
    }

    pub fn reset(&mut self) {
        self.registers = [0u64; 11];
        self.registers[1] = Memory::INPUT_START;
        self.registers[10] = self.memory.initial_frame_pointer();
        self.pc = 0;
        self.call_stack.clear();
        self.halted = false;
        self.exit_code = None;
        self.compute_units_consumed = 0;
        self.memory.reset_heap();
    }

    pub fn current_instruction(&self) -> SbpfVmResult<&Instruction> {
        self.program
            .get(self.pc)
            .ok_or(SbpfVmError::PcOutOfBounds(self.pc))
    }

    pub fn set_entrypoint(&mut self, pc: usize) {
        self.pc = pc;
    }

    pub fn is_pc_valid(&self) -> bool {
        self.pc < self.program.len()
    }

    pub fn step(&mut self) -> SbpfVmResult<()> {
        if self.halted {
            return Ok(());
        }

        if !self.is_pc_valid() {
            return Err(SbpfVmError::PcOutOfBounds(self.pc));
        }

        let inst = self.current_instruction()?.clone();
        self.execute_instruction(&inst)?;

        self.compute_units_consumed += 1;
        // TODO: Handle dynamic CU costs for syscalls
        Ok(())
    }

    fn execute_instruction(&mut self, inst: &Instruction) -> SbpfVmResult<()> {
        match inst.opcode {
            // ALU 64-bit instructions
            Opcode::Add64Imm
            | Opcode::Sub64Imm
            | Opcode::Mul64Imm
            | Opcode::Div64Imm
            | Opcode::Or64Imm
            | Opcode::And64Imm
            | Opcode::Lsh64Imm
            | Opcode::Rsh64Imm
            | Opcode::Mod64Imm
            | Opcode::Xor64Imm
            | Opcode::Mov64Imm
            | Opcode::Arsh64Imm => execute::execute_binary_immediate(self, inst)?,
            Opcode::Add64Reg
            | Opcode::Sub64Reg
            | Opcode::Mul64Reg
            | Opcode::Div64Reg
            | Opcode::Or64Reg
            | Opcode::And64Reg
            | Opcode::Lsh64Reg
            | Opcode::Rsh64Reg
            | Opcode::Mod64Reg
            | Opcode::Xor64Reg
            | Opcode::Mov64Reg
            | Opcode::Arsh64Reg => execute::execute_binary_register(self, inst)?,

            // ALU 32-bit instructions
            Opcode::Add32Imm
            | Opcode::Sub32Imm
            | Opcode::Mul32Imm
            | Opcode::Div32Imm
            | Opcode::Or32Imm
            | Opcode::And32Imm
            | Opcode::Lsh32Imm
            | Opcode::Rsh32Imm
            | Opcode::Mod32Imm
            | Opcode::Xor32Imm
            | Opcode::Mov32Imm
            | Opcode::Arsh32Imm => execute::execute_binary_immediate(self, inst)?,
            Opcode::Add32Reg
            | Opcode::Sub32Reg
            | Opcode::Mul32Reg
            | Opcode::Div32Reg
            | Opcode::Or32Reg
            | Opcode::And32Reg
            | Opcode::Lsh32Reg
            | Opcode::Rsh32Reg
            | Opcode::Mod32Reg
            | Opcode::Xor32Reg
            | Opcode::Mov32Reg
            | Opcode::Arsh32Reg => execute::execute_binary_register(self, inst)?,

            // Unary and endian instructions
            Opcode::Neg64 | Opcode::Neg32 | Opcode::Le | Opcode::Be => {
                execute::execute_unary(self, inst)?
            }

            // Load instructions
            Opcode::Lddw => execute::execute_load_immediate(self, inst)?,
            Opcode::Ldxb | Opcode::Ldxh | Opcode::Ldxw | Opcode::Ldxdw => {
                execute::execute_load_memory(self, inst)?
            }

            // Store immediate instructions
            Opcode::Stb | Opcode::Sth | Opcode::Stw | Opcode::Stdw => {
                execute::execute_store_immediate(self, inst)?
            }

            // Store register instructions
            Opcode::Stxb | Opcode::Stxh | Opcode::Stxw | Opcode::Stxdw => {
                execute::execute_store_register(self, inst)?
            }

            // Jump instructions
            Opcode::Ja => execute::execute_jump(self, inst)?,
            Opcode::JeqImm
            | Opcode::JgtImm
            | Opcode::JgeImm
            | Opcode::JltImm
            | Opcode::JleImm
            | Opcode::JsetImm
            | Opcode::JneImm
            | Opcode::JsgtImm
            | Opcode::JsgeImm
            | Opcode::JsltImm
            | Opcode::JsleImm => execute::execute_jump_immediate(self, inst)?,
            Opcode::JeqReg
            | Opcode::JgtReg
            | Opcode::JgeReg
            | Opcode::JltReg
            | Opcode::JleReg
            | Opcode::JsetReg
            | Opcode::JneReg
            | Opcode::JsgtReg
            | Opcode::JsgeReg
            | Opcode::JsltReg
            | Opcode::JsleReg => execute::execute_jump_register(self, inst)?,

            // Call instructions
            Opcode::Call => execute::execute_call_immediate(self, inst)?,
            Opcode::Callx => execute::execute_call_register(self, inst)?,

            // Exit instruction
            Opcode::Exit => execute::execute_exit(self, inst)?,

            _ => return Err(SbpfVmError::InvalidInstruction),
        }
        Ok(())
    }

    pub fn run(&mut self) -> SbpfVmResult<()> {
        let mut steps = 0;

        while !self.halted && steps < self.config.max_steps {
            self.step()?;
            steps += 1;
        }

        if !self.halted && steps >= self.config.max_steps {
            return Err(SbpfVmError::ExecutionLimitReached(self.config.max_steps));
        }

        Ok(())
    }
}

impl<H: SyscallHandler> Vm for SbpfVm<H> {
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

    fn read_u8(&self, addr: u64) -> Result<u8, ExecutionError> {
        self.memory
            .read_u8(addr)
            .map_err(|_| ExecutionError::InvalidMemoryAccess(addr))
    }

    fn read_u16(&self, addr: u64) -> Result<u16, ExecutionError> {
        self.memory
            .read_u16(addr)
            .map_err(|_| ExecutionError::InvalidMemoryAccess(addr))
    }

    fn read_u32(&self, addr: u64) -> Result<u32, ExecutionError> {
        self.memory
            .read_u32(addr)
            .map_err(|_| ExecutionError::InvalidMemoryAccess(addr))
    }

    fn read_u64(&self, addr: u64) -> Result<u64, ExecutionError> {
        self.memory
            .read_u64(addr)
            .map_err(|_| ExecutionError::InvalidMemoryAccess(addr))
    }

    fn write_u8(&mut self, addr: u64, value: u8) -> Result<(), ExecutionError> {
        self.memory
            .write_u8(addr, value)
            .map_err(|_| ExecutionError::InvalidMemoryAccess(addr))
    }

    fn write_u16(&mut self, addr: u64, value: u16) -> Result<(), ExecutionError> {
        self.memory
            .write_u16(addr, value)
            .map_err(|_| ExecutionError::InvalidMemoryAccess(addr))
    }

    fn write_u32(&mut self, addr: u64, value: u32) -> Result<(), ExecutionError> {
        self.memory
            .write_u32(addr, value)
            .map_err(|_| ExecutionError::InvalidMemoryAccess(addr))
    }

    fn write_u64(&mut self, addr: u64, value: u64) -> Result<(), ExecutionError> {
        self.memory
            .write_u64(addr, value)
            .map_err(|_| ExecutionError::InvalidMemoryAccess(addr))
    }

    fn get_call_depth(&self) -> usize {
        self.call_stack.len()
    }

    fn max_call_depth(&self) -> usize {
        self.config.max_call_depth
    }

    fn push_frame(
        &mut self,
        return_pc: usize,
        saved_registers: [u64; 4],
        saved_frame_pointer: u64,
    ) -> Result<(), ExecutionError> {
        self.call_stack.push(CallFrame {
            return_pc,
            saved_registers,
            saved_frame_pointer,
        });
        Ok(())
    }

    fn pop_frame(&mut self) -> Option<(usize, [u64; 4], u64)> {
        self.call_stack.pop().map(|frame| {
            (
                frame.return_pc,
                frame.saved_registers,
                frame.saved_frame_pointer,
            )
        })
    }

    fn halt(&mut self, exit_code: u64) {
        self.halted = true;
        self.exit_code = Some(exit_code);
    }

    fn get_stack_frame_size(&self) -> u64 {
        Memory::STACK_FRAME_SIZE
    }

    fn handle_syscall(&mut self, name: &str) -> Result<u64, ExecutionError> {
        let registers = [
            self.registers[1],
            self.registers[2],
            self.registers[3],
            self.registers[4],
            self.registers[5],
        ];
        self.syscall_handler
            .handle(name, registers, &mut self.memory)
            .map_err(|e| ExecutionError::SyscallError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::syscalls::MockSyscallHandler,
        either::Either,
        sbpf_common::{
            inst_param::{Number, Register},
            opcode::Opcode,
        },
    };

    fn make_test_instruction(
        opcode: sbpf_common::opcode::Opcode,
        dst: Option<sbpf_common::inst_param::Register>,
        src: Option<sbpf_common::inst_param::Register>,
        off: Option<Either<String, i16>>,
        imm: Option<Either<String, Number>>,
    ) -> Instruction {
        Instruction {
            opcode,
            dst,
            src,
            off,
            imm,
            span: 0..0,
        }
    }

    #[test]
    fn test_vm_initialization() {
        let program = vec![make_test_instruction(Opcode::Exit, None, None, None, None)];
        let vm = SbpfVm::new(
            program,
            vec![1, 2, 3, 4],
            vec![],
            MockSyscallHandler::default(),
        );

        assert_eq!(vm.pc, 0);
        assert_eq!(vm.registers[1], Memory::INPUT_START);
        assert_eq!(
            vm.registers[10],
            Memory::STACK_START + Memory::DEFAULT_STACK_SIZE as u64
        );
        assert!(!vm.halted);
        assert_eq!(vm.exit_code, None);
    }

    #[test]
    fn test_vm_reset() {
        let program = vec![make_test_instruction(Opcode::Exit, None, None, None, None)];
        let mut vm = SbpfVm::new(
            program,
            vec![1, 2, 3, 4],
            vec![],
            MockSyscallHandler::default(),
        );

        // modify vm
        vm.registers[0] = 11;
        vm.pc = 10;
        vm.halted = true;
        vm.exit_code = Some(1);

        // reset
        vm.reset();

        assert_eq!(vm.pc, 0);
        assert_eq!(vm.registers[0], 0);
        assert_eq!(vm.registers[1], Memory::INPUT_START);
        assert!(!vm.halted);
        assert_eq!(vm.exit_code, None);
    }

    #[test]
    fn test_current_instruction() {
        let program = vec![
            make_test_instruction(
                Opcode::Mov64Imm,
                Some(Register { n: 0 }),
                None,
                None,
                Some(Either::Right(Number::Int(123))),
            ),
            make_test_instruction(Opcode::Exit, None, None, None, None),
        ];
        let vm = SbpfVm::new(program, vec![], vec![], MockSyscallHandler::default());

        let inst = vm.current_instruction().unwrap();
        assert_eq!(inst.opcode, Opcode::Mov64Imm);
    }

    #[test]
    fn test_load_store() {
        // lddw r1, 0x12345
        // mov64 r2, r10
        // sub r2, 8
        // stxdw [r2 + 0], r1
        // ldxdw r3, [r2 + 0]
        let program = vec![
            make_test_instruction(
                Opcode::Lddw,
                Some(Register { n: 1 }),
                None,
                None,
                Some(Either::Right(Number::Int(0x12345u64 as i64))),
            ),
            make_test_instruction(
                Opcode::Mov64Reg,
                Some(Register { n: 2 }),
                Some(Register { n: 10 }),
                None,
                None,
            ),
            make_test_instruction(
                Opcode::Sub64Imm,
                Some(Register { n: 2 }),
                None,
                None,
                Some(Either::Right(Number::Int(8))),
            ),
            make_test_instruction(
                Opcode::Stxdw,
                Some(Register { n: 2 }),
                Some(Register { n: 1 }),
                Some(Either::Right(0)),
                None,
            ),
            make_test_instruction(
                Opcode::Ldxdw,
                Some(Register { n: 3 }),
                Some(Register { n: 2 }),
                Some(Either::Right(0)),
                None,
            ),
        ];

        let mut vm = SbpfVm::new(program, vec![], vec![], MockSyscallHandler::default());

        for _ in 0..5 {
            vm.step().unwrap();
        }

        assert_eq!(vm.registers[3], 0x12345);
    }

    #[test]
    fn test_alu64_operations() {
        // mov64 r1, 10
        // add64 r1, 5
        // mul r1, 2
        let program = vec![
            make_test_instruction(
                Opcode::Mov64Imm,
                Some(Register { n: 1 }),
                None,
                None,
                Some(Either::Right(Number::Int(10))),
            ),
            make_test_instruction(
                Opcode::Add64Imm,
                Some(Register { n: 1 }),
                None,
                None,
                Some(Either::Right(Number::Int(5))),
            ),
            make_test_instruction(
                Opcode::Mul64Imm,
                Some(Register { n: 1 }),
                None,
                None,
                Some(Either::Right(Number::Int(2))),
            ),
        ];

        let mut vm = SbpfVm::new(program, vec![], vec![], MockSyscallHandler::default());

        vm.step().unwrap();
        assert_eq!(vm.registers[1], 10);

        vm.step().unwrap();
        assert_eq!(vm.registers[1], 15);

        vm.step().unwrap();
        assert_eq!(vm.registers[1], 30);
    }

    #[test]
    fn test_memory_regions() {
        // Check input region
        let input = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let rodata = vec![10, 20, 30, 40];

        let program = vec![make_test_instruction(Opcode::Exit, None, None, None, None)];
        let vm = SbpfVm::new(program, input, rodata, MockSyscallHandler::default());

        assert_eq!(vm.memory.read_u8(Memory::INPUT_START).unwrap(), 1);
        assert_eq!(
            vm.memory.read_u64(Memory::INPUT_START).unwrap(),
            0x0807060504030201u64
        );

        // Check rodata region
        assert_eq!(vm.memory.read_u8(Memory::RODATA_START).unwrap(), 10);
    }

    #[test]
    fn test_program_without_exit() {
        let program = vec![
            make_test_instruction(
                Opcode::Mov64Imm,
                Some(Register { n: 0 }),
                None,
                None,
                Some(Either::Right(Number::Int(10))),
            ),
            make_test_instruction(
                Opcode::Add64Imm,
                Some(Register { n: 0 }),
                None,
                None,
                Some(Either::Right(Number::Int(8))),
            ),
            // no exit instruction
        ];

        let mut vm = SbpfVm::new(program, vec![], vec![], MockSyscallHandler::default());

        vm.step().unwrap();
        assert_eq!(vm.pc, 1);

        vm.step().unwrap();
        assert_eq!(vm.pc, 2);

        let result = vm.step();
        assert!(result.is_err());
        assert!(matches!(result, Err(SbpfVmError::PcOutOfBounds(2))));
    }

    #[test]
    fn test_step_complete_program() {
        // mov64 r1, 10
        // add64 r1, 5
        // mul r1, 3
        // sub r1, 7
        // exit
        let program = vec![
            make_test_instruction(
                Opcode::Mov64Imm,
                Some(Register { n: 1 }),
                None,
                None,
                Some(Either::Right(Number::Int(10))),
            ),
            make_test_instruction(
                Opcode::Add64Imm,
                Some(Register { n: 1 }),
                None,
                None,
                Some(Either::Right(Number::Int(5))),
            ),
            make_test_instruction(
                Opcode::Mul64Imm,
                Some(Register { n: 1 }),
                None,
                None,
                Some(Either::Right(Number::Int(3))),
            ),
            make_test_instruction(
                Opcode::Sub64Imm,
                Some(Register { n: 1 }),
                None,
                None,
                Some(Either::Right(Number::Int(7))),
            ),
            make_test_instruction(Opcode::Exit, None, None, None, None),
        ];

        let mut vm = SbpfVm::new(program, vec![], vec![], MockSyscallHandler::default());

        vm.step().unwrap();
        assert_eq!(vm.pc, 1);
        assert_eq!(vm.registers[1], 10);
        assert_eq!(vm.compute_units_consumed, 1);
        assert!(!vm.halted);

        vm.step().unwrap();
        assert_eq!(vm.pc, 2);
        assert_eq!(vm.registers[1], 15);
        assert_eq!(vm.compute_units_consumed, 2);
        assert!(!vm.halted);

        vm.step().unwrap();
        assert_eq!(vm.pc, 3);
        assert_eq!(vm.registers[1], 45);
        assert_eq!(vm.compute_units_consumed, 3);
        assert!(!vm.halted);

        vm.step().unwrap();
        assert_eq!(vm.pc, 4);
        assert_eq!(vm.registers[1], 38);
        assert_eq!(vm.compute_units_consumed, 4);
        assert!(!vm.halted);

        vm.step().unwrap();
        assert_eq!(vm.pc, 4);
        assert_eq!(vm.registers[1], 38);
        assert_eq!(vm.compute_units_consumed, 5);
        assert!(vm.halted);
    }

    #[test]
    fn test_run_complete_program() {
        // mov64 r1, 10
        // add64 r1, 5
        // mul r1, 3
        // sub r1, 7
        // exit
        let program = vec![
            make_test_instruction(
                Opcode::Mov64Imm,
                Some(Register { n: 1 }),
                None,
                None,
                Some(Either::Right(Number::Int(10))),
            ),
            make_test_instruction(
                Opcode::Add64Imm,
                Some(Register { n: 1 }),
                None,
                None,
                Some(Either::Right(Number::Int(5))),
            ),
            make_test_instruction(
                Opcode::Mul64Imm,
                Some(Register { n: 1 }),
                None,
                None,
                Some(Either::Right(Number::Int(3))),
            ),
            make_test_instruction(
                Opcode::Sub64Imm,
                Some(Register { n: 1 }),
                None,
                None,
                Some(Either::Right(Number::Int(7))),
            ),
            make_test_instruction(Opcode::Exit, None, None, None, None),
        ];

        let mut vm = SbpfVm::new(program, vec![], vec![], MockSyscallHandler::default());

        vm.run().unwrap();

        assert!(vm.halted);
        assert_eq!(vm.registers[1], 38);
        assert_eq!(vm.pc, 4);
        assert_eq!(vm.compute_units_consumed, 5);
    }

    #[test]
    fn test_program_with_input() {
        // ldxdw r2, [r1 + 0]
        // ldxdw r3, [r1 + 8]
        // mov64 r4, r2
        // add64 r4, r3
        // exit

        let mut input = Vec::new();
        input.extend_from_slice(&10u64.to_le_bytes());
        input.extend_from_slice(&20u64.to_le_bytes());

        let program = vec![
            make_test_instruction(
                Opcode::Ldxdw,
                Some(Register { n: 2 }),
                Some(Register { n: 1 }),
                Some(Either::Right(0)),
                None,
            ),
            make_test_instruction(
                Opcode::Ldxdw,
                Some(Register { n: 3 }),
                Some(Register { n: 1 }),
                Some(Either::Right(8)),
                None,
            ),
            make_test_instruction(
                Opcode::Mov64Reg,
                Some(Register { n: 4 }),
                Some(Register { n: 2 }),
                None,
                None,
            ),
            make_test_instruction(
                Opcode::Add64Reg,
                Some(Register { n: 4 }),
                Some(Register { n: 3 }),
                None,
                None,
            ),
            make_test_instruction(Opcode::Exit, None, None, None, None),
        ];

        let mut vm = SbpfVm::new(program, input, vec![], MockSyscallHandler::default());

        vm.run().unwrap();

        assert!(vm.halted);
        assert_eq!(vm.registers[2], 10);
        assert_eq!(vm.registers[3], 20);
        assert_eq!(vm.registers[4], 30);
        assert_eq!(vm.compute_units_consumed, 5);
    }

    #[test]
    fn test_program_with_internal_function_call() {
        // call test
        // lddw r2, 0x2
        // exit
        //
        // test:
        //   lddw r1, 0x1
        //   exit
        let program = vec![
            make_test_instruction(
                Opcode::Call,
                None,
                None,
                None,
                Some(Either::Right(Number::Int(3))),
            ),
            make_test_instruction(
                Opcode::Lddw,
                Some(Register { n: 2 }),
                None,
                None,
                Some(Either::Right(Number::Int(0x2))),
            ),
            make_test_instruction(Opcode::Exit, None, None, None, None),
            make_test_instruction(
                Opcode::Lddw,
                Some(Register { n: 1 }),
                None,
                None,
                Some(Either::Right(Number::Int(0x1))),
            ),
            make_test_instruction(Opcode::Exit, None, None, None, None),
        ];

        let mut vm = SbpfVm::new(program, vec![], vec![], MockSyscallHandler::default());

        vm.run().unwrap();

        assert!(vm.halted);
        assert_eq!(vm.registers[1], 0x1);
        assert_eq!(vm.registers[2], 0x2);
    }
}
