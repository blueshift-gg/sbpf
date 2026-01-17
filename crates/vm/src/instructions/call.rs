use {
    crate::{
        errors::{VmError, VmResult},
        memory::Memory,
        syscalls::{SyscallContext, SyscallHandler},
        vm::{CallFrame, Vm},
    },
    sbpf_common::{inst_param::Number, instruction::Instruction},
};

pub fn execute_call(
    vm: &mut Vm,
    inst: &Instruction,
    syscall_handler: &mut dyn SyscallHandler,
) -> VmResult<()> {
    match &inst.imm {
        // Execute syscall
        Some(either::Either::Left(syscall_name)) => {
            let ctx = SyscallContext {
                name: syscall_name,
                registers: [
                    vm.registers[1],
                    vm.registers[2],
                    vm.registers[3],
                    vm.registers[4],
                    vm.registers[5],
                ],
                memory: &mut vm.memory,
            };
            let result = syscall_handler.handle(ctx)?;
            // Store return value in r0.
            vm.registers[0] = result;
            vm.pc += 1;
            Ok(())
        }
        // Execute internal function call
        Some(either::Either::Right(Number::Int(target))) => {
            // Check call depth limit
            if vm.call_stack.len() >= vm.config.max_call_depth {
                return Err(VmError::CallDepthExceeded(vm.config.max_call_depth));
            }
            // Save call frame.
            vm.call_stack.push(CallFrame {
                return_pc: vm.pc + 1,
                saved_registers: [
                    vm.registers[6],
                    vm.registers[7],
                    vm.registers[8],
                    vm.registers[9],
                ],
                saved_frame_pointer: vm.registers[10],
            });
            // Add one stack frame size to frame pointer r10
            vm.registers[10] = vm.registers[10].wrapping_add(Memory::STACK_FRAME_SIZE);
            // Jump to target
            vm.pc = *target as usize;
            Ok(())
        }
        _ => Err(VmError::InvalidOperand),
    }
}

pub fn execute_callx(vm: &mut Vm, inst: &Instruction) -> VmResult<()> {
    let reg_num = match &inst.imm {
        Some(either::Either::Right(Number::Int(n))) => *n as usize,
        _ => return Err(VmError::InvalidOperand),
    };
    if reg_num >= 10 {
        // r10 is the frame pointer so it can't be used as callx target
        return Err(VmError::InvalidOperand);
    }
    let target = vm.registers[reg_num] as usize;

    // Check call depth limit
    if vm.call_stack.len() >= vm.config.max_call_depth {
        return Err(VmError::CallDepthExceeded(vm.config.max_call_depth));
    }

    // Save call frame
    vm.call_stack.push(CallFrame {
        return_pc: vm.pc + 1,
        saved_registers: [
            vm.registers[6],
            vm.registers[7],
            vm.registers[8],
            vm.registers[9],
        ],
        saved_frame_pointer: vm.registers[10],
    });

    // Add one stack frame size to frame pointer r10
    vm.registers[10] = vm.registers[10].wrapping_add(Memory::STACK_FRAME_SIZE);

    // Jump to target
    vm.pc = target;
    Ok(())
}

