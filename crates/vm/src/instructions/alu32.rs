use {
    crate::{
        errors::{VmError, VmResult},
        helpers::{get_dst, get_imm_i64, get_src},
        vm::Vm,
    },
    sbpf_common::{instruction::Instruction, opcode::Opcode},
};

pub fn execute_alu32_imm(vm: &mut Vm, inst: &Instruction) -> VmResult<()> {
    let dst = get_dst(inst)?;
    let imm = get_imm_i64(inst)?;

    match inst.opcode {
        Opcode::Add32Imm => {
            let result = (vm.registers[dst] as i32).wrapping_add(imm as i32);
            vm.registers[dst] = (result as i64) as u64;
        }
        Opcode::Sub32Imm => {
            let result = (vm.registers[dst] as i32).wrapping_sub(imm as i32);
            vm.registers[dst] = (result as i64) as u64;
        }
        Opcode::Mul32Imm => {
            let result = (vm.registers[dst] as i32).wrapping_mul(imm as i32);
            vm.registers[dst] = (result as i64) as u64;
        }
        Opcode::Div32Imm => {
            let imm_u32 = imm as u32;
            if imm_u32 == 0 {
                return Err(VmError::DivisionByZero);
            }
            let result = (vm.registers[dst] as u32) / imm_u32;
            vm.registers[dst] = result as u64;
        }
        Opcode::Or32Imm => {
            let result = (vm.registers[dst] as u32) | (imm as u32);
            vm.registers[dst] = result as u64;
        }
        Opcode::And32Imm => {
            let result = (vm.registers[dst] as u32) & (imm as u32);
            vm.registers[dst] = result as u64;
        }
        Opcode::Lsh32Imm => {
            let result = (vm.registers[dst] as u32).wrapping_shl(imm as u32);
            vm.registers[dst] = result as u64;
        }
        Opcode::Rsh32Imm => {
            let result = (vm.registers[dst] as u32).wrapping_shr(imm as u32);
            vm.registers[dst] = result as u64;
        }
        Opcode::Mod32Imm => {
            let imm_u32 = imm as u32;
            if imm_u32 == 0 {
                return Err(VmError::DivisionByZero);
            }
            let result = (vm.registers[dst] as u32) % imm_u32;
            vm.registers[dst] = result as u64;
        }
        Opcode::Xor32Imm => {
            let result = (vm.registers[dst] as u32) ^ (imm as u32);
            vm.registers[dst] = result as u64;
        }
        Opcode::Mov32Imm => {
            vm.registers[dst] = (imm as u32) as u64;
        }
        Opcode::Arsh32Imm => {
            let result = (vm.registers[dst] as i32).wrapping_shr(imm as u32) as u32;
            vm.registers[dst] = result as u64;
        }
        _ => return Err(VmError::InvalidInstruction),
    };

    vm.pc += 1;
    Ok(())
}

pub fn execute_alu32_reg(vm: &mut Vm, inst: &Instruction) -> VmResult<()> {
    let dst = get_dst(inst)?;
    let src = get_src(inst)?;
    let src_val = vm.registers[src] as i32;
    let dst_val = vm.registers[dst] as i32;

    match inst.opcode {
        Opcode::Add32Reg => {
            let result = dst_val.wrapping_add(src_val);
            vm.registers[dst] = (result as i64) as u64;
        }
        Opcode::Sub32Reg => {
            let result = dst_val.wrapping_sub(src_val);
            vm.registers[dst] = (result as i64) as u64;
        }
        Opcode::Mul32Reg => {
            let result = dst_val.wrapping_mul(src_val);
            vm.registers[dst] = (result as i64) as u64;
        }
        Opcode::Div32Reg => {
            let src_u32 = src_val as u32;
            let dst_u32 = dst_val as u32;
            if src_u32 == 0 {
                return Err(VmError::DivisionByZero);
            }
            let result = dst_u32 / src_u32;
            vm.registers[dst] = result as u64;
        }
        Opcode::Or32Reg => {
            let result = (dst_val as u32) | (src_val as u32);
            vm.registers[dst] = result as u64;
        }
        Opcode::And32Reg => {
            let result = (dst_val as u32) & (src_val as u32);
            vm.registers[dst] = result as u64;
        }
        Opcode::Lsh32Reg => {
            let result = (dst_val as u32).wrapping_shl(src_val as u32);
            vm.registers[dst] = result as u64;
        }
        Opcode::Rsh32Reg => {
            let result = (dst_val as u32).wrapping_shr(src_val as u32);
            vm.registers[dst] = result as u64;
        }
        Opcode::Mod32Reg => {
            let src_u32 = src_val as u32;
            let dst_u32 = dst_val as u32;
            if src_u32 == 0 {
                return Err(VmError::DivisionByZero);
            }
            let result = dst_u32 % src_u32;
            vm.registers[dst] = result as u64;
        }
        Opcode::Xor32Reg => {
            let result = (dst_val as u32) ^ (src_val as u32);
            vm.registers[dst] = result as u64;
        }
        Opcode::Mov32Reg => {
            vm.registers[dst] = (src_val as u32) as u64;
        }
        Opcode::Arsh32Reg => {
            let result = dst_val.wrapping_shr(src_val as u32) as u32;
            vm.registers[dst] = result as u64;
        }

        _ => return Err(VmError::InvalidInstruction),
    };

    vm.pc += 1;
    Ok(())
}

