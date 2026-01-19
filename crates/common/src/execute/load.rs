use {
    super::{ExecutionResult, Vm, helpers::*},
    crate::instruction::Instruction,
};

pub fn execute_lddw(vm: &mut dyn Vm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let imm = get_imm_u64(inst)?;
    vm.set_register(dst, imm);
    vm.advance_pc();
    Ok(())
}

pub fn execute_ldxb(vm: &mut dyn Vm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.get_register(src), off);
    let value = vm.read_u8(addr)?;
    vm.set_register(dst, value as u64);
    vm.advance_pc();
    Ok(())
}

pub fn execute_ldxh(vm: &mut dyn Vm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.get_register(src), off);
    let value = vm.read_u16(addr)?;
    vm.set_register(dst, value as u64);
    vm.advance_pc();
    Ok(())
}

pub fn execute_ldxw(vm: &mut dyn Vm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.get_register(src), off);
    let value = vm.read_u32(addr)?;
    vm.set_register(dst, value as u64);
    vm.advance_pc();
    Ok(())
}

pub fn execute_ldxdw(vm: &mut dyn Vm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.get_register(src), off);
    let value = vm.read_u64(addr)?;
    vm.set_register(dst, value);
    vm.advance_pc();
    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            execute::{MockVm, make_test_instruction},
            inst_param::{Number, Register},
            opcode::Opcode,
        },
        either::Either,
    };

    #[test]
    fn test_lddw() {
        // lddw r1, 0x123456789ABCDEF0
        let inst = make_test_instruction(
            Opcode::Lddw,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(0x123456789ABCDEF0u64 as i64))),
        );
        let mut vm = MockVm::new();

        execute_lddw(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0x123456789ABCDEF0);
        assert_eq!(vm.pc, 1);
    }

    #[test]
    fn test_ldxb() {
        // ldxb r1, [r2+0]
        let inst = make_test_instruction(
            Opcode::Ldxb,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            Some(Either::Right(0)),
            None,
        );
        let mut vm = MockVm::new();
        vm.registers[2] = 0x100;
        vm.write_memory(0x100, &[0x7]);

        execute_ldxb(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0x7);
        assert_eq!(vm.pc, 1);
    }

    #[test]
    fn test_ldxh() {
        // ldxh r1, [r2+0]
        let inst = make_test_instruction(
            Opcode::Ldxh,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            Some(Either::Right(0)),
            None,
        );
        let mut vm = MockVm::new();
        vm.registers[2] = 0x100;
        vm.write_memory(0x100, &0xabcdu16.to_le_bytes());

        execute_ldxh(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0xabcd);
        assert_eq!(vm.pc, 1);
    }

    #[test]
    fn test_ldxw() {
        // ldxw r1, [r2+0]
        let inst = make_test_instruction(
            Opcode::Ldxw,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            Some(Either::Right(0)),
            None,
        );
        let mut vm = MockVm::new();
        vm.registers[2] = 0x100;
        vm.write_memory(0x100, &0xabcdef12u32.to_le_bytes());

        execute_ldxw(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0xabcdef12);
        assert_eq!(vm.pc, 1);
    }

    #[test]
    fn test_ldxdw() {
        // ldxdw r1, [r2+0]
        let inst = make_test_instruction(
            Opcode::Ldxdw,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            Some(Either::Right(0)),
            None,
        );
        let mut vm = MockVm::new();
        vm.registers[2] = 0x100;
        vm.write_memory(0x100, &0x123456789abcdef0u64.to_le_bytes());

        execute_ldxdw(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0x123456789abcdef0);
        assert_eq!(vm.pc, 1);
    }
}
