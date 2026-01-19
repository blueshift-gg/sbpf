use {
    super::{ExecutionResult, Vm},
    crate::{errors::ExecutionError, inst_param::Number, instruction::Instruction},
};

pub fn execute_call_immediate(vm: &mut dyn Vm, inst: &Instruction) -> ExecutionResult<()> {
    match &inst.imm {
        Some(either::Either::Left(syscall_name)) => {
            let result = vm.handle_syscall(syscall_name)?;
            vm.set_register(0, result);
            vm.advance_pc();
            Ok(())
        }
        Some(either::Either::Right(Number::Int(target))) => {
            if vm.get_call_depth() >= vm.max_call_depth() {
                return Err(ExecutionError::CallDepthExceeded(vm.max_call_depth()));
            }
            let saved_registers = [
                vm.get_register(6),
                vm.get_register(7),
                vm.get_register(8),
                vm.get_register(9),
            ];
            let saved_frame_pointer = vm.get_register(10);
            let return_pc = vm.get_pc() + 1;
            vm.push_frame(return_pc, saved_registers, saved_frame_pointer)?;
            vm.set_register(
                10,
                saved_frame_pointer.wrapping_add(vm.get_stack_frame_size()),
            );
            vm.set_pc(*target as usize);
            Ok(())
        }
        _ => Err(ExecutionError::InvalidOperand),
    }
}

pub fn execute_call_register(vm: &mut dyn Vm, inst: &Instruction) -> ExecutionResult<()> {
    let reg_num = match &inst.imm {
        Some(either::Either::Right(Number::Int(n))) => *n as usize,
        _ => return Err(ExecutionError::InvalidOperand),
    };
    if reg_num >= 10 {
        return Err(ExecutionError::InvalidOperand);
    }
    let target = vm.get_register(reg_num) as usize;

    if vm.get_call_depth() >= vm.max_call_depth() {
        return Err(ExecutionError::CallDepthExceeded(vm.max_call_depth()));
    }

    let saved_registers = [
        vm.get_register(6),
        vm.get_register(7),
        vm.get_register(8),
        vm.get_register(9),
    ];
    let saved_frame_pointer = vm.get_register(10);
    let return_pc = vm.get_pc() + 1;
    vm.push_frame(return_pc, saved_registers, saved_frame_pointer)?;
    vm.set_register(
        10,
        saved_frame_pointer.wrapping_add(vm.get_stack_frame_size()),
    );
    vm.set_pc(target);
    Ok(())
}

