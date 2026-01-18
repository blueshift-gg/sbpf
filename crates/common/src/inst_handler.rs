use crate::{
    decode::{
        decode_binary_immediate, decode_binary_register, decode_call_immediate,
        decode_call_register, decode_exit, decode_jump, decode_jump_immediate,
        decode_jump_register, decode_load_immediate, decode_load_memory, decode_store_immediate,
        decode_store_register, decode_unary,
    },
    errors::{ExecutionError, SBPFError},
    execute::{
        SbpfVm, execute_binary_immediate, execute_binary_register, execute_call_immediate,
        execute_call_register, execute_exit, execute_jump, execute_jump_immediate,
        execute_jump_register, execute_load_immediate, execute_load_memory,
        execute_store_immediate, execute_store_register, execute_unary,
    },
    instruction::Instruction,
    opcode::{
        BIN_IMM_OPS, BIN_REG_OPS, CALL_IMM_OPS, CALL_REG_OPS, EXIT_OPS, JUMP_IMM_OPS, JUMP_OPS,
        JUMP_REG_OPS, LOAD_IMM_OPS, LOAD_MEMORY_OPS, Opcode, OperationType, STORE_IMM_OPS,
        STORE_REG_OPS, UNARY_OPS,
    },
    validate::{
        validate_binary_immediate, validate_binary_register, validate_call_immediate,
        validate_call_register, validate_exit, validate_jump, validate_jump_immediate,
        validate_jump_register, validate_load_immediate, validate_load_memory,
        validate_store_immediate, validate_store_register, validate_unary,
    },
};

type DecodeFn = fn(&[u8]) -> Result<Instruction, SBPFError>;
type ValidateFn = fn(&Instruction) -> Result<(), SBPFError>;
pub type ExecuteFn = fn(&mut dyn SbpfVm, &Instruction) -> Result<(), ExecutionError>;

pub struct InstructionHandler {
    pub decode: DecodeFn,
    pub validate: ValidateFn,
    pub execute: ExecuteFn,
}

use {once_cell::sync::Lazy, std::collections::HashMap};

pub static OPCODE_TO_HANDLER: Lazy<HashMap<Opcode, InstructionHandler>> = Lazy::new(|| {
    //
    let mut map = HashMap::new();

    fn register_group(
        map: &mut HashMap<Opcode, InstructionHandler>,
        ops: &[Opcode],
        decode: DecodeFn,
        validate: ValidateFn,
        execute: ExecuteFn,
    ) {
        for &op in ops {
            map.insert(
                op,
                InstructionHandler {
                    decode,
                    validate,
                    execute,
                },
            );
        }
    }

    register_group(
        &mut map,
        LOAD_IMM_OPS,
        decode_load_immediate,
        validate_load_immediate,
        execute_load_immediate,
    );
    register_group(
        &mut map,
        LOAD_MEMORY_OPS,
        decode_load_memory,
        validate_load_memory,
        execute_load_memory,
    );
    register_group(
        &mut map,
        STORE_IMM_OPS,
        decode_store_immediate,
        validate_store_immediate,
        execute_store_immediate,
    );
    register_group(
        &mut map,
        STORE_REG_OPS,
        decode_store_register,
        validate_store_register,
        execute_store_register,
    );
    register_group(
        &mut map,
        BIN_IMM_OPS,
        decode_binary_immediate,
        validate_binary_immediate,
        execute_binary_immediate,
    );
    register_group(
        &mut map,
        BIN_REG_OPS,
        decode_binary_register,
        validate_binary_register,
        execute_binary_register,
    );
    register_group(
        &mut map,
        UNARY_OPS,
        decode_unary,
        validate_unary,
        execute_unary,
    );
    register_group(&mut map, JUMP_OPS, decode_jump, validate_jump, execute_jump);
    register_group(
        &mut map,
        JUMP_IMM_OPS,
        decode_jump_immediate,
        validate_jump_immediate,
        execute_jump_immediate,
    );
    register_group(
        &mut map,
        JUMP_REG_OPS,
        decode_jump_register,
        validate_jump_register,
        execute_jump_register,
    );
    register_group(
        &mut map,
        CALL_IMM_OPS,
        decode_call_immediate,
        validate_call_immediate,
        execute_call_immediate,
    );
    register_group(
        &mut map,
        CALL_REG_OPS,
        decode_call_register,
        validate_call_register,
        execute_call_register,
    );
    register_group(&mut map, EXIT_OPS, decode_exit, validate_exit, execute_exit);

    map
});

pub static OPCODE_TO_TYPE: Lazy<HashMap<Opcode, OperationType>> = Lazy::new(|| {
    let mut map = HashMap::new();

    fn register_group(
        map: &mut HashMap<Opcode, OperationType>,
        ops: &[Opcode],
        op_type: OperationType,
    ) {
        for &op in ops {
            map.insert(op, op_type);
        }
    }

    register_group(&mut map, LOAD_IMM_OPS, OperationType::LoadImmediate);
    register_group(&mut map, LOAD_MEMORY_OPS, OperationType::LoadMemory);
    register_group(&mut map, STORE_IMM_OPS, OperationType::StoreImmediate);
    register_group(&mut map, STORE_REG_OPS, OperationType::StoreRegister);
    register_group(&mut map, BIN_IMM_OPS, OperationType::BinaryImmediate);
    register_group(&mut map, BIN_REG_OPS, OperationType::BinaryRegister);
    register_group(&mut map, UNARY_OPS, OperationType::Unary);
    register_group(&mut map, JUMP_OPS, OperationType::Jump);
    register_group(&mut map, JUMP_IMM_OPS, OperationType::JumpImmediate);
    register_group(&mut map, JUMP_REG_OPS, OperationType::JumpRegister);
    register_group(&mut map, CALL_IMM_OPS, OperationType::CallImmediate);
    register_group(&mut map, CALL_REG_OPS, OperationType::CallRegister);
    register_group(&mut map, EXIT_OPS, OperationType::Exit);

    map
});
