use {
    crate::{
        errors::{VmError, VmResult},
        memory::Memory,
    },
    sbpf_common::instruction::Instruction,
    serde::{Deserialize, Serialize},
};

/// VM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmConfig {
    pub max_call_depth: usize,
    pub max_steps: u64,
    pub stack_size: usize,
    pub heap_size: usize,
}

impl Default for VmConfig {
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vm {
    pub config: VmConfig,
    pub registers: [u64; 11],
    pub pc: usize,
    pub call_stack: Vec<CallFrame>,
    pub memory: Memory,
    pub program: Vec<Instruction>,
    pub halted: bool,
    pub exit_code: Option<u64>,
    pub compute_units_consumed: u64,
}

impl Vm {
    pub fn new(program: Vec<Instruction>, input: Vec<u8>, rodata: Vec<u8>) -> Self {
        Self::new_with_config(program, input, rodata, VmConfig::default())
    }

    pub fn new_with_config(
        program: Vec<Instruction>,
        input: Vec<u8>,
        rodata: Vec<u8>,
        config: VmConfig,
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

    pub fn current_instruction(&self) -> VmResult<&Instruction> {
        self.program
            .get(self.pc)
            .ok_or(VmError::PcOutOfBounds(self.pc))
    }

    pub fn set_entrypoint(&mut self, pc: usize) {
        self.pc = pc;
    }

    pub fn is_pc_valid(&self) -> bool {
        self.pc < self.program.len()
    }

    pub fn step(
        &mut self,
        syscall_handler: &mut dyn crate::syscalls::SyscallHandler,
    ) -> VmResult<()> {
        if self.halted {
            return Ok(());
        }

        if !self.is_pc_valid() {
            return Err(VmError::PcOutOfBounds(self.pc));
        }

        let inst = self.current_instruction()?.clone();
        crate::instructions::execute_instruction(self, &inst, syscall_handler)?;

        self.compute_units_consumed += 1;
        // TODO: Handle dynamic CU costs for syscalls
        Ok(())
    }

    pub fn run(
        &mut self,
        syscall_handler: &mut dyn crate::syscalls::SyscallHandler,
    ) -> VmResult<()> {
        let mut steps = 0;

        while !self.halted && steps < self.config.max_steps {
            self.step(syscall_handler)?;
            steps += 1;
        }

        if !self.halted && steps >= self.config.max_steps {
            return Err(VmError::ExecutionLimitReached(self.config.max_steps));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{helpers::make_test_instruction, syscalls::MockSyscallHandler},
        either::Either,
        sbpf_common::{
            inst_param::{Number, Register},
            opcode::Opcode,
        },
    };

    #[test]
    fn test_vm_initialization() {
        let program = vec![make_test_instruction(Opcode::Exit, None, None, None, None)];
        let vm = Vm::new(program, vec![1, 2, 3, 4], vec![]);

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
        let mut vm = Vm::new(program, vec![1, 2, 3, 4], vec![]);

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
        let vm = Vm::new(program, vec![], vec![]);

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

        let mut vm = Vm::new(program, vec![], vec![]);
        let mut handler = MockSyscallHandler::default();

        for _ in 0..5 {
            vm.step(&mut handler).unwrap();
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

        let mut vm = Vm::new(program, vec![], vec![]);
        let mut handler = MockSyscallHandler::default();

        vm.step(&mut handler).unwrap();
        assert_eq!(vm.registers[1], 10);

        vm.step(&mut handler).unwrap();
        assert_eq!(vm.registers[1], 15);

        vm.step(&mut handler).unwrap();
        assert_eq!(vm.registers[1], 30);
    }

    #[test]
    fn test_memory_regions() {
        // Check input region
        let input = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let rodata = vec![10, 20, 30, 40];

        let program = vec![make_test_instruction(Opcode::Exit, None, None, None, None)];
        let vm = Vm::new(program, input, rodata);

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

        let mut vm = Vm::new(program, vec![], vec![]);
        let mut handler = MockSyscallHandler::default();

        vm.step(&mut handler).unwrap();
        assert_eq!(vm.pc, 1);

        vm.step(&mut handler).unwrap();
        assert_eq!(vm.pc, 2);

        let result = vm.step(&mut handler);
        assert!(result.is_err());
        assert!(matches!(result, Err(VmError::PcOutOfBounds(2))));
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

        let mut vm = Vm::new(program, vec![], vec![]);
        let mut handler = MockSyscallHandler::default();

        vm.step(&mut handler).unwrap();
        assert_eq!(vm.pc, 1);
        assert_eq!(vm.registers[1], 10);
        assert_eq!(vm.compute_units_consumed, 1);
        assert!(!vm.halted);

        vm.step(&mut handler).unwrap();
        assert_eq!(vm.pc, 2);
        assert_eq!(vm.registers[1], 15);
        assert_eq!(vm.compute_units_consumed, 2);
        assert!(!vm.halted);

        vm.step(&mut handler).unwrap();
        assert_eq!(vm.pc, 3);
        assert_eq!(vm.registers[1], 45);
        assert_eq!(vm.compute_units_consumed, 3);
        assert!(!vm.halted);

        vm.step(&mut handler).unwrap();
        assert_eq!(vm.pc, 4);
        assert_eq!(vm.registers[1], 38);
        assert_eq!(vm.compute_units_consumed, 4);
        assert!(!vm.halted);

        vm.step(&mut handler).unwrap();
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

        let mut vm = Vm::new(program, vec![], vec![]);
        let mut handler = MockSyscallHandler::default();

        vm.run(&mut handler).unwrap();

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

        let mut vm = Vm::new(program, input, vec![]);
        let mut handler = MockSyscallHandler::default();

        vm.run(&mut handler).unwrap();

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

        let mut vm = Vm::new(program, vec![], vec![]);
        let mut handler = MockSyscallHandler::default();

        vm.run(&mut handler).unwrap();

        assert!(vm.halted);
        assert_eq!(vm.registers[1], 0x1);
        assert_eq!(vm.registers[2], 0x2);
    }
}
