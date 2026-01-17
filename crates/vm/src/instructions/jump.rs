use {
    crate::{
        errors::{VmError, VmResult},
        helpers::{get_dst, get_imm_i64, get_src},
        vm::Vm,
    },
    sbpf_common::{instruction::Instruction, opcode::Opcode},
};

pub fn execute_jump(vm: &mut Vm, inst: &Instruction) -> VmResult<()> {
    match inst.opcode {
        // Unconditional jump
        Opcode::Ja => execute_ja(vm, inst),

        // Immediate conditional jumps
        Opcode::JeqImm => execute_jump_imm(vm, inst, |a, b| a == b),
        Opcode::JgtImm => execute_jump_imm(vm, inst, |a, b| a > b),
        Opcode::JgeImm => execute_jump_imm(vm, inst, |a, b| a >= b),
        Opcode::JltImm => execute_jump_imm(vm, inst, |a, b| a < b),
        Opcode::JleImm => execute_jump_imm(vm, inst, |a, b| a <= b),
        Opcode::JsetImm => execute_jump_imm(vm, inst, |a, b| (a & b) != 0),
        Opcode::JneImm => execute_jump_imm(vm, inst, |a, b| a != b),
        Opcode::JsgtImm => execute_jump_imm(vm, inst, |a, b| (a as i64) > (b as i64)),
        Opcode::JsgeImm => execute_jump_imm(vm, inst, |a, b| (a as i64) >= (b as i64)),
        Opcode::JsltImm => execute_jump_imm(vm, inst, |a, b| (a as i64) < (b as i64)),
        Opcode::JsleImm => execute_jump_imm(vm, inst, |a, b| (a as i64) <= (b as i64)),

        // Register conditional jumps
        Opcode::JeqReg => execute_jump_reg(vm, inst, |a, b| a == b),
        Opcode::JgtReg => execute_jump_reg(vm, inst, |a, b| a > b),
        Opcode::JgeReg => execute_jump_reg(vm, inst, |a, b| a >= b),
        Opcode::JltReg => execute_jump_reg(vm, inst, |a, b| a < b),
        Opcode::JleReg => execute_jump_reg(vm, inst, |a, b| a <= b),
        Opcode::JsetReg => execute_jump_reg(vm, inst, |a, b| (a & b) != 0),
        Opcode::JneReg => execute_jump_reg(vm, inst, |a, b| a != b),
        Opcode::JsgtReg => execute_jump_reg(vm, inst, |a, b| (a as i64) > (b as i64)),
        Opcode::JsgeReg => execute_jump_reg(vm, inst, |a, b| (a as i64) >= (b as i64)),
        Opcode::JsltReg => execute_jump_reg(vm, inst, |a, b| (a as i64) < (b as i64)),
        Opcode::JsleReg => execute_jump_reg(vm, inst, |a, b| (a as i64) <= (b as i64)),

        _ => Err(VmError::InvalidInstruction),
    }
}

fn get_jump_offset(inst: &Instruction) -> VmResult<i16> {
    match &inst.off {
        Some(either::Either::Right(off)) => Ok(*off),
        Some(either::Either::Left(_ident)) => Err(VmError::InvalidOperand), // Unresolved label
        None => Err(VmError::InvalidOperand),
    }
}

fn execute_jump_imm<F>(vm: &mut Vm, inst: &Instruction, condition: F) -> VmResult<()>
where
    F: FnOnce(u64, u64) -> bool,
{
    let dst = get_dst(inst)?;
    let off = get_jump_offset(inst)?;
    let imm = (get_imm_i64(inst)? as i32 as i64) as u64;

    if condition(vm.registers[dst], imm) {
        vm.pc = calculate_jump_target(vm.pc, off);
    } else {
        vm.pc += 1;
    }
    Ok(())
}

fn execute_jump_reg<F>(vm: &mut Vm, inst: &Instruction, condition: F) -> VmResult<()>
where
    F: FnOnce(u64, u64) -> bool,
{
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let off = get_jump_offset(inst)?;

    if condition(vm.registers[dst], vm.registers[src]) {
        vm.pc = calculate_jump_target(vm.pc, off);
    } else {
        vm.pc += 1;
    }
    Ok(())
}

fn execute_ja(vm: &mut Vm, inst: &Instruction) -> VmResult<()> {
    let off = get_jump_offset(inst)?;
    vm.pc = calculate_jump_target(vm.pc, off);
    Ok(())
}

