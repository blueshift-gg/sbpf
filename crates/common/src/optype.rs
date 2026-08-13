use {
    crate::{
        errors::{ExecutionError, SBPFError},
        execute::Vm,
        instruction::Instruction,
    },
    sbpf_common_derive::OpcodeGroup,
};

pub type DecodeFn = fn(&[u8]) -> Result<Instruction, SBPFError>;
pub type ValidateFn = fn(&Instruction) -> Result<(), SBPFError>;
pub type ExecuteFn = fn(&mut dyn Vm, &Instruction) -> Result<(), ExecutionError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, OpcodeGroup)]
#[handlers(
    decode = DecodeFn,
    validate = ValidateFn,
    execute = ExecuteFn,
)]
pub enum OperationType {
    #[group(
        title = "Load Immediate Operation",
        description = "Load a 64-bit immediate value into a register.",
        decode = crate::decode::decode_load_immediate,
        validate = crate::validate::validate_load_immediate,
        execute = crate::execute::execute_load_immediate,
    )]
    LoadImmediate,
    #[group(
        title = "Load Memory Operations",
        description = "Load a value from memory into a register.",
        decode = crate::decode::decode_load_memory,
        validate = crate::validate::validate_load_memory,
        execute = crate::execute::execute_load_memory,
    )]
    LoadMemory,
    #[group(
        title = "Store Immediate Operations",
        description = "Store an immediate value to memory.",
        decode = crate::decode::decode_store_immediate,
        validate = crate::validate::validate_store_immediate,
        execute = crate::execute::execute_store_immediate,
    )]
    StoreImmediate,
    #[group(
        title = "Store Register Operations",
        description = "Store a register value to memory.",
        decode = crate::decode::decode_store_register,
        validate = crate::validate::validate_store_register,
        execute = crate::execute::execute_store_register,
    )]
    StoreRegister,
    #[group(
        title = "Binary Immediate Operations",
        description = "Perform a binary ALU operation using an immediate operand.",
        decode = crate::decode::decode_binary_immediate,
        validate = crate::validate::validate_binary_immediate,
        execute = crate::execute::execute_binary_immediate,
    )]
    BinaryImmediate,
    #[group(
        title = "Binary Register Operations",
        description = "Perform a binary ALU operation using a register operand.",
        decode = crate::decode::decode_binary_register,
        validate = crate::validate::validate_binary_register,
        execute = crate::execute::execute_binary_register,
    )]
    BinaryRegister,
    #[group(
        title = "Unary Operations",
        description = "Perform a unary ALU operation on a register operand.",
        decode = crate::decode::decode_unary,
        validate = crate::validate::validate_unary,
        execute = crate::execute::execute_unary,
    )]
    Unary,
    #[group(
        title = "Endian Operations",
        description = "Perform byte-order conversion on a register value.",
        decode = crate::decode::decode_endian,
        validate = crate::validate::validate_endian,
        execute = crate::execute::execute_endian,
    )]
    Endian,
    #[group(
        title = "Jump Operation",
        description = "Perform an unconditional jump to a target address.",
        decode = crate::decode::decode_jump,
        validate = crate::validate::validate_jump,
        execute = crate::execute::execute_jump,
    )]
    Jump,
    #[group(
        title = "Jump Immediate Operations",
        description = "Perform a conditional jump by comparing a register with an immediate.",
        decode = crate::decode::decode_jump_immediate,
        validate = crate::validate::validate_jump_immediate,
        execute = crate::execute::execute_jump_immediate,
    )]
    JumpImmediate,
    #[group(
        title = "Jump Register Operations",
        description = "Perform a conditional jump by comparing two registers.",
        decode = crate::decode::decode_jump_register,
        validate = crate::validate::validate_jump_register,
        execute = crate::execute::execute_jump_register,
    )]
    JumpRegister,
    #[group(
        title = "Jump32 Immediate Operations",
        description = "Perform a 32-bit conditional jump by comparing a register with an immediate.",
        decode = crate::decode::decode_jump32_immediate,
        validate = crate::validate::validate_jump_immediate,
        execute = crate::execute::execute_jump_immediate,
    )]
    Jump32Immediate,
    #[group(
        title = "Jump32 Register Operations",
        description = "Perform a 32-bit conditional jump by comparing two registers.",
        decode = crate::decode::decode_jump32_register,
        validate = crate::validate::validate_jump_register,
        execute = crate::execute::execute_jump_register,
    )]
    Jump32Register,
    #[group(
        title = "Call Immediate Operation",
        description = "Call a function using an immediate target address.",
        decode = crate::decode::decode_call_immediate,
        validate = crate::validate::validate_call_immediate,
        execute = crate::execute::execute_call_immediate,
    )]
    CallImmediate,
    #[group(
        title = "Call Register Operation",
        description = "Call a function using a target address stored in a register.",
        decode = crate::decode::decode_call_register,
        validate = crate::validate::validate_call_register,
        execute = crate::execute::execute_call_register,
    )]
    CallRegister,
    #[group(
        title = "Exit Operation",
        description = "Return from function or exit program.",
        decode = crate::decode::decode_exit,
        validate = crate::validate::validate_exit,
        execute = crate::execute::execute_exit,
    )]
    Exit,
}
