use {
    crate::{
        errors::VmResult,
        helpers::{calculate_address, get_dst, get_imm_u64, get_offset, get_src},
        vm::Vm,
    },
    sbpf_common::instruction::Instruction,
};

pub fn execute_lddw(vm: &mut Vm, inst: &Instruction) -> VmResult<()> {
    let dst = get_dst(inst)?;
    let imm = get_imm_u64(inst)?;

    vm.registers[dst] = imm;
    vm.pc += 1;
    Ok(())
}

pub fn execute_ldxb(vm: &mut Vm, inst: &Instruction) -> VmResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.registers[src], off);
    let value = vm.memory.read_u8(addr)?;

    vm.registers[dst] = value as u64;
    vm.pc += 1;
    Ok(())
}

pub fn execute_ldxh(vm: &mut Vm, inst: &Instruction) -> VmResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.registers[src], off);
    let value = vm.memory.read_u16(addr)?;

    vm.registers[dst] = value as u64;
    vm.pc += 1;
    Ok(())
}

pub fn execute_ldxw(vm: &mut Vm, inst: &Instruction) -> VmResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.registers[src], off);
    let value = vm.memory.read_u32(addr)?;

    vm.registers[dst] = value as u64;
    vm.pc += 1;
    Ok(())
}

pub fn execute_ldxdw(vm: &mut Vm, inst: &Instruction) -> VmResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.registers[src], off);
    let value = vm.memory.read_u64(addr)?;

    vm.registers[dst] = value;
    vm.pc += 1;
    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::helpers::make_test_instruction,
        either::Either,
        sbpf_common::{
            inst_param::{Number, Register},
            opcode::Opcode,
        },
    };

    #[test]
    fn test_lddw() {
        // lddw r1, 0x123456789ABCDEF0
        let program = vec![make_test_instruction(
            Opcode::Lddw,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(0x123456789ABCDEF0u64 as i64))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);

        let inst = vm.current_instruction().unwrap().clone();
        execute_lddw(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0x123456789ABCDEF0);
        assert_eq!(vm.pc, 1);
    }

    #[test]
    fn test_ldxb() {
        // ldxb r1, [r10-1]
        let program = vec![make_test_instruction(
            Opcode::Ldxb,
            Some(Register { n: 1 }),
            Some(Register { n: 10 }),
            Some(Either::Right(-1)),
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);

        // Write test value to stack at [r10-1]
        let addr = calculate_address(vm.registers[10], -1);
        vm.memory.write_u8(addr, 0x7).unwrap();

        let inst = vm.current_instruction().unwrap().clone();
        execute_ldxb(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0x7);
        assert_eq!(vm.pc, 1);
    }

    #[test]
    fn test_ldxh() {
        // ldxh r1, [r10-2]
        let program = vec![make_test_instruction(
            Opcode::Ldxh,
            Some(Register { n: 1 }),
            Some(Register { n: 10 }),
            Some(Either::Right(-2)),
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);

        // Write test value to stack at [r10-2]
        let addr = calculate_address(vm.registers[10], -2);
        vm.memory.write_u16(addr, 0xabcd).unwrap();

        let inst = vm.current_instruction().unwrap().clone();
        execute_ldxh(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0xabcd);
        assert_eq!(vm.pc, 1);
    }

    #[test]
    fn test_ldxw() {
        // ldxw r1, [r10-4]
        let program = vec![make_test_instruction(
            Opcode::Ldxw,
            Some(Register { n: 1 }),
            Some(Register { n: 10 }),
            Some(Either::Right(-4)),
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);

        // Write test value to stack at [r10-4]
        let addr = calculate_address(vm.registers[10], -4);
        vm.memory.write_u32(addr, 0xabcdef12).unwrap();

        let inst = vm.current_instruction().unwrap().clone();
        execute_ldxw(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0xabcdef12);
        assert_eq!(vm.pc, 1);
    }

    #[test]
    fn test_ldxdw() {
        // ldxdw r1, [r10-8]
        let program = vec![make_test_instruction(
            Opcode::Ldxdw,
            Some(Register { n: 1 }),
            Some(Register { n: 10 }),
            Some(Either::Right(-8)),
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);

        // Write test value to stack at [r10-8]
        let addr = calculate_address(vm.registers[10], -8);
        vm.memory.write_u64(addr, 0x123456789abcdef0).unwrap();

        let inst = vm.current_instruction().unwrap().clone();
        execute_ldxdw(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0x123456789abcdef0);
        assert_eq!(vm.pc, 1);
    }
}