fn calculate_jump_target(current_pc: usize, offset: i16) -> usize {
    ((current_pc as i64) + 1 + (offset as i64)) as usize
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::helpers::make_test_instruction,
        either::Either,
        sbpf_common::inst_param::{Number, Register},
    };

    #[test]
    fn test_ja_forward() {
        // ja +5
        let program = vec![make_test_instruction(
            Opcode::Ja,
            None,
            None,
            Some(Either::Right(5)),
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);

        let inst = vm.current_instruction().unwrap().clone();
        execute_ja(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 6);
    }

    #[test]
    fn test_ja_backward() {
        // ja -3
        let program =
            vec![make_test_instruction(Opcode::Ja, None, None, Some(Either::Right(-3)), None); 15];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.pc = 10;

        let inst = vm.current_instruction().unwrap().clone();
        execute_ja(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 8);
    }

    #[test]
    fn test_jeq_imm() {
        // jeq, r1, 5, +10
        let program = vec![make_test_instruction(
            Opcode::JeqImm,
            Some(Register { n: 1 }),
            None,
            Some(Either::Right(10)),
            Some(Either::Right(Number::Int(5))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 5;

        let inst = vm.current_instruction().unwrap().clone();
        execute_jump(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 11);
    }

    #[test]
    fn test_jgt_imm() {
        // jgt r1, 10, +3
        let program = vec![make_test_instruction(
            Opcode::JgtImm,
            Some(Register { n: 1 }),
            None,
            Some(Either::Right(3)),
            Some(Either::Right(Number::Int(10))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 11;

        let inst = vm.current_instruction().unwrap().clone();
        execute_jump(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 4);
    }

    #[test]
    fn test_jge_imm() {
        // jge r1, 10, +2
        let program = vec![make_test_instruction(
            Opcode::JgeImm,
            Some(Register { n: 1 }),
            None,
            Some(Either::Right(2)),
            Some(Either::Right(Number::Int(10))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 10;

        let inst = vm.current_instruction().unwrap().clone();
        execute_jump(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 3);
    }

    #[test]
    fn test_jlt_imm() {
        // jlt r1, 10, +3
        let program = vec![make_test_instruction(
            Opcode::JltImm,
            Some(Register { n: 1 }),
            None,
            Some(Either::Right(3)),
            Some(Either::Right(Number::Int(10))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 5;

        let inst = vm.current_instruction().unwrap().clone();
        execute_jump(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 4);
    }

    #[test]
    fn test_jle_imm() {
        // jle r1, 10, +4
        let program = vec![make_test_instruction(
            Opcode::JleImm,
            Some(Register { n: 0 }),
            None,
            Some(Either::Right(4)),
            Some(Either::Right(Number::Int(10))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 11;

        let inst = vm.current_instruction().unwrap().clone();
        execute_jump(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 5);
    }

    #[test]
    fn test_jset_imm() {
        // jset r1, 0x0f, +2
        let program = vec![make_test_instruction(
            Opcode::JsetImm,
            Some(Register { n: 1 }),
            None,
            Some(Either::Right(2)),
            Some(Either::Right(Number::Int(0x0f))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 0xff;

        let inst = vm.current_instruction().unwrap().clone();
        execute_jump(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 3);
    }
    #[test]
    fn test_jne_imm() {
        // jne r1, 10, +2
        let program = vec![make_test_instruction(
            Opcode::JneImm,
            Some(Register { n: 1 }),
            None,
            Some(Either::Right(2)),
            Some(Either::Right(Number::Int(10))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 9;

        let inst = vm.current_instruction().unwrap().clone();
        execute_jump(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 3);
    }

    #[test]
    fn test_jsgt_imm() {
        // jsgt r1, -10, +4
        let program = vec![make_test_instruction(
            Opcode::JsgtImm,
            Some(Register { n: 1 }),
            None,
            Some(Either::Right(4)),
            Some(Either::Right(Number::Int(-10))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 5;

        let inst = vm.current_instruction().unwrap().clone();
        execute_jump(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 5);
    }

    #[test]
    fn test_jsge_imm() {
        // jsge r1, -5, +1
        let program = vec![make_test_instruction(
            Opcode::JsgeImm,
            Some(Register { n: 1 }),
            None,
            Some(Either::Right(1)),
            Some(Either::Right(Number::Int(-5))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = (-5i64) as u64;

        let inst = vm.current_instruction().unwrap().clone();
        execute_jump(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 2);
    }

    #[test]
    fn test_jslt_imm() {
        // jslt r1, 10, +3
        let program = vec![make_test_instruction(
            Opcode::JsltImm,
            Some(Register { n: 1 }),
            None,
            Some(Either::Right(3)),
            Some(Either::Right(Number::Int(-4))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = (-5i64) as u64;

        let inst = vm.current_instruction().unwrap().clone();
        execute_jump(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 4);
    }

    #[test]
    fn test_jsle_imm() {
        // jsle r1, -5, +2
        let program = vec![make_test_instruction(
            Opcode::JsleImm,
            Some(Register { n: 1 }),
            None,
            Some(Either::Right(2)),
            Some(Either::Right(Number::Int(-5))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = (-10i64) as u64;

        let inst = vm.current_instruction().unwrap().clone();
        execute_jump(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 3);
    }

    #[test]
    fn test_jeq_reg() {
        // jeq r1, r2, +3
        let program = vec![make_test_instruction(
            Opcode::JeqReg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            Some(Either::Right(3)),
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 5;
        vm.registers[2] = 5;

        let inst = vm.current_instruction().unwrap().clone();
        execute_jump(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 4);
    }

    #[test]
    fn test_jgt_reg() {
        // jgt r1, r2, +5
        let program = vec![make_test_instruction(
            Opcode::JgtReg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            Some(Either::Right(5)),
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 100;
        vm.registers[2] = 50;

        let inst = vm.current_instruction().unwrap().clone();
        execute_jump(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 6);
    }

    #[test]
    fn test_jge_reg() {
        // jge r1, r2, +2
        let program = vec![make_test_instruction(
            Opcode::JgeReg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            Some(Either::Right(2)),
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 100;
        vm.registers[2] = 100;

        let inst = vm.current_instruction().unwrap().clone();
        execute_jump(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 3);
    }

    #[test]
    fn test_jlt_reg() {
        // jlt r1, r2, +4
        let program = vec![make_test_instruction(
            Opcode::JltReg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            Some(Either::Right(4)),
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 50;
        vm.registers[2] = 100;

        let inst = vm.current_instruction().unwrap().clone();
        execute_jump(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 5);
    }

    #[test]
    fn test_jle_reg() {
        // jle r1, r2, +3
        let program = vec![make_test_instruction(
            Opcode::JleReg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            Some(Either::Right(3)),
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 50;
        vm.registers[2] = 60;

        let inst = vm.current_instruction().unwrap().clone();
        execute_jump(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 4);
    }

    #[test]
    fn test_jset_reg() {
        // jset r1, r2, +2
        let program = vec![make_test_instruction(
            Opcode::JsetReg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            Some(Either::Right(2)),
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 0xFF;
        vm.registers[2] = 0x0F;

        let inst = vm.current_instruction().unwrap().clone();
        execute_jump(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 3);
    }

    #[test]
    fn test_jne_reg() {
        // jne r1, r2, +1
        let program = vec![make_test_instruction(
            Opcode::JneReg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            Some(Either::Right(1)),
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 10;
        vm.registers[2] = 20;

        let inst = vm.current_instruction().unwrap().clone();
        execute_jump(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 2);
    }

    #[test]
    fn test_jsgt_reg() {
        // jsgt r1, r2, +3
        let program = vec![make_test_instruction(
            Opcode::JsgtReg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            Some(Either::Right(3)),
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 5;
        vm.registers[2] = (-10i64) as u64;

        let inst = vm.current_instruction().unwrap().clone();
        execute_jump(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 4);
    }

    #[test]
    fn test_jsge_reg() {
        // jsge r1, r2, +2
        let program = vec![make_test_instruction(
            Opcode::JsgeReg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            Some(Either::Right(2)),
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = (-5i64) as u64;
        vm.registers[2] = (-6i64) as u64;

        let inst = vm.current_instruction().unwrap().clone();
        execute_jump(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 3);
    }

    #[test]
    fn test_jslt_reg() {
        // jslt r1, r2, +2
        let program = vec![make_test_instruction(
            Opcode::JsltReg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            Some(Either::Right(2)),
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = (-5i64) as u64;
        vm.registers[2] = 2;

        let inst = vm.current_instruction().unwrap().clone();
        execute_jump(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 3);
    }

    #[test]
    fn test_jsle_reg() {
        // jsle r0, r1, +2
        let program = vec![make_test_instruction(
            Opcode::JsleReg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            Some(Either::Right(2)),
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = (-10i64) as u64;
        vm.registers[2] = 5;

        let inst = vm.current_instruction().unwrap().clone();
        execute_jump(&mut vm, &inst).unwrap();

        assert_eq!(vm.pc, 3);
    }
}
