use crate::decode::{
    decode_load_immediate,
    decode_load_memory,
    decode_store_immediate,
    decode_store_register,
    decode_binary_immediate,
    decode_binary_register,
    decode_unary,
    decode_jump,
    decode_jump_immediate,
    decode_jump_register,
    decode_call_immediate,
    decode_call_register,
    decode_exit,
};
use crate::errors::SBPFError;
use crate::instruction::Instruction;
use crate::opcode::{
    Opcode,
    OperationType,
    LOAD_IMM_OPS,
    LOAD_MEMORY_OPS,
    STORE_IMM_OPS,
    STORE_REG_OPS,
    BIN_IMM_OPS,
    BIN_REG_OPS,
    UNARY_OPS,
    JUMP_OPS,
    JUMP_IMM_OPS,
    JUMP_REG_OPS,
    CALL_IMM_OPS,
    CALL_REG_OPS,
    EXIT_OPS,
};

type DecodeFn = fn(&[u8]) -> Result<Instruction, SBPFError>;

pub struct InstructionHandler {
    pub decode: DecodeFn,
}

use once_cell::sync::Lazy;
use std::collections::HashMap;

pub static OPCODE_TO_HANDLER: 
    Lazy<HashMap<Opcode, InstructionHandler>> = Lazy::new(|| {
    //
    let mut map = HashMap::new();

    fn register_group(map: &mut HashMap<Opcode, InstructionHandler>
                    , ops: &[Opcode]
                    , decode: DecodeFn) {
        for &op in ops {
            map.insert(op, InstructionHandler { decode });
        }
    }
    
    register_group(&mut map, LOAD_IMM_OPS, decode_load_immediate);
    register_group(&mut map, LOAD_MEMORY_OPS, decode_load_memory);
    register_group(&mut map, STORE_IMM_OPS, decode_store_immediate);
    register_group(&mut map, STORE_REG_OPS, decode_store_register);
    register_group(&mut map, BIN_IMM_OPS, decode_binary_immediate);
    register_group(&mut map, BIN_REG_OPS, decode_binary_register);
    register_group(&mut map, UNARY_OPS, decode_unary);
    register_group(&mut map, JUMP_OPS, decode_jump);
    register_group(&mut map, JUMP_IMM_OPS, decode_jump_immediate);
    register_group(&mut map, JUMP_REG_OPS, decode_jump_register);
    register_group(&mut map, CALL_IMM_OPS, decode_call_immediate);
    register_group(&mut map, CALL_REG_OPS, decode_call_register);
    register_group(&mut map, EXIT_OPS, decode_exit);

    map
});

pub static OPCODE_TO_TYPE: Lazy<HashMap<Opcode, OperationType>> = Lazy::new(|| {
    let mut map = HashMap::new();

    fn register_group(map: &mut HashMap<Opcode, OperationType>
                    , ops: &[Opcode]
                    , op_type: OperationType) {
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
