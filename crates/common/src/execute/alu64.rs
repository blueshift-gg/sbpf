use {
    super::{ExecutionResult, Vm, helpers::*},
    crate::{errors::ExecutionError, instruction::Instruction, opcode::Opcode},
};

pub fn execute_alu64_imm(vm: &mut dyn Vm, inst: &Instruction) -> ExecutionResult<()> {
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

pub fn execute_alu64_reg(vm: &mut dyn Vm, inst: &Instruction) -> ExecutionResult<()> {
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
        Opcode::Lsh64Reg => vm.set_register(dst, vm.get_register(dst).wrapping_shl(src_val as u32)),
        Opcode::Rsh64Reg => vm.set_register(dst, vm.get_register(dst).wrapping_shr(src_val as u32)),
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

pub fn execute_neg64(vm: &mut dyn Vm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    vm.set_register(dst, (vm.get_register(dst) as i64).wrapping_neg() as u64);
    vm.advance_pc();
    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            errors::ExecutionError,
            execute::{MockVm, make_test_instruction},
            inst_param::{Number, Register},
        },
        either::Either,
    };

    #[test]
    fn test_add64_imm() {
        // add64 r1, 10
        let inst = make_test_instruction(
            Opcode::Add64Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(10))),
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 5;

        execute_alu64_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 15);
    }

    #[test]
    fn test_sub64_imm() {
        // sub64 r1, 3
        let inst = make_test_instruction(
            Opcode::Sub64Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(3))),
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 10;

        execute_alu64_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 7);
    }

    #[test]
    fn test_mul64_imm() {
        // mul64 r1, 5
        let inst = make_test_instruction(
            Opcode::Mul64Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(5))),
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 6;

        execute_alu64_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 30);
    }

    #[test]
    fn test_div64_imm() {
        // div64 r1, 5
        let inst = make_test_instruction(
            Opcode::Div64Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(5))),
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 20;

        execute_alu64_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 4);
    }

    #[test]
    fn test_div64_imm_by_zero() {
        // div64 r1, 0
        let inst = make_test_instruction(
            Opcode::Div64Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(0))),
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 10;

        let result = execute_alu64_imm(&mut vm, &inst);

        assert!(matches!(result, Err(ExecutionError::DivisionByZero)));
    }

    #[test]
    fn test_or64_imm() {
        // or64 r1, 0x0f
        let inst = make_test_instruction(
            Opcode::Or64Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(0x0f))),
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 0xf0;

        execute_alu64_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0xff);
    }

    #[test]
    fn test_and64_imm() {
        // and64 r1, 0x0f
        let inst = make_test_instruction(
            Opcode::And64Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(0x0f))),
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 0xff;

        execute_alu64_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0x0f);
    }

    #[test]
    fn test_lsh64_imm() {
        // lsh64 r1, 4
        let inst = make_test_instruction(
            Opcode::Lsh64Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(4))),
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 0x1;

        execute_alu64_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0x10);
    }

    #[test]
    fn test_rsh64_imm() {
        // rsh64 r1, 4
        let inst = make_test_instruction(
            Opcode::Rsh64Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(4))),
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 0xf0;

        execute_alu64_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0x0f);
    }

    #[test]
    fn test_mod64_imm() {
        // mod64 r1, 7
        let inst = make_test_instruction(
            Opcode::Mod64Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(7))),
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 15;

        execute_alu64_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 1);
    }

    #[test]
    fn test_mod64_imm_by_zero() {
        // mod64 r1, 0
        let inst = make_test_instruction(
            Opcode::Mod64Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(0))),
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 15;

        let result = execute_alu64_imm(&mut vm, &inst);

        assert!(matches!(result, Err(ExecutionError::DivisionByZero)));
    }

    #[test]
    fn test_xor64_imm() {
        // xor64 r1, 0xff
        let inst = make_test_instruction(
            Opcode::Xor64Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(0xff))),
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 0xaa;

        execute_alu64_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0x55);
    }

    #[test]
    fn test_mov64_imm() {
        // mov64 r1, 10
        let inst = make_test_instruction(
            Opcode::Mov64Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(10))),
        );
        let mut vm = MockVm::new();

        execute_alu64_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 10);
    }

    #[test]
    fn test_arsh64_imm() {
        // arsh64 r1, 1
        let inst = make_test_instruction(
            Opcode::Arsh64Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(1))),
        );
        let mut vm = MockVm::new();
        vm.registers[1] = (-4i64) as u64;

        execute_alu64_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1] as i64, -2);
    }

    #[test]
    fn test_add64_reg() {
        // add64 r1, r2
        let inst = make_test_instruction(
            Opcode::Add64Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 10;
        vm.registers[2] = 5;

        execute_alu64_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 15);
    }

    #[test]
    fn test_sub64_reg() {
        // sub64 r1, r2
        let inst = make_test_instruction(
            Opcode::Sub64Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 20;
        vm.registers[2] = 8;

        execute_alu64_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 12);
    }

    #[test]
    fn test_mul64_reg() {
        // mul64 r1, r2
        let inst = make_test_instruction(
            Opcode::Mul64Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 10;
        vm.registers[2] = 2;

        execute_alu64_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 20);
    }

    #[test]
    fn test_div64_reg() {
        // div64 r1, r2
        let inst = make_test_instruction(
            Opcode::Div64Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 20;
        vm.registers[2] = 4;

        execute_alu64_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 5);
    }

    #[test]
    fn test_div64_reg_by_zero() {
        // div64 r1, r2 (r2 = 0)
        let inst = make_test_instruction(
            Opcode::Div64Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 20;
        vm.registers[2] = 0;

        let result = execute_alu64_reg(&mut vm, &inst);

        assert!(matches!(result, Err(ExecutionError::DivisionByZero)));
    }

    #[test]
    fn test_or64_reg() {
        // or64 r1, r2
        let inst = make_test_instruction(
            Opcode::Or64Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 0xf0;
        vm.registers[2] = 0x0f;

        execute_alu64_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0xff);
    }

    #[test]
    fn test_and64_reg() {
        // and64 r1, r2
        let inst = make_test_instruction(
            Opcode::And64Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 0xff;
        vm.registers[2] = 0x0f;

        execute_alu64_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0x0f);
    }

    #[test]
    fn test_lsh64_reg() {
        // lsh64 r1, r2
        let inst = make_test_instruction(
            Opcode::Lsh64Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 0x1;
        vm.registers[2] = 4;

        execute_alu64_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0x10);
    }

    #[test]
    fn test_rsh64_reg() {
        // rsh64 r1, r2
        let inst = make_test_instruction(
            Opcode::Rsh64Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 0xf0;
        vm.registers[2] = 4;

        execute_alu64_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0x0f);
    }

    #[test]
    fn test_mod64_reg() {
        // mod64 r1, r2
        let inst = make_test_instruction(
            Opcode::Mod64Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 17;
        vm.registers[2] = 5;

        execute_alu64_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 2);
    }

    #[test]
    fn test_mod64_reg_by_zero() {
        // mod64 r1, r2 (r2 = 0)
        let inst = make_test_instruction(
            Opcode::Mod64Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 17;
        vm.registers[2] = 0;

        let result = execute_alu64_reg(&mut vm, &inst);

        assert!(matches!(result, Err(ExecutionError::DivisionByZero)));
    }

    #[test]
    fn test_xor64_reg() {
        // xor64 r1, r2
        let inst = make_test_instruction(
            Opcode::Xor64Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        );
        let mut vm = MockVm::new();
        vm.registers[1] = 0xaa;
        vm.registers[2] = 0x55;

        execute_alu64_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0xff);
    }

    #[test]
    fn test_mov64_reg() {
        // mov64 r1, r2
        let inst = make_test_instruction(
            Opcode::Mov64Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        );
        let mut vm = MockVm::new();
        vm.registers[2] = 0x1234;

        execute_alu64_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0x1234);
    }

    #[test]
    fn test_arsh64_reg() {
        // arsh64 r1, r2
        let inst = make_test_instruction(
            Opcode::Arsh64Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        );
        let mut vm = MockVm::new();
        vm.registers[1] = (-8i64) as u64;
        vm.registers[2] = 2;

        execute_alu64_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1] as i64, -2);
    }

    #[test]
    fn test_neg64() {
        // neg64 r1
        let inst = make_test_instruction(Opcode::Neg64, Some(Register { n: 1 }), None, None, None);
        let mut vm = MockVm::new();
        vm.registers[1] = 10;

        execute_neg64(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1] as i64, -10);
    }
}
