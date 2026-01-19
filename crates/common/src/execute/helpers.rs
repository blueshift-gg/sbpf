use {
    super::ExecutionResult,
    crate::{errors::ExecutionError, inst_param::Number, instruction::Instruction},
};

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

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{execute::make_test_instruction, inst_param::Register, opcode::Opcode},
        either::Either,
    };

    #[test]
    fn test_calculate_address() {
        assert_eq!(calculate_address(0x1000, 8), 0x1008);
        assert_eq!(calculate_address(0x1000, -8), 0x0ff8);
        assert_eq!(calculate_address(0x1000, 0), 0x1000);
    }

    #[test]
    fn test_get_imm() {
        let inst = make_test_instruction(
            Opcode::Add64Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(10))),
        );
        assert_eq!(get_imm_i64(&inst).unwrap(), 10);
        assert_eq!(get_imm_u64(&inst).unwrap(), 10);
    }

    #[test]
    fn test_get_offset() {
        let inst = make_test_instruction(
            Opcode::Ldxw,
            Some(Register { n: 0 }),
            Some(Register { n: 1 }),
            Some(Either::Right(-8)),
            None,
        );
        assert_eq!(get_offset(&inst).unwrap(), -8);
    }

    #[test]
    fn test_get_registers() {
        let inst = make_test_instruction(
            Opcode::Add64Reg,
            Some(Register { n: 3 }),
            Some(Register { n: 5 }),
            None,
            None,
        );
        assert_eq!(get_dst(&inst).unwrap(), 3);
        assert_eq!(get_src(&inst).unwrap(), 5);
    }
}
