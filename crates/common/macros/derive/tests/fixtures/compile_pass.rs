use sbpf_common_derive::{OpcodeGroup, OpcodeTable};
use sbpf_common_tablegen::{OpcodeGroup as _, OpcodeTable as _};
use std::str::FromStr;

#[path = "./handlers.rs"]
mod handlers;

use handlers::{
    DecodeFn, ExecuteFn, ValidateFn, stub_decode, stub_decode_b, stub_execute, stub_execute_b,
    stub_validate, stub_validate_b,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, OpcodeTable)]
pub enum Opcode {
    #[opcode(mnemonic = "x", code = 0x01, group = Group::A, doc = "op x")]
    X,

    #[opcode(mnemonic = "y", code = 0x02, group = Group::A, doc = "op y")]
    Y,

    #[opcode(
        mnemonic = "z",
        code = 0x03,
        group = Group::B,
        doc = "op z",
        arch = v3,
    )]
    Z,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, OpcodeGroup)]
#[handlers(
    decode = DecodeFn,
    validate = ValidateFn,
    execute = ExecuteFn,
)]
pub enum Group {
    #[group(
        title = "A",
        description = "Group A",
        decode = stub_decode,
        validate = stub_validate,
        execute = stub_execute,
    )]
    A,
    #[group(
        title = "B",
        description = "Group B",
        decode = stub_decode_b,
        validate = stub_validate_b,
        execute = stub_execute_b,
    )]
    B,
}

fn main() {
    // Check if OpcodeTable trait is implementated correctly.
    assert_eq!(Opcode::from_str("x").unwrap(), Opcode::X);
    assert_eq!(Opcode::from_str("y").unwrap(), Opcode::Y);
    assert_eq!(Opcode::from_str("z").unwrap(), Opcode::Z);
    assert!(Opcode::from_str("null").is_err());

    assert_eq!(Opcode::X.to_str(), "x");
    assert_eq!(Opcode::Y.to_str(), "y");
    assert_eq!(Opcode::Z.to_str(), "z");

    assert_eq!(format!("{}", Opcode::X), "x");
    assert_eq!(format!("{}", Opcode::Y), "y");
    assert_eq!(format!("{}", Opcode::Z), "z");

    assert_eq!(u8::from(Opcode::X), 0x01);
    assert_eq!(u8::from(Opcode::Y), 0x02);
    assert_eq!(u8::from(Opcode::Z), 0x03);

    assert_eq!(Opcode::try_from(0x01u8).unwrap(), Opcode::X);
    assert_eq!(Opcode::try_from(0x02u8).unwrap(), Opcode::Y);
    assert_eq!(Opcode::try_from_sbpf_v3(0x03).unwrap(), Opcode::Z);
    assert!(Opcode::try_from(0x03u8).is_err());
    assert!(Opcode::try_from(0x04u8).is_err());

    assert_eq!(Opcode::all().len(), 3);
    assert_eq!(Opcode::X.group(), Group::A);
    assert_eq!(Opcode::Y.group(), Group::A);
    assert_eq!(Opcode::Z.group(), Group::B);

    assert!(core::ptr::fn_addr_eq(
        Opcode::X.group().decode_fn(),
        stub_decode as DecodeFn
    ));
    assert!(core::ptr::fn_addr_eq(
        Opcode::Y.group().decode_fn(),
        stub_decode as DecodeFn
    ));
    assert!(core::ptr::fn_addr_eq(
        Opcode::Z.group().decode_fn(),
        stub_decode_b as DecodeFn
    ));
    assert!(core::ptr::fn_addr_eq(
        Opcode::X.group().validate_fn(),
        stub_validate as ValidateFn
    ));
    assert!(core::ptr::fn_addr_eq(
        Opcode::Y.group().validate_fn(),
        stub_validate as ValidateFn
    ));
    assert!(core::ptr::fn_addr_eq(
        Opcode::Z.group().validate_fn(),
        stub_validate_b as ValidateFn
    ));
    assert!(core::ptr::fn_addr_eq(
        Opcode::X.group().execute_fn(),
        stub_execute as ExecuteFn
    ));
    assert!(core::ptr::fn_addr_eq(
        Opcode::Y.group().execute_fn(),
        stub_execute as ExecuteFn
    ));
    assert!(core::ptr::fn_addr_eq(
        Opcode::Z.group().execute_fn(),
        stub_execute_b as ExecuteFn
    ));

    // Check if OpcodeGroup trait is implementated correctly.
    assert_eq!(Group::A.title(), "A");
    assert_eq!(Group::A.description(), "Group A");
    assert_eq!(Group::B.title(), "B");
    assert_eq!(Group::B.description(), "Group B");
    assert_eq!(Group::all().len(), 2);

    assert!(core::ptr::fn_addr_eq(
        Group::A.decode_fn(),
        stub_decode as DecodeFn
    ));
    assert!(core::ptr::fn_addr_eq(
        Group::A.validate_fn(),
        stub_validate as ValidateFn
    ));
    assert!(core::ptr::fn_addr_eq(
        Group::A.execute_fn(),
        stub_execute as ExecuteFn
    ));
    assert!(core::ptr::fn_addr_eq(
        Group::B.decode_fn(),
        stub_decode_b as DecodeFn
    ));
    assert!(core::ptr::fn_addr_eq(
        Group::B.validate_fn(),
        stub_validate_b as ValidateFn
    ));
    assert!(core::ptr::fn_addr_eq(
        Group::B.execute_fn(),
        stub_execute_b as ExecuteFn
    ));
}
