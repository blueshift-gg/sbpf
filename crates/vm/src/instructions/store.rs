use {
    crate::{
        errors::VmResult,
        helpers::{calculate_address, get_dst, get_imm_i64, get_offset, get_src},
        vm::Vm,
    },
    sbpf_common::instruction::Instruction,
};

pub fn execute_stb(vm: &mut Vm, inst: &Instruction) -> VmResult<()> {
    let dst = get_dst(inst)?;
    let off = get_offset(inst)?;
    let imm = get_imm_i64(inst)? as u8;
    let addr = calculate_address(vm.registers[dst], off);

    vm.memory.write_u8(addr, imm)?;
    vm.pc += 1;
    Ok(())
}

pub fn execute_sth(vm: &mut Vm, inst: &Instruction) -> VmResult<()> {
    let dst = get_dst(inst)?;
    let off = get_offset(inst)?;
    let imm = get_imm_i64(inst)? as u16;
    let addr = calculate_address(vm.registers[dst], off);

    vm.memory.write_u16(addr, imm)?;
    vm.pc += 1;
    Ok(())
}

pub fn execute_stw(vm: &mut Vm, inst: &Instruction) -> VmResult<()> {
    let dst = get_dst(inst)?;
    let off = get_offset(inst)?;
    let imm = get_imm_i64(inst)? as u32;
    let addr = calculate_address(vm.registers[dst], off);

    vm.memory.write_u32(addr, imm)?;
    vm.pc += 1;
    Ok(())
}

pub fn execute_stdw(vm: &mut Vm, inst: &Instruction) -> VmResult<()> {
    let dst = get_dst(inst)?;
    let off = get_offset(inst)?;
    let imm = get_imm_i64(inst)? as u64;
    let addr = calculate_address(vm.registers[dst], off);

    vm.memory.write_u64(addr, imm)?;
    vm.pc += 1;
    Ok(())
}

pub fn execute_stxb(vm: &mut Vm, inst: &Instruction) -> VmResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.registers[dst], off);

    vm.memory.write_u8(addr, vm.registers[src] as u8)?;
    vm.pc += 1;
    Ok(())
}

pub fn execute_stxh(vm: &mut Vm, inst: &Instruction) -> VmResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.registers[dst], off);

    vm.memory.write_u16(addr, vm.registers[src] as u16)?;
    vm.pc += 1;
    Ok(())
}

pub fn execute_stxw(vm: &mut Vm, inst: &Instruction) -> VmResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.registers[dst], off);

    vm.memory.write_u32(addr, vm.registers[src] as u32)?;
    vm.pc += 1;
    Ok(())
}