pub fn execute_neg32(vm: &mut Vm, inst: &Instruction) -> VmResult<()> {
    let dst = get_dst(inst)?;
    let result = (vm.registers[dst] as i32).wrapping_neg();
    vm.registers[dst] = result as u32 as u64;
    vm.pc += 1;
    Ok(())
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
    fn test_add32_imm() {
        // add32 r1, 10
        let program = vec![make_test_instruction(
            Opcode::Add32Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(10))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 5;

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 15);
    }

    #[test]
    fn test_sub32_imm() {
        // sub32 r1, 2
        let program = vec![make_test_instruction(
            Opcode::Sub32Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(2))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 5;

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 3);
    }

    #[test]
    fn test_mul32_imm() {
        // mul32 r1, 5
        let program = vec![make_test_instruction(
            Opcode::Mul32Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(5))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 10;

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 50);
    }

    #[test]
    fn test_div32_imm() {
        // div32 r1, 5
        let program = vec![make_test_instruction(
            Opcode::Div32Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(5))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 10;

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 2);
    }

    #[test]
    fn test_div32_imm_by_zero() {
        // div32 r1, 0
        let program = vec![make_test_instruction(
            Opcode::Div32Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(0))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 10;

        let inst = vm.current_instruction().unwrap().clone();
        let result = execute_alu32_imm(&mut vm, &inst);

        assert!(matches!(result, Err(VmError::DivisionByZero)));
    }

    #[test]
    fn test_or32_imm() {
        // or32 r1, 0x0f
        let program = vec![make_test_instruction(
            Opcode::Or32Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(0x0f))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 0xf0;

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0xff);
    }

    #[test]
    fn test_and32_imm() {
        // and32 r1, 0x0f
        let program = vec![make_test_instruction(
            Opcode::And32Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(0x0f))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 0xff;

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0x0f);
    }

    #[test]
    fn test_lsh32_imm() {
        // lsh32 r1, 4
        let program = vec![make_test_instruction(
            Opcode::Lsh32Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(4))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 0x1;

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0x10);
    }

    #[test]
    fn test_rsh32_imm() {
        // rsh32 r1, 4
        let program = vec![make_test_instruction(
            Opcode::Rsh32Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(4))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 0xf0;

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0x0f);
    }

    #[test]
    fn test_mod32_imm() {
        // mod32 r1, 7
        let program = vec![make_test_instruction(
            Opcode::Mod32Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(7))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 17;

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 3);
    }

    #[test]
    fn test_mod32_imm_by_zero() {
        // mod32 r1, 0
        let program = vec![make_test_instruction(
            Opcode::Mod32Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(0))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 17;

        let inst = vm.current_instruction().unwrap().clone();
        let result = execute_alu32_imm(&mut vm, &inst);

        assert!(matches!(result, Err(VmError::DivisionByZero)));
    }

    #[test]
    fn test_xor32_imm() {
        // xor32 r1, 0xff
        let program = vec![make_test_instruction(
            Opcode::Xor32Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(0xff))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 0xaa;

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0x55);
    }

    #[test]
    fn test_mov32_imm() {
        // mov32 r1, 10
        let program = vec![make_test_instruction(
            Opcode::Mov32Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(10))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 10);
    }

    #[test]
    fn test_arsh32_imm() {
        // arsh32 r1, 1
        let program = vec![make_test_instruction(
            Opcode::Arsh32Imm,
            Some(Register { n: 1 }),
            None,
            None,
            Some(Either::Right(Number::Int(1))),
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = (-4i32) as u32 as u64;

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_imm(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1] as i32, -2);
    }

    #[test]
    fn test_add32_reg() {
        // add32 r1, r2
        let program = vec![make_test_instruction(
            Opcode::Add32Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 10;
        vm.registers[2] = 5;

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 15);
    }

    #[test]
    fn test_sub32_reg() {
        // sub32 r1, r2
        let program = vec![make_test_instruction(
            Opcode::Sub32Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 10;
        vm.registers[2] = 5;

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 5);
    }

    #[test]
    fn test_mul32_reg() {
        // mul32 r1, r2
        let program = vec![make_test_instruction(
            Opcode::Mul32Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 10;
        vm.registers[2] = 5;

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 50);
    }

    #[test]
    fn test_div32_reg() {
        // div32 r1, r2
        let program = vec![make_test_instruction(
            Opcode::Div32Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 10;
        vm.registers[2] = 5;

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 2);
    }

    #[test]
    fn test_div32_reg_by_zero() {
        // div32 r1, r2 (r2 = 0)
        let program = vec![make_test_instruction(
            Opcode::Div32Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 10;
        vm.registers[2] = 0;

        let inst = vm.current_instruction().unwrap().clone();
        let result = execute_alu32_reg(&mut vm, &inst);

        assert!(matches!(result, Err(VmError::DivisionByZero)));
    }

    #[test]
    fn test_or32_reg() {
        // or32 r1, r2
        let program = vec![make_test_instruction(
            Opcode::Or32Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 0xf0;
        vm.registers[2] = 0x0f;

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0xff);
    }

    #[test]
    fn test_and32_reg() {
        // and32 r1, r2
        let program = vec![make_test_instruction(
            Opcode::And32Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 0xff;
        vm.registers[2] = 0x0f;

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0x0f);
    }

    #[test]
    fn test_lsh32_reg() {
        // lsh32 r1, r2
        let program = vec![make_test_instruction(
            Opcode::Lsh32Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 0x01;
        vm.registers[2] = 4;

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0x10);
    }

    #[test]
    fn test_rsh32_reg() {
        // rsh32 r1, r2
        let program = vec![make_test_instruction(
            Opcode::Rsh32Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 0xf0;
        vm.registers[2] = 4;

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0x0f);
    }

    #[test]
    fn test_mod32_reg() {
        // mod32 r1, r2
        let program = vec![make_test_instruction(
            Opcode::Mod32Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 17;
        vm.registers[2] = 5;

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 2);
    }

    #[test]
    fn test_mod32_reg_by_zero() {
        // mod32 r1, r2 (r2 = 0)
        let program = vec![make_test_instruction(
            Opcode::Mod32Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 17;
        vm.registers[2] = 0;

        let inst = vm.current_instruction().unwrap().clone();
        let result = execute_alu32_reg(&mut vm, &inst);

        assert!(matches!(result, Err(VmError::DivisionByZero)));
    }

    #[test]
    fn test_xor32_reg() {
        // xor32 r1, r2
        let program = vec![make_test_instruction(
            Opcode::Xor32Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 0xaa;
        vm.registers[2] = 0x55;

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0xff);
    }

    #[test]
    fn test_mov32_reg() {
        // mov32 r1, r2
        let program = vec![make_test_instruction(
            Opcode::Mov32Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[2] = 0x1234;

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1], 0x1234);
    }

    #[test]
    fn test_arsh32_reg() {
        // arsh32 r1, r2
        let program = vec![make_test_instruction(
            Opcode::Arsh32Reg,
            Some(Register { n: 1 }),
            Some(Register { n: 2 }),
            None,
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = (-8i32) as u32 as u64;
        vm.registers[2] = 2;

        let inst = vm.current_instruction().unwrap().clone();
        execute_alu32_reg(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1] as i32, -2);
    }

    #[test]
    fn test_neg32() {
        // neg32 r1
        let program = vec![make_test_instruction(
            Opcode::Neg32,
            Some(Register { n: 1 }),
            None,
            None,
            None,
        )];
        let mut vm = Vm::new(program, vec![], vec![]);
        vm.registers[1] = 10;

        let inst = vm.current_instruction().unwrap().clone();
        execute_neg32(&mut vm, &inst).unwrap();

        assert_eq!(vm.registers[1] as i32, -10);
    }
}
