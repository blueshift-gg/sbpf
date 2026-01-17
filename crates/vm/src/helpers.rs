use {
    crate::errors::{VmError, VmResult},
    either::Either,
    sbpf_common::{inst_param::Number, instruction::Instruction},
};

pub fn get_imm_i64(inst: &Instruction) -> VmResult<i64> {
    match &inst.imm {
        Some(Either::Right(Number::Int(v))) => Ok(*v),
        Some(Either::Right(Number::Addr(v))) => Ok(*v),
        Some(Either::Left(_ident)) => Err(VmError::InvalidOperand),
        None => Err(VmError::InvalidOperand),
    }
}

pub fn get_imm_u64(inst: &Instruction) -> VmResult<u64> {
    get_imm_i64(inst).map(|v| v as u64)
}

pub fn get_offset(inst: &Instruction) -> VmResult<i16> {
    match &inst.off {
        Some(Either::Right(off)) => Ok(*off),
        Some(Either::Left(_ident)) => Err(VmError::InvalidOperand),
        None => Err(VmError::InvalidOperand),
    }
}

pub fn get_dst(inst: &Instruction) -> VmResult<usize> {
    inst.dst
        .as_ref()
        .map(|r| r.n as usize)
        .ok_or(VmError::InvalidOperand)
}

pub fn get_src(inst: &Instruction) -> VmResult<usize> {
    inst.src
        .as_ref()
        .map(|r| r.n as usize)
        .ok_or(VmError::InvalidOperand)
}

pub fn calculate_address(base: u64, offset: i16) -> u64 {
    if offset >= 0 {
        base.wrapping_add(offset as u64)
    } else {
        base.wrapping_sub((-offset) as u64)
    }
}

#[cfg(test)]
pub fn make_test_instruction(
    opcode: sbpf_common::opcode::Opcode,
    dst: Option<sbpf_common::inst_param::Register>,
    src: Option<sbpf_common::inst_param::Register>,
    off: Option<Either<String, i16>>,
    imm: Option<Either<String, Number>>,
) -> Instruction {
    Instruction {
        opcode,
        dst,
        src,
        off,
        imm,
        span: 0..0,
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        sbpf_common::{inst_param::Register, opcode::Opcode},
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
