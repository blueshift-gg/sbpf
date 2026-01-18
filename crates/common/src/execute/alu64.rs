use crate::{errors::ExecutionError, instruction::Instruction, opcode::Opcode};

use super::{SbpfVm, ExecutionResult, helpers::*};

pub fn execute_alu64_imm(
    vm: &mut dyn SbpfVm,
    inst: &Instruction,
) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let imm = get_imm_i64(inst)?;
    let imm_u64 = imm as u64;

    match inst.opcode {
        Opcode::Add64Imm => vm.set_register(dst, vm.get_register(dst).wrapping_add(imm_u64)),
        Opcode::Sub64Imm => vm.set_register(dst, vm.get_register(dst).wrapping_sub(imm_u64)),
        Opcode::Mul64Imm => vm.set_register(dst, vm.get_register(dst).wrapping_mul(imm_u64)),
        Opcode::Div64Imm => {
            if imm_u64 == 0 {
                return Err(ExecutionError::DivisionByZero);
            }
            vm.set_register(dst, vm.get_register(dst) / imm_u64);
        }
        Opcode::Or64Imm => vm.set_register(dst, vm.get_register(dst) | imm_u64),
        Opcode::And64Imm => vm.set_register(dst, vm.get_register(dst) & imm_u64),
        Opcode::Lsh64Imm => vm.set_register(dst, vm.get_register(dst).wrapping_shl(imm as u32)),
        Opcode::Rsh64Imm => vm.set_register(dst, vm.get_register(dst).wrapping_shr(imm as u32)),
        Opcode::Mod64Imm => {
            if imm_u64 == 0 {
                return Err(ExecutionError::DivisionByZero);
            }
            vm.set_register(dst, vm.get_register(dst) % imm_u64);
        }
        Opcode::Xor64Imm => vm.set_register(dst, vm.get_register(dst) ^ imm_u64),
        Opcode::Mov64Imm => vm.set_register(dst, imm_u64),
        Opcode::Arsh64Imm => vm.set_register(
            dst,
            (vm.get_register(dst) as i64).wrapping_shr(imm as u32) as u64,
        ),
        _ => return Err(ExecutionError::InvalidInstruction),
    }

    vm.advance_pc();
    Ok(())
}

pub fn execute_alu64_reg(
    vm: &mut dyn SbpfVm,
    inst: &Instruction,
) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let src_val = vm.get_register(src);

    match inst.opcode {
        Opcode::Add64Reg => vm.set_register(dst, vm.get_register(dst).wrapping_add(src_val)),
        Opcode::Sub64Reg => vm.set_register(dst, vm.get_register(dst).wrapping_sub(src_val)),
        Opcode::Mul64Reg => vm.set_register(dst, vm.get_register(dst).wrapping_mul(src_val)),
        Opcode::Div64Reg => {
            if src_val == 0 {
                return Err(ExecutionError::DivisionByZero);
            }
            vm.set_register(dst, vm.get_register(dst) / src_val);
        }
        Opcode::Or64Reg => vm.set_register(dst, vm.get_register(dst) | src_val),
        Opcode::And64Reg => vm.set_register(dst, vm.get_register(dst) & src_val),
        Opcode::Lsh64Reg => {
            vm.set_register(dst, vm.get_register(dst).wrapping_shl(src_val as u32))
        }
        Opcode::Rsh64Reg => {
            vm.set_register(dst, vm.get_register(dst).wrapping_shr(src_val as u32))
        }
        Opcode::Mod64Reg => {
            if src_val == 0 {
                return Err(ExecutionError::DivisionByZero);
            }
            vm.set_register(dst, vm.get_register(dst) % src_val);
        }
        Opcode::Xor64Reg => vm.set_register(dst, vm.get_register(dst) ^ src_val),
        Opcode::Mov64Reg => vm.set_register(dst, src_val),
        Opcode::Arsh64Reg => vm.set_register(
            dst,
            (vm.get_register(dst) as i64).wrapping_shr(src_val as u32) as u64,
        ),
        _ => return Err(ExecutionError::InvalidInstruction),
    }

    vm.advance_pc();
    Ok(())
}

pub fn execute_neg64(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    vm.set_register(dst, (vm.get_register(dst) as i64).wrapping_neg() as u64);
    vm.advance_pc();
    Ok(())
}