pub fn execute_exit(vm: &mut Vm) -> VmResult<()> {
    if let Some(frame) = vm.call_stack.pop() {
        // Return from internal function call

        // Restore callee-saved registers (r6-r9)
        vm.registers[6] = frame.saved_registers[0];
        vm.registers[7] = frame.saved_registers[1];
        vm.registers[8] = frame.saved_registers[2];
        vm.registers[9] = frame.saved_registers[3];
        // Restore frame pointer and return to caller
        vm.registers[10] = frame.saved_frame_pointer;
        vm.pc = frame.return_pc;
        Ok(())
    } else {
        // Exit program
        vm.halted = true;
        vm.exit_code = Some(vm.registers[0]);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{helpers::make_test_instruction, syscalls::MockSyscallHandler, vm::VmConfig},
        either::Either,
        sbpf_common::{inst_param::Number, opcode::Opcode},
    };

    #[test]
    fn test_syscall() {
        // call sol_log_
        let program = vec![make_test_instruction(
            Opcode::Call,
            None,
            None,
            None,
            Some(Either::Left("sol_log_".to_string())),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        let mut handler = MockSyscallHandler::default();

        let inst = vm.program[0].clone();
        execute_call(&mut vm, &inst, &mut handler).unwrap();

        assert_eq!(vm.pc, 1);
        // check if the syscall was logged
        assert_eq!(handler.logs.len(), 1);
        assert_eq!(handler.logs[0], "syscall: sol_log_");
    }

    #[test]
    fn test_internal_call() {
        // call 10
        let program = vec![make_test_instruction(
            Opcode::Call,
            None,
            None,
            None,
            Some(Either::Right(Number::Int(10))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[6] = 100;
        vm.registers[7] = 200;

        let mut handler = MockSyscallHandler::default();
        let inst = vm.program[0].clone();
        execute_call(&mut vm, &inst, &mut handler).unwrap();

        assert_eq!(vm.pc, 10); // jumped to target
        assert_eq!(vm.call_stack.len(), 1);
        assert_eq!(vm.call_stack[0].return_pc, 1);
        assert_eq!(vm.call_stack[0].saved_registers[0], 100); // r6
        assert_eq!(vm.call_stack[0].saved_registers[1], 200); // r7
    }

    #[test]
    fn test_internal_call_depth_exceeded() {
        // call 10
        let program = vec![make_test_instruction(
            Opcode::Call,
            None,
            None,
            None,
            Some(Either::Right(Number::Int(10))),
        )];
        let config = VmConfig {
            max_call_depth: 2,
            ..Default::default()
        };
        let mut vm = Vm::new_with_config(program, vec![], vec![], config);
        let mut handler = MockSyscallHandler::default();

        let inst = vm.program[0].clone();

        // first call succeeds
        execute_call(&mut vm, &inst, &mut handler).unwrap();
        assert_eq!(vm.call_stack.len(), 1);

        // second call succeeds
        execute_call(&mut vm, &inst, &mut handler).unwrap();
        assert_eq!(vm.call_stack.len(), 2);

        // third call should fail
        let result = execute_call(&mut vm, &inst, &mut handler);
        assert!(matches!(result, Err(VmError::CallDepthExceeded(2))));
    }

    #[test]
    fn test_callx() {
        // callx r5
        let program = vec![make_test_instruction(
            Opcode::Callx,
            None,
            None,
            None,
            Some(Either::Right(Number::Int(5))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[5] = 20; // target address
        vm.registers[6] = 100;
        vm.registers[7] = 200;

        let inst = vm.program[0].clone();
        execute_callx(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 20); // jumped to target
        assert_eq!(vm.call_stack.len(), 1);
        assert_eq!(vm.call_stack[0].return_pc, 1);
        assert_eq!(vm.call_stack[0].saved_registers[0], 100); // r6
        assert_eq!(vm.call_stack[0].saved_registers[1], 200); // r7
    }

    #[test]
    fn test_callx_r10_invalid() {
        // callx r10
        let program = vec![make_test_instruction(
            Opcode::Callx,
            None,
            None,
            None,
            Some(Either::Right(Number::Int(10))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);

        let inst = vm.program[0].clone();
        let result = execute_callx(&mut vm, &inst);

        assert!(matches!(result, Err(VmError::InvalidOperand)));
    }

    #[test]
    fn test_callx_depth_exceeded() {
        // callx r5
        let program = vec![make_test_instruction(
            Opcode::Callx,
            None,
            None,
            None,
            Some(Either::Right(Number::Int(5))),
        )];
        let config = VmConfig {
            max_call_depth: 1,
            ..Default::default()
        };
        let mut vm = Vm::new_with_config(program, vec![], vec![], config);
        vm.registers[5] = 0; // target address

        let inst = vm.program[0].clone();

        // first call succeeds
        execute_callx(&mut vm, &inst).unwrap();
        assert_eq!(vm.call_stack.len(), 1);

        // second call should fail
        let result = execute_callx(&mut vm, &inst);
        assert!(matches!(result, Err(VmError::CallDepthExceeded(1))));
    }

    #[test]
    fn test_exit() {
        // exit
        let program = vec![make_test_instruction(Opcode::Exit, None, None, None, None)];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[0] = 0; // exit code

        execute_exit(&mut vm).unwrap();

        assert!(vm.halted);
        assert_eq!(vm.exit_code, Some(0));
    }
}
