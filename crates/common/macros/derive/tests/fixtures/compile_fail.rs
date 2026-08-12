use sbpf_common_derive::{OpcodeGroup, OpcodeTable};

#[path = "./handlers.rs"]
mod handlers;

use handlers::{DecodeFn, ExecuteFn, ValidateFn, stub_decode, stub_execute, stub_validate};

#[derive(Clone, Copy, OpcodeGroup)]
#[handlers(
    decode = DecodeFn,
    validate = ValidateFn,
    execute = ExecuteFn,
)]
pub enum Group {
    #[group(
        title = "A",
        description = "A",
        decode = stub_decode,
        validate = stub_validate,
        execute = stub_execute,
    )]
    A,
}

// Empty OpcodeTable
#[derive(Clone, Copy, OpcodeTable)]
pub enum OpcodeEmpty {}

// Empty OpcodeGroup
#[derive(Clone, Copy, OpcodeGroup)]
#[handlers(
    decode = DecodeFn,
    validate = ValidateFn,
    execute = ExecuteFn,
)]
pub enum GroupEmpty {}

// Missing `doc` field
#[derive(OpcodeTable)]
pub enum OpcodeMissingDoc {
    #[opcode(mnemonic = "x", code = 0x01, group = GroupOk::A)]
    X,
}

// `mnemonic` field specified more than once
#[derive(Clone, Copy, OpcodeTable)]
pub enum OpcodeRepeatedField {
    #[opcode(
        mnemonic = "x",
        mnemonic = "y",
        code = 0x01,
        group = Group::A,
        doc = "x",
    )]
    X,
}

// Duplicate code (0x01)
#[derive(OpcodeTable)]
pub enum OpcodeDuplicateCode {
    #[opcode(mnemonic = "a", code = 0x01, group = GroupOk::A, doc = "a")]
    A,
    #[opcode(mnemonic = "b", code = 0x01, group = GroupOk::A, doc = "b")]
    B,
}

// Missing #[handlers(...)] attribute
#[derive(Clone, Copy, OpcodeGroup)]
pub enum GroupMissingHandlers {
    #[group(
        title = "A",
        description = "A",
        decode = stub_decode,
        validate = stub_validate,
        execute = stub_execute,
    )]
    A,
}

// No decode, validate, and execute handlers
#[derive(OpcodeGroup)]
#[handlers(
    decode = DecodeFn,
    validate = ValidateFn,
    execute = ExecuteFn,
)]
pub enum GroupMissingFns {
    #[group(title = "A", description = "Group A")]
    A,
}

// Wrong handler signature (expected handlers::DecodeFn)
fn bad_decode(_: i32) {}

#[derive(Clone, Copy, OpcodeGroup)]
#[handlers(
    decode = DecodeFn,
    validate = ValidateFn,
    execute = ExecuteFn,
)]
pub enum GroupWrongHandlerSignature {
    #[group(
        title = "A",
        description = "A",
        decode = bad_decode,
        validate = stub_validate,
        execute = stub_execute,
    )]
    A,
}

// No OpcodeGroup trait implemented
#[derive(Clone, Copy)]
pub enum InvalidOpcodeGroup {
    A,
}

#[derive(Clone, Copy, OpcodeTable)]
pub enum OpcodeWrongGroup {
    #[opcode(
        mnemonic = "a",
        code = 0x01,
        group = InvalidOpcodeGroup::A,
        doc = "a",
    )]
    A,
}

fn main() {}
