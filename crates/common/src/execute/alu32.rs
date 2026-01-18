use crate::{errors::ExecutionError, instruction::Instruction, opcode::Opcode};

use super::{SbpfVm, ExecutionResult, helpers::*};

pub fn execute_alu32_imm(
    vm: &mut dyn SbpfVm,
    inst: &Instruction,
) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let imm = get_imm_i64(inst)?;

    match inst.opcode {
        Opcode::Add32Imm => {
            let result = (vm.get_register(dst) as i32).wrapping_add(imm as i32);
            vm.set_register(dst, (result as i64) as u64);
        }
        Opcode::Sub32Imm => {
            let result = (vm.get_register(dst) as i32).wrapping_sub(imm as i32);
            vm.set_register(dst, (result as i64) as u64);
        }
        Opcode::Mul32Imm => {
            let result = (vm.get_register(dst) as i32).wrapping_mul(imm as i32);
            vm.set_register(dst, (result as i64) as u64);
        }
        Opcode::Div32Imm => {
            let imm_u32 = imm as u32;
            if imm_u32 == 0 {
                return Err(ExecutionError::DivisionByZero);
            }
            let result = (vm.get_register(dst) as u32) / imm_u32;
            vm.set_register(dst, result as u64);
        }
        Opcode::Or32Imm => {
            let result = (vm.get_register(dst) as u32) | (imm as u32);
            vm.set_register(dst, result as u64);
        }
        Opcode::And32Imm => {
            let result = (vm.get_register(dst) as u32) & (imm as u32);
            vm.set_register(dst, result as u64);
        }
        Opcode::Lsh32Imm => {
            let result = (vm.get_register(dst) as u32).wrapping_shl(imm as u32);
            vm.set_register(dst, result as u64);
        }
        Opcode::Rsh32Imm => {
            let result = (vm.get_register(dst) as u32).wrapping_shr(imm as u32);
            vm.set_register(dst, result as u64);
        }
        Opcode::Mod32Imm => {
            let imm_u32 = imm as u32;
            if imm_u32 == 0 {
                return Err(ExecutionError::DivisionByZero);
            }
            let result = (vm.get_register(dst) as u32) % imm_u32;
            vm.set_register(dst, result as u64);
        }
        Opcode::Xor32Imm => {
            let result = (vm.get_register(dst) as u32) ^ (imm as u32);
            vm.set_register(dst, result as u64);
        }
        Opcode::Mov32Imm => {
            vm.set_register(dst, (imm as u32) as u64);
        }
        Opcode::Arsh32Imm => {
            let result = (vm.get_register(dst) as i32).wrapping_shr(imm as u32) as u32;
            vm.set_register(dst, result as u64);
        }
        _ => return Err(ExecutionError::InvalidInstruction),
    };

    vm.advance_pc();
    Ok(())
}

pub fn execute_alu32_reg(
    vm: &mut dyn SbpfVm,
    inst: &Instruction,
) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let src_val = vm.get_register(src) as i32;
    let dst_val = vm.get_register(dst) as i32;

    match inst.opcode {
        Opcode::Add32Reg => {
            let result = dst_val.wrapping_add(src_val);
            vm.set_register(dst, (result as i64) as u64);
        }
        Opcode::Sub32Reg => {
            let result = dst_val.wrapping_sub(src_val);
            vm.set_register(dst, (result as i64) as u64);
        }
        Opcode::Mul32Reg => {
            let result = dst_val.wrapping_mul(src_val);
            vm.set_register(dst, (result as i64) as u64);
        }
        Opcode::Div32Reg => {
            let src_u32 = src_val as u32;
            let dst_u32 = dst_val as u32;
            if src_u32 == 0 {
                return Err(ExecutionError::DivisionByZero);
            }
            let result = dst_u32 / src_u32;
            vm.set_register(dst, result as u64);
        }
        Opcode::Or32Reg => {
            let result = (dst_val as u32) | (src_val as u32);
            vm.set_register(dst, result as u64);
        }
        Opcode::And32Reg => {
            let result = (dst_val as u32) & (src_val as u32);
            vm.set_register(dst, result as u64);
        }
        Opcode::Lsh32Reg => {
            let result = (dst_val as u32).wrapping_shl(src_val as u32);
            vm.set_register(dst, result as u64);
        }
        Opcode::Rsh32Reg => {
            let result = (dst_val as u32).wrapping_shr(src_val as u32);
            vm.set_register(dst, result as u64);
        }
        Opcode::Mod32Reg => {
            let src_u32 = src_val as u32;
            let dst_u32 = dst_val as u32;
            if src_u32 == 0 {
                return Err(ExecutionError::DivisionByZero);
            }
            let result = dst_u32 % src_u32;
            vm.set_register(dst, result as u64);
        }
        Opcode::Xor32Reg => {
            let result = (dst_val as u32) ^ (src_val as u32);
            vm.set_register(dst, result as u64);
        }
        Opcode::Mov32Reg => {
            vm.set_register(dst, (src_val as u32) as u64);
        }
        Opcode::Arsh32Reg => {
            let result = dst_val.wrapping_shr(src_val as u32) as u32;
            vm.set_register(dst, result as u64);
        }
        _ => return Err(ExecutionError::InvalidInstruction),
    };

    vm.advance_pc();
    Ok(())
}

pub fn execute_neg32(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let result = (vm.get_register(dst) as i32).wrapping_neg();
    vm.set_register(dst, result as u32 as u64);
    vm.advance_pc();
    Ok(())
}
