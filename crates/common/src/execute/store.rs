use {
    super::{ExecutionResult, Vm, helpers::*},
    crate::instruction::Instruction,
};

pub fn execute_stb(vm: &mut dyn Vm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let off = get_offset(inst)?;
    let imm = get_imm_i64(inst)? as u8;
    let addr = calculate_address(vm.get_register(dst), off);
    vm.write_u8(addr, imm)?;
    vm.advance_pc();
    Ok(())
}

pub fn execute_sth(vm: &mut dyn Vm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let off = get_offset(inst)?;
    let imm = get_imm_i64(inst)? as u16;
    let addr = calculate_address(vm.get_register(dst), off);
    vm.write_u16(addr, imm)?;
    vm.advance_pc();
    Ok(())
}

pub fn execute_stw(vm: &mut dyn Vm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let off = get_offset(inst)?;
    let imm = get_imm_i64(inst)? as u32;
    let addr = calculate_address(vm.get_register(dst), off);
    vm.write_u32(addr, imm)?;
    vm.advance_pc();
    Ok(())
}

pub fn execute_stdw(vm: &mut dyn Vm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let off = get_offset(inst)?;
    let imm = get_imm_i64(inst)? as u64;
    let addr = calculate_address(vm.get_register(dst), off);
    vm.write_u64(addr, imm)?;
    vm.advance_pc();
    Ok(())
}

pub fn execute_stxb(vm: &mut dyn Vm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.get_register(dst), off);
    vm.write_u8(addr, vm.get_register(src) as u8)?;
    vm.advance_pc();
    Ok(())
}

pub fn execute_stxh(vm: &mut dyn Vm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.get_register(dst), off);
    vm.write_u16(addr, vm.get_register(src) as u16)?;
    vm.advance_pc();
    Ok(())
}

pub fn execute_stxw(vm: &mut dyn Vm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.get_register(dst), off);
    vm.write_u32(addr, vm.get_register(src) as u32)?;
    vm.advance_pc();
    Ok(())
}

pub fn execute_stxdw(vm: &mut dyn Vm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_offset(inst)?;
    let addr = calculate_address(vm.get_register(dst), off);
    vm.write_u64(addr, vm.get_register(src))?;
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
    fn test_stb() {
        // stb [r2+0], 0x7
        let inst = make_test_instruction(
            Opcode::Stb,
            Some(Register { n: 2 }),
            None,
            Some(Either::Right(0)),
            Some(Either::Right(Number::Int(0x7))),
        );
        let mut vm = MockVm::new();
        vm.registers[2] = 0x100;

        execute_stb(&mut vm, &inst).unwrap();

        assert_eq!(vm.read_memory(0x100, 1), vec![0x7]);
        assert_eq!(vm.pc, 1);
    }

    #[test]
    fn test_sth() {
        // sth [r2+0], 0xabcd
        let inst = make_test_instruction(
            Opcode::Sth,
            Some(Register { n: 2 }),
            None,
            Some(Either::Right(0)),
            Some(Either::Right(Number::Int(0xabcd))),
        );
        let mut vm = MockVm::new();
        vm.registers[2] = 0x100;

        execute_sth(&mut vm, &inst).unwrap();

        assert_eq!(vm.read_u16(0x100).unwrap(), 0xabcd);
        assert_eq!(vm.pc, 1);
    }

    #[test]
    fn test_stw() {
        // stw [r2+0], 0xabcd123
        let inst = make_test_instruction(
            Opcode::Stw,
            Some(Register { n: 2 }),
            None,
            Some(Either::Right(0)),
            Some(Either::Right(Number::Int(0xabcd123))),
        );
        let mut vm = MockVm::new();
        vm.registers[2] = 0x100;

        execute_stw(&mut vm, &inst).unwrap();

        assert_eq!(vm.read_u32(0x100).unwrap(), 0xabcd123);
        assert_eq!(vm.pc, 1);
    }

    #[test]
    fn test_stdw() {
        // stdw [r2+0], 0x123456789abcdef0
        let inst = make_test_instruction(
            Opcode::Stdw,
            Some(Register { n: 2 }),
            None,
            Some(Either::Right(0)),
            Some(Either::Right(Number::Int(0x123456789abcdef0u64 as i64))),
        );
        let mut vm = MockVm::new();
        vm.registers[2] = 0x100;

        execute_stdw(&mut vm, &inst).unwrap();

        assert_eq!(vm.read_u64(0x100).unwrap(), 0x123456789abcdef0);
        assert_eq!(vm.pc, 1);
    }

    #[test]
    fn test_stxb() {
        // stxb [r2+0], r1
        let inst = make_test_instruction(
            Opcode::Stxb,
            Some(Register { n: 2 }),
            Some(Register { n: 1 }),
            Some(Either::Right(0)),
            None,
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 0x7;
        vm.registers[2] = 0x100;

        execute_stxb(&mut vm, &inst).unwrap();

        assert_eq!(vm.read_u8(0x100).unwrap(), 0x7);
        assert_eq!(vm.pc, 1);
    }

    #[test]
    fn test_stxh() {
        // stxh [r2+0], r1
        let inst = make_test_instruction(
            Opcode::Stxh,
            Some(Register { n: 2 }),
            Some(Register { n: 1 }),
            Some(Either::Right(0)),
            None,
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 0xabcd;
        vm.registers[2] = 0x100;

        execute_stxh(&mut vm, &inst).unwrap();

        assert_eq!(vm.read_u16(0x100).unwrap(), 0xabcd);
        assert_eq!(vm.pc, 1);
    }

    #[test]
    fn test_stxw() {
        // stxw [r2+0], r1
        let inst = make_test_instruction(
            Opcode::Stxw,
            Some(Register { n: 2 }),
            Some(Register { n: 1 }),
            Some(Either::Right(0)),
            None,
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 0xabcdef12;
        vm.registers[2] = 0x100;

        execute_stxw(&mut vm, &inst).unwrap();

        assert_eq!(vm.read_u32(0x100).unwrap(), 0xabcdef12);
        assert_eq!(vm.pc, 1);
    }

    #[test]
    fn test_stxdw() {
        // stxdw [r2+0], r1
        let inst = make_test_instruction(
            Opcode::Stxdw,
            Some(Register { n: 2 }),
            Some(Register { n: 1 }),
            Some(Either::Right(0)),
            None,
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 0x123456789abcdef0;
        vm.registers[2] = 0x100;

        execute_stxdw(&mut vm, &inst).unwrap();

        assert_eq!(vm.read_u64(0x100).unwrap(), 0x123456789abcdef0);
        assert_eq!(vm.pc, 1);
    }
}
