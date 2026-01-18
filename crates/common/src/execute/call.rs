use crate::{errors::ExecutionError, inst_param::Number, instruction::Instruction};

use super::{SbpfVm, ExecutionResult};

pub fn execute_call_immediate(
    vm: &mut dyn SbpfVm,
    inst: &Instruction,
) -> ExecutionResult<()> {
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
            let saved_regs = [
                vm.get_register(6),
                vm.get_register(7),
                vm.get_register(8),
                vm.get_register(9),
            ];
            let saved_fp = vm.get_register(10);
            let return_pc = vm.get_pc() + 1;
            vm.push_frame(return_pc, saved_regs, saved_fp)?;
            vm.set_register(10, saved_fp.wrapping_add(vm.stack_frame_size()));
            vm.set_pc(*target as usize);
            Ok(())
        }
        _ => Err(ExecutionError::InvalidOperand),
    }
}

pub fn execute_call_register(
    vm: &mut dyn SbpfVm,
    inst: &Instruction,
) -> ExecutionResult<()> {
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

    let saved_regs = [
        vm.get_register(6),
        vm.get_register(7),
        vm.get_register(8),
        vm.get_register(9),
    ];
    let saved_fp = vm.get_register(10);
    let return_pc = vm.get_pc() + 1;
    vm.push_frame(return_pc, saved_regs, saved_fp)?;
    vm.set_register(10, saved_fp.wrapping_add(vm.stack_frame_size()));
    vm.set_pc(target);
    Ok(())
}

pub fn execute_exit(vm: &mut dyn SbpfVm, _inst: &Instruction) -> ExecutionResult<()> {
    if let Some((return_pc, saved_regs, saved_fp)) = vm.pop_frame() {
        vm.set_register(6, saved_regs[0]);
        vm.set_register(7, saved_regs[1]);
        vm.set_register(8, saved_regs[2]);
        vm.set_register(9, saved_regs[3]);
        vm.set_register(10, saved_fp);
        vm.set_pc(return_pc);
        Ok(())
    } else {
        vm.halt(vm.get_register(0));
        Ok(())
    }
}
