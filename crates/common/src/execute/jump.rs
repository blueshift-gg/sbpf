use crate::{errors::ExecutionError, instruction::Instruction, opcode::Opcode};

use super::{SbpfVm, ExecutionResult, helpers::*};

pub fn execute_jump(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    let off = get_offset(inst)?;
    vm.set_pc(((vm.get_pc() as i64) + 1 + (off as i64)) as usize);
    Ok(())
}

pub fn execute_jump_immediate(
    vm: &mut dyn SbpfVm,
    inst: &Instruction,
) -> ExecutionResult<()> {
    match inst.opcode {
        Opcode::JeqImm => execute_jump_immediate_conditional(vm, inst, |a, b| a == b),
        Opcode::JgtImm => execute_jump_immediate_conditional(vm, inst, |a, b| a > b),
        Opcode::JgeImm => execute_jump_immediate_conditional(vm, inst, |a, b| a >= b),
        Opcode::JltImm => execute_jump_immediate_conditional(vm, inst, |a, b| a < b),
        Opcode::JleImm => execute_jump_immediate_conditional(vm, inst, |a, b| a <= b),
        Opcode::JsetImm => execute_jump_immediate_conditional(vm, inst, |a, b| (a & b) != 0),
        Opcode::JneImm => execute_jump_immediate_conditional(vm, inst, |a, b| a != b),
        Opcode::JsgtImm => {
            execute_jump_immediate_conditional(vm, inst, |a, b| (a as i64) > (b as i64))
        }
        Opcode::JsgeImm => {
            execute_jump_immediate_conditional(vm, inst, |a, b| (a as i64) >= (b as i64))
        }
        Opcode::JsltImm => {
            execute_jump_immediate_conditional(vm, inst, |a, b| (a as i64) < (b as i64))
        }
        Opcode::JsleImm => {
            execute_jump_immediate_conditional(vm, inst, |a, b| (a as i64) <= (b as i64))
        }
        _ => Err(ExecutionError::InvalidInstruction),
    }
}

pub fn execute_jump_register(
    vm: &mut dyn SbpfVm,
    inst: &Instruction,
) -> ExecutionResult<()> {
    match inst.opcode {
        Opcode::JeqReg => execute_jump_register_conditional(vm, inst, |a, b| a == b),
        Opcode::JgtReg => execute_jump_register_conditional(vm, inst, |a, b| a > b),
        Opcode::JgeReg => execute_jump_register_conditional(vm, inst, |a, b| a >= b),
        Opcode::JltReg => execute_jump_register_conditional(vm, inst, |a, b| a < b),
        Opcode::JleReg => execute_jump_register_conditional(vm, inst, |a, b| a <= b),
        Opcode::JsetReg => execute_jump_register_conditional(vm, inst, |a, b| (a & b) != 0),
        Opcode::JneReg => execute_jump_register_conditional(vm, inst, |a, b| a != b),
        Opcode::JsgtReg => {
            execute_jump_register_conditional(vm, inst, |a, b| (a as i64) > (b as i64))
        }
        Opcode::JsgeReg => {
            execute_jump_register_conditional(vm, inst, |a, b| (a as i64) >= (b as i64))
        }
        Opcode::JsltReg => {
            execute_jump_register_conditional(vm, inst, |a, b| (a as i64) < (b as i64))
        }
        Opcode::JsleReg => {
            execute_jump_register_conditional(vm, inst, |a, b| (a as i64) <= (b as i64))
        }
        _ => Err(ExecutionError::InvalidInstruction),
    }
}

fn execute_jump_immediate_conditional(
    vm: &mut dyn SbpfVm,
    inst: &Instruction,
    condition: fn(u64, u64) -> bool,
) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let off = get_offset(inst)?;
    let imm = (get_imm_i64(inst)? as i32 as i64) as u64;

    if condition(vm.get_register(dst), imm) {
        vm.set_pc(((vm.get_pc() as i64) + 1 + (off as i64)) as usize);
    } else {
        vm.advance_pc();
    }
    Ok(())
}

fn execute_jump_register_conditional(
    vm: &mut dyn SbpfVm,
    inst: &Instruction,
    condition: fn(u64, u64) -> bool,
) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;

    if condition(vm.get_register(dst), vm.get_register(src)) {
        vm.set_pc(((vm.get_pc() as i64) + 1 + (off as i64)) as usize);
    } else {
        vm.advance_pc();
    }
    Ok(())
}
