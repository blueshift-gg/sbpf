use crate::instruction::Instruction;

use super::{SbpfVm, ExecutionResult, helpers::*};

pub fn execute_stb(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let off = get_offset(inst)?;
    let imm = get_imm_i64(inst)? as u8;
    let addr = calculate_address(vm.get_register(dst), off);
    vm.write_u8(addr, imm)?;
    vm.advance_pc();
    Ok(())
}

pub fn execute_sth(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let off = get_offset(inst)?;
    let imm = get_imm_i64(inst)? as u16;
    let addr = calculate_address(vm.get_register(dst), off);
    vm.write_u16(addr, imm)?;
    vm.advance_pc();
    Ok(())
}

pub fn execute_stw(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let off = get_offset(inst)?;
    let imm = get_imm_i64(inst)? as u32;
    let addr = calculate_address(vm.get_register(dst), off);
    vm.write_u32(addr, imm)?;
    vm.advance_pc();
    Ok(())
}

pub fn execute_stdw(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let off = get_offset(inst)?;
    let imm = get_imm_i64(inst)? as u64;
    let addr = calculate_address(vm.get_register(dst), off);
    vm.write_u64(addr, imm)?;
    vm.advance_pc();
    Ok(())
}

pub fn execute_stxb(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.get_register(dst), off);
    vm.write_u8(addr, vm.get_register(src) as u8)?;
    vm.advance_pc();
    Ok(())
}

pub fn execute_stxh(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.get_register(dst), off);
    vm.write_u16(addr, vm.get_register(src) as u16)?;
    vm.advance_pc();
    Ok(())
}

pub fn execute_stxw(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.get_register(dst), off);
    vm.write_u32(addr, vm.get_register(src) as u32)?;
    vm.advance_pc();
    Ok(())
}

pub fn execute_stxdw(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.get_register(dst), off);
    vm.write_u64(addr, vm.get_register(src))?;
    vm.advance_pc();
    Ok(())
}
