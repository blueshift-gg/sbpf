use crate::instruction::Instruction;

use super::{SbpfVm, ExecutionResult, helpers::*};

pub fn execute_lddw(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let imm = get_imm_u64(inst)?;
    vm.set_register(dst, imm);
    vm.advance_pc();
    Ok(())
}

pub fn execute_ldxb(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.get_register(src), off);
    let value = vm.read_u8(addr)?;
    vm.set_register(dst, value as u64);
    vm.advance_pc();
    Ok(())
}

pub fn execute_ldxh(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.get_register(src), off);
    let value = vm.read_u16(addr)?;
    vm.set_register(dst, value as u64);
    vm.advance_pc();
    Ok(())
}

pub fn execute_ldxw(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.get_register(src), off);
    let value = vm.read_u32(addr)?;
    vm.set_register(dst, value as u64);
    vm.advance_pc();
    Ok(())
}

pub fn execute_ldxdw(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.get_register(src), off);
    let value = vm.read_u64(addr)?;
    vm.set_register(dst, value);
    vm.advance_pc();
    Ok(())
}