pub fn execute_stxdw(vm: &mut Vm, inst: &Instruction) -> VmResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.registers[dst], off);

    vm.memory.write_u64(addr, vm.registers[src])?;
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
    fn test_stb() {
        // stb [r10-1], 0x7
        let program = vec![make_test_instruction(
            Opcode::Stb,
            Some(Register { n: 10 }),
            None,
            Some(Either::Right(-1)),
            Some(Either::Right(Number::Int(0x7))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);

        let inst = vm.current_instruction().unwrap().clone();
        execute_stb(&mut vm, &inst).unwrap();

        let addr = calculate_address(vm.registers[10], -1);
        assert_eq!(vm.memory.read_u8(addr).unwrap(), 0x7);
        assert_eq!(vm.pc, 1);
    }

    #[test]
    fn test_sth() {
        // sth [r10-2], 0xabcd
        let program = vec![make_test_instruction(
            Opcode::Sth,
            Some(Register { n: 10 }),
            None,
            Some(Either::Right(-2)),
            Some(Either::Right(Number::Int(0xabcd))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);

        let inst = vm.current_instruction().unwrap().clone();
        execute_sth(&mut vm, &inst).unwrap();

        let addr = calculate_address(vm.registers[10], -2);
        assert_eq!(vm.memory.read_u16(addr).unwrap(), 0xabcd);
        assert_eq!(vm.pc, 1);
    }

    #[test]
    fn test_stw() {
        // stw [r10-4], 0xabcd123
        let program = vec![make_test_instruction(
            Opcode::Stw,
            Some(Register { n: 10 }),
            None,
            Some(Either::Right(-4)),
            Some(Either::Right(Number::Int(0xabcd123))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);

        let inst = vm.current_instruction().unwrap().clone();
        execute_stw(&mut vm, &inst).unwrap();

        let addr = calculate_address(vm.registers[10], -4);
        assert_eq!(vm.memory.read_u32(addr).unwrap(), 0xabcd123);
        assert_eq!(vm.pc, 1);
    }

    #[test]
    fn test_stdw() {
        // stdw [r10-8], 0x123456789abcdef0
        let program = vec![make_test_instruction(
            Opcode::Stdw,
            Some(Register { n: 10 }),
            None,
            Some(Either::Right(-8)),
            Some(Either::Right(Number::Int(0x123456789abcdef0 as i64))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);

        let inst = vm.current_instruction().unwrap().clone();
        execute_stdw(&mut vm, &inst).unwrap();

        let addr = calculate_address(vm.registers[10], -8);
        assert_eq!(vm.memory.read_u64(addr).unwrap(), 0x123456789abcdef0);
        assert_eq!(vm.pc, 1);
    }

    #[test]
    fn test_stxb() {
        // stxb [r10-1], r2
        let program = vec![make_test_instruction(
            Opcode::Stxb,
            Some(Register { n: 10 }),
            Some(Register { n: 2 }),
            Some(Either::Right(-1)),
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[2] = 0x7;

        let inst = vm.current_instruction().unwrap().clone();
        execute_stxb(&mut vm, &inst).unwrap();

        let addr = calculate_address(vm.registers[10], -1);
        assert_eq!(vm.memory.read_u8(addr).unwrap(), 0x7);
        assert_eq!(vm.pc, 1);
    }

    #[test]
    fn test_stxh() {
        // stxh [r10-2], r2
        let program = vec![make_test_instruction(
            Opcode::Stxh,
            Some(Register { n: 10 }),
            Some(Register { n: 2 }),
            Some(Either::Right(-2)),
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[2] = 0xabcd;

        let inst = vm.current_instruction().unwrap().clone();
        execute_stxh(&mut vm, &inst).unwrap();

        let addr = calculate_address(vm.registers[10], -2);
        assert_eq!(vm.memory.read_u16(addr).unwrap(), 0xabcd);
        assert_eq!(vm.pc, 1);
    }

    #[test]
    fn test_stxw() {
        // stxw [r10-4], r2
        let program = vec![make_test_instruction(
            Opcode::Stxw,
            Some(Register { n: 10 }),
            Some(Register { n: 2 }),
            Some(Either::Right(-4)),
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[2] = 0xabcdef12;

        let inst = vm.current_instruction().unwrap().clone();
        execute_stxw(&mut vm, &inst).unwrap();

        let addr = calculate_address(vm.registers[10], -4);
        assert_eq!(vm.memory.read_u32(addr).unwrap(), 0xabcdef12);
        assert_eq!(vm.pc, 1);
    }

    #[test]
    fn test_stxdw() {
        // stxdw [r10-8], r2
        let program = vec![make_test_instruction(
            Opcode::Stxdw,
            Some(Register { n: 10 }),
            Some(Register { n: 2 }),
            Some(Either::Right(-8)),
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[2] = 0x123456789abcdef0;

        let inst = vm.current_instruction().unwrap().clone();
        execute_stxdw(&mut vm, &inst).unwrap();

        let addr = calculate_address(vm.registers[10], -8);
        assert_eq!(vm.memory.read_u64(addr).unwrap(), 0x123456789abcdef0);
        assert_eq!(vm.pc, 1);
    }
}