pub fn execute_exit(vm: &mut dyn Vm, _inst: &Instruction) -> ExecutionResult<()> {
    if let Some((return_pc, saved_registers, saved_frame_pointer)) = vm.pop_frame() {
        vm.set_register(6, saved_registers[0]);
        vm.set_register(7, saved_registers[1]);
        vm.set_register(8, saved_registers[2]);
        vm.set_register(9, saved_registers[3]);
        vm.set_register(10, saved_frame_pointer);
        vm.set_pc(return_pc);
        Ok(())
    } else {
        vm.halt(vm.get_register(0));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            errors::ExecutionError,
            execute::{MockVm, make_test_instruction},
            inst_param::Number,
            opcode::Opcode,
        },
        either::Either,
    };

    #[test]
    fn test_syscall() {
        // call sol_log_
        let inst = make_test_instruction(
            Opcode::Call,
            None,
            None,
            None,
            Some(Either::Left("sol_log_".to_string())),
        );
        let mut vm = MockVm::new();

        execute_call_immediate(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 1);
        assert_eq!(vm.syscall_logs.len(), 1);
        assert_eq!(vm.syscall_logs[0], "sol_log_");
    }

    #[test]
    fn test_internal_call() {
        // call 10
        let inst = make_test_instruction(
            Opcode::Call,
            None,
            None,
            None,
            Some(Either::Right(Number::Int(10))),
        );
        let mut vm = MockVm::new();
        vm.registers[6] = 100;
        vm.registers[7] = 200;

        execute_call_immediate(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 10); // jumped to target
        assert_eq!(vm.call_stack.len(), 1);
        assert_eq!(vm.call_stack[0].0, 1);
        assert_eq!(vm.call_stack[0].1[0], 100); // r6
        assert_eq!(vm.call_stack[0].1[1], 200); // r7
    }

    #[test]
    fn test_internal_call_depth_exceeded() {
        // call 10
        let inst = make_test_instruction(
            Opcode::Call,
            None,
            None,
            None,
            Some(Either::Right(Number::Int(10))),
        );

        let mut vm = MockVm::new();
        vm.call_depth_limit = 1;

        // first call succeeds
        execute_call_immediate(&mut vm, &inst).unwrap();
        assert_eq!(vm.call_stack.len(), 1);

        // second call fails
        let result = execute_call_immediate(&mut vm, &inst);
        assert!(matches!(result, Err(ExecutionError::CallDepthExceeded(1))));
    }

    #[test]
    fn test_callx() {
        // callx r5
        let inst = make_test_instruction(
            Opcode::Callx,
            None,
            None,
            None,
            Some(Either::Right(Number::Int(5))),
        );
        let mut vm = MockVm::new();
        vm.registers[5] = 20; // target address
        vm.registers[6] = 100;
        vm.registers[7] = 200;

        execute_call_register(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 20); // jumped to target
        assert_eq!(vm.call_stack.len(), 1);
        assert_eq!(vm.call_stack[0].0, 1);
        assert_eq!(vm.call_stack[0].1[0], 100); // r6
        assert_eq!(vm.call_stack[0].1[1], 200); // r7
    }

    #[test]
    fn test_callx_r10_invalid() {
        // callx r10
        let inst = make_test_instruction(
            Opcode::Callx,
            None,
            None,
            None,
            Some(Either::Right(Number::Int(10))),
        );
        let mut vm = MockVm::new();

        let result = execute_call_register(&mut vm, &inst);
        assert!(matches!(result, Err(ExecutionError::InvalidOperand)));
    }

    #[test]
    fn test_callx_depth_exceeded() {
        // callx r5
        let inst = make_test_instruction(
            Opcode::Callx,
            None,
            None,
            None,
            Some(Either::Right(Number::Int(5))),
        );
        let mut vm = MockVm::new();
        vm.registers[5] = 0;
        vm.call_depth_limit = 1;

        // first call succeeds
        execute_call_register(&mut vm, &inst).unwrap();
        assert_eq!(vm.call_stack.len(), 1);

        // second call fails
        let result = execute_call_register(&mut vm, &inst);
        assert!(matches!(result, Err(ExecutionError::CallDepthExceeded(1))));
    }

    #[test]
    fn test_exit_halt() {
        // exit - halts with r0 as exit code
        let inst = make_test_instruction(Opcode::Exit, None, None, None, None);
        let mut vm = MockVm::new();
        vm.registers[0] = 42; // exit code

        execute_exit(&mut vm, &inst).unwrap();

        assert!(vm.halted);
        assert_eq!(vm.exit_code, Some(42));
    }

    #[test]
    fn test_exit_return() {
        // exit from a function call - should return to caller
        let inst = make_test_instruction(Opcode::Exit, None, None, None, None);
        let mut vm = MockVm::new();

        // Simulate being in a function call
        vm.call_stack.push((5, [100, 200, 300, 400], 0x1000));
        vm.registers[0] = 99;

        execute_exit(&mut vm, &inst).unwrap();

        assert!(!vm.halted);
        assert_eq!(vm.pc, 5); // returned to saved PC
        assert_eq!(vm.registers[6], 100);
        assert_eq!(vm.registers[7], 200);
        assert_eq!(vm.registers[8], 300);
        assert_eq!(vm.registers[9], 400);
        assert_eq!(vm.registers[10], 0x1000);
    }
}
