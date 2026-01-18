use crate::{errors::ExecutionError, instruction::Instruction, opcode::Opcode};

use super::{SbpfVm, ExecutionResult, helpers::*};

pub fn execute_endian(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let imm = get_imm_i64(inst)?;

    if imm != 16 && imm != 32 && imm != 64 {
        return Err(ExecutionError::InvalidOperand);
    }

    vm.set_register(
        dst,
        match inst.opcode {
            Opcode::Le => match imm {
                16 => (vm.get_register(dst) as u16).to_le() as u64,
                32 => (vm.get_register(dst) as u32).to_le() as u64,
                64 => vm.get_register(dst).to_le(),
                _ => unreachable!(),
            },
            Opcode::Be => match imm {
                16 => (vm.get_register(dst) as u16).to_be() as u64,
                32 => (vm.get_register(dst) as u32).to_be() as u64,
                64 => vm.get_register(dst).to_be(),
                _ => unreachable!(),
            },
            _ => return Err(ExecutionError::InvalidInstruction),
        },
    );

    vm.advance_pc();
    Ok(())
}
