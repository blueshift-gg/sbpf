use {
    super::{ExecutionResult, Vm, helpers::*},
    crate::{errors::ExecutionError, instruction::Instruction, opcode::Opcode},
};

pub fn execute_endian(vm: &mut dyn Vm, inst: &Instruction) -> ExecutionResult<()> {
    let dst = get_dst(inst)?;
    let imm = get_imm_i64(inst)?;

    if imm != 16 && imm != 32 && imm != 64 {
        return Err(ExecutionError::InvalidOperand);
    }

    vm.set_register(
        dst,
        match inst.opcode {
            Opcode::Le => match imm {
                16 => (vm.get_register(dst) as u16).to_le() as u64,
                32 => (vm.get_register(dst) as u32).to_le() as u64,
                64 => vm.get_register(dst).to_le(),
                _ => unreachable!(),
            },
            Opcode::Be => match imm {
                16 => (vm.get_register(dst) as u16).to_be() as u64,
                32 => (vm.get_register(dst) as u32).to_be() as u64,
                64 => vm.get_register(dst).to_be(),
                _ => unreachable!(),
            },
            _ => return Err(ExecutionError::InvalidInstruction),
        },
    );

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
    fn test_le() {
        let test_cases = [
            (16, 0xFFFFFFFF_FFFF1234u64, 0x1234u64),
            (32, 0xFFFFFFFF_12345678u64, 0x12345678u64),
            (64, 0xFFFFFFFF_12345678u64, 0xFFFFFFFF_12345678u64),
        ];

        for (imm, input, expected) in test_cases {
            let inst = make_test_instruction(
                Opcode::Le,
                Some(Register { n: 0 }),
                None,
                None,
                Some(Either::Right(Number::Int(imm))),
            );
            let mut vm = MockVm::new();
            vm.registers[0] = input;

            execute_endian(&mut vm, &inst).unwrap();

            assert_eq!(
                vm.registers[0], expected,
                "le{} failed: input=0x{:X}, expected=0x{:X}, got=0x{:X}",
                imm, input, expected, vm.registers[0]
            );
        }
    }

    #[test]
    fn test_be() {
        let test_cases = [
            (16, 0x1234u64, 0x3412u64),
            (32, 0x12345678u64, 0x78563412u64),
            (64, 0x0123456789ABCDEFu64, 0xEFCDAB8967452301u64),
        ];

        for (imm, input, expected) in test_cases {
            let inst = make_test_instruction(
                Opcode::Be,
                Some(Register { n: 0 }),
                None,
                None,
                Some(Either::Right(Number::Int(imm))),
            );
            let mut vm = MockVm::new();
            vm.registers[0] = input;

            execute_endian(&mut vm, &inst).unwrap();

            assert_eq!(
                vm.registers[0], expected,
                "be{} failed: input=0x{:X}, expected=0x{:X}, got=0x{:X}",
                imm, input, expected, vm.registers[0]
            );
        }
    }

    #[test]
    fn test_invalid_imm() {
        let inst = make_test_instruction(
            Opcode::Le,
            Some(Register { n: 0 }),
            None,
            None,
            Some(Either::Right(Number::Int(8))),
        );
        let mut vm = MockVm::new();

        let result = execute_endian(&mut vm, &inst);

        assert!(matches!(result, Err(ExecutionError::InvalidOperand)));
    }
}
