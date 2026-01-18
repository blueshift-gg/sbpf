use crate::{errors::ExecutionError, inst_param::Number, instruction::Instruction};

use super::ExecutionResult;

pub fn get_dst(inst: &Instruction) -> ExecutionResult<usize> {
    inst.dst
        .as_ref()
        .map(|r| r.n as usize)
        .ok_or(ExecutionError::InvalidOperand)
}

pub fn get_src(inst: &Instruction) -> ExecutionResult<usize> {
    inst.src
        .as_ref()
        .map(|r| r.n as usize)
        .ok_or(ExecutionError::InvalidOperand)
}

pub fn get_imm_i64(inst: &Instruction) -> ExecutionResult<i64> {
    match &inst.imm {
        Some(either::Either::Right(Number::Int(n))) => Ok(*n),
        _ => Err(ExecutionError::InvalidOperand),
    }
}

pub fn get_imm_u64(inst: &Instruction) -> ExecutionResult<u64> {
    match &inst.imm {
        Some(either::Either::Right(Number::Int(n))) => Ok(*n as u64),
        _ => Err(ExecutionError::InvalidOperand),
    }
}

pub fn get_offset(inst: &Instruction) -> ExecutionResult<i16> {
    match &inst.off {
        Some(either::Either::Right(off)) => Ok(*off),
        _ => Err(ExecutionError::InvalidOperand),
    }
}

pub fn calculate_address(base: u64, offset: i16) -> u64 {
    (base as i64).wrapping_add(offset as i64) as u64
}
