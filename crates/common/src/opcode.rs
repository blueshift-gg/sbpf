use {
    crate::optype::OperationType,
    num_derive::FromPrimitive,
    sbpf_common_derive::OpcodeTable,
    serde::{Deserialize, Serialize},
};

#[derive(
    Debug, Clone, Copy, Hash, Eq, PartialEq, FromPrimitive, Serialize, Deserialize, OpcodeTable,
)]
pub enum Opcode {
    #[opcode(
        mnemonic = "lddw",
        code = 0x18,
        group = OperationType::LoadImmediate,
        doc = "lddw dst, imm",
    )]
    Lddw,
    #[opcode(
        mnemonic = "ldxb",
        code = 0x71,
        group = OperationType::LoadMemory,
        doc = "ldxb dst, [src + off]",
        size = "u8",
    )]
    Ldxb,
    #[opcode(
        mnemonic = "ldxh",
        code = 0x69,
        group = OperationType::LoadMemory,
        doc = "ldxh dst, [src + off]",
        size = "u16",
    )]
    Ldxh,
    #[opcode(
        mnemonic = "ldxw",
        code = 0x61,
        group = OperationType::LoadMemory,
        doc = "ldxw dst, [src + off]",
        size = "u32",
    )]
    Ldxw,
    #[opcode(
        mnemonic = "ldxdw",
        code = 0x79,
        group = OperationType::LoadMemory,
        doc = "ldxdw dst, [src + off]",
        size = "u64",
    )]
    Ldxdw,
    #[opcode(
        mnemonic = "stb",
        code = 0x72,
        group = OperationType::StoreImmediate,
        doc = "stb [dst + off], imm",
        size = "u8",
    )]
    Stb,
    #[opcode(
        mnemonic = "sth",
        code = 0x6a,
        group = OperationType::StoreImmediate,
        doc = "sth [dst + off], imm",
        size = "u16",
    )]
    Sth,
    #[opcode(
        mnemonic = "stw",
        code = 0x62,
        group = OperationType::StoreImmediate,
        doc = "stw [dst + off], imm",
        size = "u32",
    )]
    Stw,
    #[opcode(
        mnemonic = "stdw",
        code = 0x7a,
        group = OperationType::StoreImmediate,
        doc = "stdw [dst + off], imm",
        size = "u64",
    )]
    Stdw,
    #[opcode(
        mnemonic = "stxb",
        code = 0x73,
        group = OperationType::StoreRegister,
        doc = "stxb [dst + off], src",
        size = "u8",
    )]
    Stxb,
    #[opcode(
        mnemonic = "stxh",
        code = 0x6b,
        group = OperationType::StoreRegister,
        doc = "stxh [dst + off], src",
        size = "u16",
    )]
    Stxh,
    #[opcode(
        mnemonic = "stxw",
        code = 0x63,
        group = OperationType::StoreRegister,
        doc = "stxw [dst + off], src",
        size = "u32",
    )]
    Stxw,
    #[opcode(
        mnemonic = "stxdw",
        code = 0x7b,
        group = OperationType::StoreRegister,
        doc = "stxdw [dst + off], src",
        size = "u64",
    )]
    Stxdw,
    #[opcode(
        mnemonic = "add32",
        code = 0x04,
        group = OperationType::BinaryImmediate,
        doc = "add32 dst, imm",
        operator = "+=",
    )]
    Add32Imm,
    #[opcode(
        mnemonic = "add32",
        code = 0x0c,
        group = OperationType::BinaryRegister,
        doc = "add32 dst, src",
        operator = "+=",
    )]
    Add32Reg,
    #[opcode(
        mnemonic = "sub32",
        code = 0x14,
        group = OperationType::BinaryImmediate,
        doc = "sub32 dst, imm",
        operator = "-=",
    )]
    Sub32Imm,
    #[opcode(
        mnemonic = "sub32",
        code = 0x1c,
        group = OperationType::BinaryRegister,
        doc = "sub32 dst, src",
        operator = "-=",
    )]
    Sub32Reg,
    #[opcode(
        mnemonic = "mul32",
        code = 0x24,
        group = OperationType::BinaryImmediate,
        doc = "mul32 dst, imm",
        operator = "*=",
    )]
    Mul32Imm,
    #[opcode(
        mnemonic = "mul32",
        code = 0x2c,
        group = OperationType::BinaryRegister,
        doc = "mul32 dst, src",
        operator = "*=",
    )]
    Mul32Reg,
    #[opcode(
        mnemonic = "div32",
        code = 0x34,
        group = OperationType::BinaryImmediate,
        doc = "div32 dst, imm",
        operator = "/=",
    )]
    Div32Imm,
    #[opcode(
        mnemonic = "div32",
        code = 0x3c,
        group = OperationType::BinaryRegister,
        doc = "div32 dst, src",
        operator = "/=",
    )]
    Div32Reg,
    #[opcode(
        mnemonic = "or32",
        code = 0x44,
        group = OperationType::BinaryImmediate,
        doc = "or32 dst, imm",
        operator = "|=",
    )]
    Or32Imm,
    #[opcode(
        mnemonic = "or32",
        code = 0x4c,
        group = OperationType::BinaryRegister,
        doc = "or32 dst, src",
        operator = "|=",
    )]
    Or32Reg,
    #[opcode(
        mnemonic = "and32",
        code = 0x54,
        group = OperationType::BinaryImmediate,
        doc = "and32 dst, imm",
        operator = "&=",
    )]
    And32Imm,
    #[opcode(
        mnemonic = "and32",
        code = 0x5c,
        group = OperationType::BinaryRegister,
        doc = "and32 dst, src",
        operator = "&=",
    )]
    And32Reg,
    #[opcode(
        mnemonic = "lsh32",
        code = 0x64,
        group = OperationType::BinaryImmediate,
        doc = "lsh32 dst, imm",
        operator = "<<=",
    )]
    Lsh32Imm,
    #[opcode(
        mnemonic = "lsh32",
        code = 0x6c,
        group = OperationType::BinaryRegister,
        doc = "lsh32 dst, src",
        operator = "<<=",
    )]
    Lsh32Reg,
    #[opcode(
        mnemonic = "rsh32",
        code = 0x74,
        group = OperationType::BinaryImmediate,
        doc = "rsh32 dst, imm",
        operator = ">>=",
    )]
    Rsh32Imm,
    #[opcode(
        mnemonic = "rsh32",
        code = 0x7c,
        group = OperationType::BinaryRegister,
        doc = "rsh32 dst, src",
        operator = ">>=",
    )]
    Rsh32Reg,
    #[opcode(
        mnemonic = "mod32",
        code = 0x94,
        group = OperationType::BinaryImmediate,
        doc = "mod32 dst, imm",
        operator = "%=",
    )]
    Mod32Imm,
    #[opcode(
        mnemonic = "mod32",
        code = 0x9c,
        group = OperationType::BinaryRegister,
        doc = "mod32 dst, src",
        operator = "%=",
    )]
    Mod32Reg,
    #[opcode(
        mnemonic = "xor32",
        code = 0xa4,
        group = OperationType::BinaryImmediate,
        doc = "xor32 dst, imm",
        operator = "^=",
    )]
    Xor32Imm,
    #[opcode(
        mnemonic = "xor32",
        code = 0xac,
        group = OperationType::BinaryRegister,
        doc = "xor32 dst, src",
        operator = "^=",
    )]
    Xor32Reg,
    #[opcode(
        mnemonic = "mov32",
        code = 0xb4,
        group = OperationType::BinaryImmediate,
        doc = "mov32 dst, imm",
        operator = "=",
    )]
    Mov32Imm,
    #[opcode(
        mnemonic = "mov32",
        code = 0xbc,
        group = OperationType::BinaryRegister,
        doc = "mov32 dst, src",
        operator = "=",
    )]
    Mov32Reg,
    #[opcode(
        mnemonic = "arsh32",
        code = 0xc4,
        group = OperationType::BinaryImmediate,
        doc = "arsh32 dst, imm",
        operator = "s>>=",
    )]
    Arsh32Imm,
    #[opcode(
        mnemonic = "arsh32",
        code = 0xcc,
        group = OperationType::BinaryRegister,
        doc = "arsh32 dst, src",
        operator = "s>>=",
    )]
    Arsh32Reg,
    #[opcode(
        mnemonic = "lmul32",
        code = 0x86,
        group = OperationType::BinaryImmediate,
        doc = "lmul32 dst, imm",
        arch = v2,
    )]
    Lmul32Imm,
    #[opcode(
        mnemonic = "lmul32",
        code = 0x8e,
        group = OperationType::BinaryRegister,
        doc = "lmul32 dst, src",
        arch = v2,
    )]
    Lmul32Reg,
    #[opcode(
        mnemonic = "udiv32",
        code = 0x46,
        group = OperationType::BinaryImmediate,
        doc = "udiv32 dst, imm",
        arch = v2,
    )]
    Udiv32Imm,
    #[opcode(
        mnemonic = "udiv32",
        code = 0x4e,
        group = OperationType::BinaryRegister,
        doc = "udiv32 dst, src",
        arch = v2,
    )]
    Udiv32Reg,
    #[opcode(
        mnemonic = "urem32",
        code = 0x66,
        group = OperationType::BinaryImmediate,
        doc = "urem32 dst, imm",
        arch = v2,
    )]
    Urem32Imm,
    #[opcode(
        mnemonic = "urem32",
        code = 0x6e,
        group = OperationType::BinaryRegister,
        doc = "urem32 dst, src",
        arch = v2,
    )]
    Urem32Reg,
    #[opcode(
        mnemonic = "sdiv32",
        code = 0xc6,
        group = OperationType::BinaryImmediate,
        doc = "sdiv32 dst, imm",
        arch = v2,
    )]
    Sdiv32Imm,
    #[opcode(
        mnemonic = "sdiv32",
        code = 0xce,
        group = OperationType::BinaryRegister,
        doc = "sdiv32 dst, src",
        arch = v2,
    )]
    Sdiv32Reg,
    #[opcode(
        mnemonic = "srem32",
        code = 0xe6,
        group = OperationType::BinaryImmediate,
        doc = "srem32 dst, imm",
        arch = v2,
    )]
    Srem32Imm,
    #[opcode(
        mnemonic = "srem32",
        code = 0xee,
        group = OperationType::BinaryRegister,
        doc = "srem32 dst, src",
        arch = v2,
    )]
    Srem32Reg,
    #[opcode(
        mnemonic = "le",
        code = 0xd4,
        group = OperationType::Endian,
        doc = "le16 dst / le32 dst / le64 dst",
    )]
    Le,
    #[opcode(
        mnemonic = "be",
        code = 0xdc,
        group = OperationType::Endian,
        doc = "be16 dst / be32 dst / be64 dst",
    )]
    Be,
    #[opcode(
        mnemonic = "add64",
        code = 0x07,
        group = OperationType::BinaryImmediate,
        doc = "add64 dst, imm",
        operator = "+=",
    )]
    Add64Imm,
    #[opcode(
        mnemonic = "add64",
        code = 0x0f,
        group = OperationType::BinaryRegister,
        doc = "add64 dst, src",
        operator = "+=",
    )]
    Add64Reg,
    #[opcode(
        mnemonic = "sub64",
        code = 0x17,
        group = OperationType::BinaryImmediate,
        doc = "sub64 dst, imm",
        operator = "-=",
    )]
    Sub64Imm,
    #[opcode(
        mnemonic = "sub64",
        code = 0x1f,
        group = OperationType::BinaryRegister,
        doc = "sub64 dst, src",
        operator = "-=",
    )]
    Sub64Reg,
    #[opcode(
        mnemonic = "mul64",
        code = 0x27,
        group = OperationType::BinaryImmediate,
        doc = "mul64 dst, imm",
        operator = "*=",
    )]
    Mul64Imm,
    #[opcode(
        mnemonic = "mul64",
        code = 0x2f,
        group = OperationType::BinaryRegister,
        doc = "mul64 dst, src",
        operator = "*=",
    )]
    Mul64Reg,
    #[opcode(
        mnemonic = "div64",
        code = 0x37,
        group = OperationType::BinaryImmediate,
        doc = "div64 dst, imm",
        operator = "/=",
    )]
    Div64Imm,
    #[opcode(
        mnemonic = "div64",
        code = 0x3f,
        group = OperationType::BinaryRegister,
        doc = "div64 dst, src",
        operator = "/=",
    )]
    Div64Reg,
    #[opcode(
        mnemonic = "or64",
        code = 0x47,
        group = OperationType::BinaryImmediate,
        doc = "or64 dst, imm",
        operator = "|=",
    )]
    Or64Imm,
    #[opcode(
        mnemonic = "or64",
        code = 0x4f,
        group = OperationType::BinaryRegister,
        doc = "or64 dst, src",
        operator = "|=",
    )]
    Or64Reg,
    #[opcode(
        mnemonic = "and64",
        code = 0x57,
        group = OperationType::BinaryImmediate,
        doc = "and64 dst, imm",
        operator = "&=",
    )]
    And64Imm,
    #[opcode(
        mnemonic = "and64",
        code = 0x5f,
        group = OperationType::BinaryRegister,
        doc = "and64 dst, src",
        operator = "&=",
    )]
    And64Reg,
    #[opcode(
        mnemonic = "lsh64",
        code = 0x67,
        group = OperationType::BinaryImmediate,
        doc = "lsh64 dst, imm",
        operator = "<<=",
    )]
    Lsh64Imm,
    #[opcode(
        mnemonic = "lsh64",
        code = 0x6f,
        group = OperationType::BinaryRegister,
        doc = "lsh64 dst, src",
        operator = "<<=",
    )]
    Lsh64Reg,
    #[opcode(
        mnemonic = "rsh64",
        code = 0x77,
        group = OperationType::BinaryImmediate,
        doc = "rsh64 dst, imm",
        operator = ">>=",
    )]
    Rsh64Imm,
    #[opcode(
        mnemonic = "rsh64",
        code = 0x7f,
        group = OperationType::BinaryRegister,
        doc = "rsh64 dst, src",
        operator = ">>=",
    )]
    Rsh64Reg,
    #[opcode(
        mnemonic = "mod64",
        code = 0x97,
        group = OperationType::BinaryImmediate,
        doc = "mod64 dst, imm",
        operator = "%=",
    )]
    Mod64Imm,
    #[opcode(
        mnemonic = "mod64",
        code = 0x9f,
        group = OperationType::BinaryRegister,
        doc = "mod64 dst, src",
        operator = "%=",
    )]
    Mod64Reg,
    #[opcode(
        mnemonic = "xor64",
        code = 0xa7,
        group = OperationType::BinaryImmediate,
        doc = "xor64 dst, imm",
        operator = "^=",
    )]
    Xor64Imm,
    #[opcode(
        mnemonic = "xor64",
        code = 0xaf,
        group = OperationType::BinaryRegister,
        doc = "xor64 dst, src",
        operator = "^=",
    )]
    Xor64Reg,
    #[opcode(
        mnemonic = "mov64",
        code = 0xb7,
        group = OperationType::BinaryImmediate,
        doc = "mov64 dst, imm",
        operator = "=",
    )]
    Mov64Imm,
    #[opcode(
        mnemonic = "mov64",
        code = 0xbf,
        group = OperationType::BinaryRegister,
        doc = "mov64 dst, src",
        operator = "=",
    )]
    Mov64Reg,
    #[opcode(
        mnemonic = "arsh64",
        code = 0xc7,
        group = OperationType::BinaryImmediate,
        doc = "arsh64 dst, imm",
        operator = "s>>=",
    )]
    Arsh64Imm,
    #[opcode(
        mnemonic = "arsh64",
        code = 0xcf,
        group = OperationType::BinaryRegister,
        doc = "arsh64 dst, src",
        operator = "s>>=",
    )]
    Arsh64Reg,
    #[opcode(
        mnemonic = "hor64",
        code = 0xf7,
        group = OperationType::BinaryImmediate,
        doc = "hor64 dst, imm",
        arch = v2,
    )]
    Hor64Imm,
    #[opcode(
        mnemonic = "lmul64",
        code = 0x96,
        group = OperationType::BinaryImmediate,
        doc = "lmul64 dst, imm",
        arch = v2,
    )]
    Lmul64Imm,
    #[opcode(
        mnemonic = "lmul64",
        code = 0x9e,
        group = OperationType::BinaryRegister,
        doc = "lmul64 dst, src",
        arch = v2,
    )]
    Lmul64Reg,
    #[opcode(
        mnemonic = "uhmul64",
        code = 0x36,
        group = OperationType::BinaryImmediate,
        doc = "uhmul64 dst, imm",
        arch = v2,
    )]
    Uhmul64Imm,
    #[opcode(
        mnemonic = "uhmul64",
        code = 0x3e,
        group = OperationType::BinaryRegister,
        doc = "uhmul64 dst, src",
        arch = v2,
    )]
    Uhmul64Reg,
    #[opcode(
        mnemonic = "udiv64",
        code = 0x56,
        group = OperationType::BinaryImmediate,
        doc = "udiv64 dst, imm",
        arch = v2,
    )]
    Udiv64Imm,
    #[opcode(
        mnemonic = "udiv64",
        code = 0x5e,
        group = OperationType::BinaryRegister,
        doc = "udiv64 dst, src",
        arch = v2,
    )]
    Udiv64Reg,
    #[opcode(
        mnemonic = "urem64",
        code = 0x76,
        group = OperationType::BinaryImmediate,
        doc = "urem64 dst, imm",
        arch = v2,
    )]
    Urem64Imm,
    #[opcode(
        mnemonic = "urem64",
        code = 0x7e,
        group = OperationType::BinaryRegister,
        doc = "urem64 dst, src",
        arch = v2,
    )]
    Urem64Reg,
    #[opcode(
        mnemonic = "shmul64",
        code = 0xb6,
        group = OperationType::BinaryImmediate,
        doc = "shmul64 dst, imm",
        arch = v2,
    )]
    Shmul64Imm,
    #[opcode(
        mnemonic = "shmul64",
        code = 0xbe,
        group = OperationType::BinaryRegister,
        doc = "shmul64 dst, src",
        arch = v2,
    )]
    Shmul64Reg,
    #[opcode(
        mnemonic = "sdiv64",
        code = 0xd6,
        group = OperationType::BinaryImmediate,
        doc = "sdiv64 dst, imm",
        arch = v2,
    )]
    Sdiv64Imm,
    #[opcode(
        mnemonic = "sdiv64",
        code = 0xde,
        group = OperationType::BinaryRegister,
        doc = "sdiv64 dst, src",
        arch = v2,
    )]
    Sdiv64Reg,
    #[opcode(
        mnemonic = "srem64",
        code = 0xf6,
        group = OperationType::BinaryImmediate,
        doc = "srem64 dst, imm",
        arch = v2,
    )]
    Srem64Imm,
    #[opcode(
        mnemonic = "srem64",
        code = 0xfe,
        group = OperationType::BinaryRegister,
        doc = "srem64 dst, src",
        arch = v2,
    )]
    Srem64Reg,
    #[opcode(
        mnemonic = "neg32",
        code = 0x84,
        group = OperationType::Unary,
        doc = "neg32 dst",
    )]
    Neg32,
    #[opcode(
        mnemonic = "neg64",
        code = 0x87,
        group = OperationType::Unary,
        doc = "neg64 dst",
    )]
    Neg64,
    #[opcode(
        mnemonic = "ja",
        code = 0x05,
        group = OperationType::Jump,
        doc = "ja off",
    )]
    Ja,
    #[opcode(
        mnemonic = "jeq",
        code = 0x15,
        group = OperationType::JumpImmediate,
        doc = "jeq dst, imm, off",
        operator = "==",
    )]
    JeqImm,
    #[opcode(
        mnemonic = "jeq",
        code = 0x1d,
        group = OperationType::JumpRegister,
        doc = "jeq dst, src, off",
        operator = "==",
    )]
    JeqReg,
    #[opcode(
        mnemonic = "jgt",
        code = 0x25,
        group = OperationType::JumpImmediate,
        doc = "jgt dst, imm, off",
        operator = ">",
    )]
    JgtImm,
    #[opcode(
        mnemonic = "jgt",
        code = 0x2d,
        group = OperationType::JumpRegister,
        doc = "jgt dst, src, off",
        operator = ">",
    )]
    JgtReg,
    #[opcode(
        mnemonic = "jge",
        code = 0x35,
        group = OperationType::JumpImmediate,
        doc = "jge dst, imm, off",
        operator = ">=",
    )]
    JgeImm,
    #[opcode(
        mnemonic = "jge",
        code = 0x3d,
        group = OperationType::JumpRegister,
        doc = "jge dst, src, off",
        operator = ">=",
    )]
    JgeReg,
    #[opcode(
        mnemonic = "jlt",
        code = 0xa5,
        group = OperationType::JumpImmediate,
        doc = "jlt dst, imm, off",
        operator = "<",
    )]
    JltImm,
    #[opcode(
        mnemonic = "jlt",
        code = 0xad,
        group = OperationType::JumpRegister,
        doc = "jlt dst, src, off",
        operator = "<",
    )]
    JltReg,
    #[opcode(
        mnemonic = "jle",
        code = 0xb5,
        group = OperationType::JumpImmediate,
        doc = "jle dst, imm, off",
        operator = "<=",
    )]
    JleImm,
    #[opcode(
        mnemonic = "jle",
        code = 0xbd,
        group = OperationType::JumpRegister,
        doc = "jle dst, src, off",
        operator = "<=",
    )]
    JleReg,
    #[opcode(
        mnemonic = "jset",
        code = 0x45,
        group = OperationType::JumpImmediate,
        doc = "jset dst, imm, off",
        operator = "&",
    )]
    JsetImm,
    #[opcode(
        mnemonic = "jset",
        code = 0x4d,
        group = OperationType::JumpRegister,
        doc = "jset dst, src, off",
        operator = "&",
    )]
    JsetReg,
    #[opcode(
        mnemonic = "jne",
        code = 0x55,
        group = OperationType::JumpImmediate,
        doc = "jne dst, imm, off",
        operator = "!=",
    )]
    JneImm,
    #[opcode(
        mnemonic = "jne",
        code = 0x5d,
        group = OperationType::JumpRegister,
        doc = "jne dst, src, off",
        operator = "!=",
    )]
    JneReg,
    #[opcode(
        mnemonic = "jsgt",
        code = 0x65,
        group = OperationType::JumpImmediate,
        doc = "jsgt dst, imm, off",
        operator = "s>",
    )]
    JsgtImm,
    #[opcode(
        mnemonic = "jsgt",
        code = 0x6d,
        group = OperationType::JumpRegister,
        doc = "jsgt dst, src, off",
        operator = "s>",
    )]
    JsgtReg,
    #[opcode(
        mnemonic = "jsge",
        code = 0x75,
        group = OperationType::JumpImmediate,
        doc = "jsge dst, imm, off",
        operator = "s>=",
    )]
    JsgeImm,
    #[opcode(
        mnemonic = "jsge",
        code = 0x7d,
        group = OperationType::JumpRegister,
        doc = "jsge dst, src, off",
        operator = "s>=",
    )]
    JsgeReg,
    #[opcode(
        mnemonic = "jslt",
        code = 0xc5,
        group = OperationType::JumpImmediate,
        doc = "jslt dst, imm, off",
        operator = "s<",
    )]
    JsltImm,
    #[opcode(
        mnemonic = "jslt",
        code = 0xcd,
        group = OperationType::JumpRegister,
        doc = "jslt dst, src, off",
        operator = "s<",
    )]
    JsltReg,
    #[opcode(
        mnemonic = "jsle",
        code = 0xd5,
        group = OperationType::JumpImmediate,
        doc = "jsle dst, imm, off",
        operator = "s<=",
    )]
    JsleImm,
    #[opcode(
        mnemonic = "jsle",
        code = 0xdd,
        group = OperationType::JumpRegister,
        doc = "jsle dst, src, off",
        operator = "s<=",
    )]
    JsleReg,
    #[opcode(
        mnemonic = "jeq32",
        code = 0x16,
        group = OperationType::Jump32Immediate,
        doc = "jeq32 dst, imm, off",
        arch = v3,
        operator = "==",
    )]
    Jeq32Imm,
    #[opcode(
        mnemonic = "jeq32",
        code = 0x1e,
        group = OperationType::Jump32Register,
        doc = "jeq32 dst, src, off",
        arch = v3,
        operator = "==",
    )]
    Jeq32Reg,
    #[opcode(
        mnemonic = "jgt32",
        code = 0x26,
        group = OperationType::Jump32Immediate,
        doc = "jgt32 dst, imm, off",
        arch = v3,
        operator = ">",
    )]
    Jgt32Imm,
    #[opcode(
        mnemonic = "jgt32",
        code = 0x2e,
        group = OperationType::Jump32Register,
        doc = "jgt32 dst, src, off",
        arch = v3,
        operator = ">",
    )]
    Jgt32Reg,
    #[opcode(
        mnemonic = "jge32",
        code = 0x36,
        group = OperationType::Jump32Immediate,
        doc = "jge32 dst, imm, off",
        arch = v3,
        operator = ">=",
    )]
    Jge32Imm,
    #[opcode(
        mnemonic = "jge32",
        code = 0x3e,
        group = OperationType::Jump32Register,
        doc = "jge32 dst, src, off",
        arch = v3,
        operator = ">=",
    )]
    Jge32Reg,
    #[opcode(
        mnemonic = "jlt32",
        code = 0xa6,
        group = OperationType::Jump32Immediate,
        doc = "jlt32 dst, imm, off",
        arch = v3,
        operator = "<",
    )]
    Jlt32Imm,
    #[opcode(
        mnemonic = "jlt32",
        code = 0xae,
        group = OperationType::Jump32Register,
        doc = "jlt32 dst, src, off",
        arch = v3,
        operator = "<",
    )]
    Jlt32Reg,
    #[opcode(
        mnemonic = "jle32",
        code = 0xb6,
        group = OperationType::Jump32Immediate,
        doc = "jle32 dst, imm, off",
        arch = v3,
        operator = "<=",
    )]
    Jle32Imm,
    #[opcode(
        mnemonic = "jle32",
        code = 0xbe,
        group = OperationType::Jump32Register,
        doc = "jle32 dst, src, off",
        arch = v3,
        operator = "<=",
    )]
    Jle32Reg,
    #[opcode(
        mnemonic = "jset32",
        code = 0x46,
        group = OperationType::Jump32Immediate,
        doc = "jset32 dst, imm, off",
        arch = v3,
        operator = "&",
    )]
    Jset32Imm,
    #[opcode(
        mnemonic = "jset32",
        code = 0x4e,
        group = OperationType::Jump32Register,
        doc = "jset32 dst, src, off",
        arch = v3,
        operator = "&",
    )]
    Jset32Reg,
    #[opcode(
        mnemonic = "jne32",
        code = 0x56,
        group = OperationType::Jump32Immediate,
        doc = "jne32 dst, imm, off",
        arch = v3,
        operator = "!=",
    )]
    Jne32Imm,
    #[opcode(
        mnemonic = "jne32",
        code = 0x5e,
        group = OperationType::Jump32Register,
        doc = "jne32 dst, src, off",
        arch = v3,
        operator = "!=",
    )]
    Jne32Reg,
    #[opcode(
        mnemonic = "jsgt32",
        code = 0x66,
        group = OperationType::Jump32Immediate,
        doc = "jsgt32 dst, imm, off",
        arch = v3,
        operator = "s>",
    )]
    Jsgt32Imm,
    #[opcode(
        mnemonic = "jsgt32",
        code = 0x6e,
        group = OperationType::Jump32Register,
        doc = "jsgt32 dst, src, off",
        arch = v3,
        operator = "s>",
    )]
    Jsgt32Reg,
    #[opcode(
        mnemonic = "jsge32",
        code = 0x76,
        group = OperationType::Jump32Immediate,
        doc = "jsge32 dst, imm, off",
        arch = v3,
        operator = "s>=",
    )]
    Jsge32Imm,
    #[opcode(
        mnemonic = "jsge32",
        code = 0x7e,
        group = OperationType::Jump32Register,
        doc = "jsge32 dst, src, off",
        arch = v3,
        operator = "s>=",
    )]
    Jsge32Reg,
    #[opcode(
        mnemonic = "jslt32",
        code = 0xc6,
        group = OperationType::Jump32Immediate,
        doc = "jslt32 dst, imm, off",
        arch = v3,
        operator = "s<",
    )]
    Jslt32Imm,
    #[opcode(
        mnemonic = "jslt32",
        code = 0xce,
        group = OperationType::Jump32Register,
        doc = "jslt32 dst, src, off",
        arch = v3,
        operator = "s<",
    )]
    Jslt32Reg,
    #[opcode(
        mnemonic = "jsle32",
        code = 0xd6,
        group = OperationType::Jump32Immediate,
        doc = "jsle32 dst, imm, off",
        arch = v3,
        operator = "s<=",
    )]
    Jsle32Imm,
    #[opcode(
        mnemonic = "jsle32",
        code = 0xde,
        group = OperationType::Jump32Register,
        doc = "jsle32 dst, src, off",
        arch = v3,
        operator = "s<=",
    )]
    Jsle32Reg,
    #[opcode(
        mnemonic = "call",
        code = 0x85,
        group = OperationType::CallImmediate,
        doc = "call imm",
    )]
    Call,
    #[opcode(
        mnemonic = "callx",
        code = 0x8d,
        group = OperationType::CallRegister,
        doc = "callx src",
    )]
    Callx,
    #[opcode(
        mnemonic = "exit",
        code = 0x95,
        group = OperationType::Exit,
        doc = "exit",
    )]
    Exit,
}

#[cfg(test)]
mod tests {
    use {super::*, crate::OpcodeTable, core::str::FromStr};

    #[test]
    fn test_opcode_from_str_load_ops() {
        assert_eq!(Opcode::from_str("lddw").unwrap(), Opcode::Lddw);
        assert_eq!(Opcode::from_str("LDDW").unwrap(), Opcode::Lddw);
        assert_eq!(Opcode::from_str("ldxb").unwrap(), Opcode::Ldxb);
        assert_eq!(Opcode::from_str("ldxh").unwrap(), Opcode::Ldxh);
        assert_eq!(Opcode::from_str("ldxw").unwrap(), Opcode::Ldxw);
        assert_eq!(Opcode::from_str("ldxdw").unwrap(), Opcode::Ldxdw);
    }

    #[test]
    fn test_opcode_from_str_store_ops() {
        assert_eq!(Opcode::from_str("stb").unwrap(), Opcode::Stb);
        assert_eq!(Opcode::from_str("sth").unwrap(), Opcode::Sth);
        assert_eq!(Opcode::from_str("stw").unwrap(), Opcode::Stw);
        assert_eq!(Opcode::from_str("stdw").unwrap(), Opcode::Stdw);
        assert_eq!(Opcode::from_str("stxb").unwrap(), Opcode::Stxb);
        assert_eq!(Opcode::from_str("stxh").unwrap(), Opcode::Stxh);
        assert_eq!(Opcode::from_str("stxw").unwrap(), Opcode::Stxw);
        assert_eq!(Opcode::from_str("stxdw").unwrap(), Opcode::Stxdw);
    }

    #[test]
    fn test_opcode_from_str_alu32_ops() {
        assert_eq!(Opcode::from_str("add32").unwrap(), Opcode::Add32Imm);
        assert_eq!(Opcode::from_str("sub32").unwrap(), Opcode::Sub32Imm);
        assert_eq!(Opcode::from_str("mul32").unwrap(), Opcode::Mul32Imm);
        assert_eq!(Opcode::from_str("div32").unwrap(), Opcode::Div32Imm);
        assert_eq!(Opcode::from_str("or32").unwrap(), Opcode::Or32Imm);
        assert_eq!(Opcode::from_str("and32").unwrap(), Opcode::And32Imm);
        assert_eq!(Opcode::from_str("lsh32").unwrap(), Opcode::Lsh32Imm);
        assert_eq!(Opcode::from_str("rsh32").unwrap(), Opcode::Rsh32Imm);
        assert_eq!(Opcode::from_str("neg32").unwrap(), Opcode::Neg32);
        assert_eq!(Opcode::from_str("mod32").unwrap(), Opcode::Mod32Imm);
        assert_eq!(Opcode::from_str("xor32").unwrap(), Opcode::Xor32Imm);
        assert_eq!(Opcode::from_str("mov32").unwrap(), Opcode::Mov32Imm);
        assert_eq!(Opcode::from_str("arsh32").unwrap(), Opcode::Arsh32Imm);
        assert_eq!(Opcode::from_str("lmul32").unwrap(), Opcode::Lmul32Imm);
        assert_eq!(Opcode::from_str("udiv32").unwrap(), Opcode::Udiv32Imm);
        assert_eq!(Opcode::from_str("urem32").unwrap(), Opcode::Urem32Imm);
        assert_eq!(Opcode::from_str("sdiv32").unwrap(), Opcode::Sdiv32Imm);
        assert_eq!(Opcode::from_str("srem32").unwrap(), Opcode::Srem32Imm);
    }

    #[test]
    fn test_opcode_from_str_alu64_ops() {
        assert_eq!(Opcode::from_str("add64").unwrap(), Opcode::Add64Imm);
        assert_eq!(Opcode::from_str("sub64").unwrap(), Opcode::Sub64Imm);
        assert_eq!(Opcode::from_str("mul64").unwrap(), Opcode::Mul64Imm);
        assert_eq!(Opcode::from_str("div64").unwrap(), Opcode::Div64Imm);
        assert_eq!(Opcode::from_str("or64").unwrap(), Opcode::Or64Imm);
        assert_eq!(Opcode::from_str("and64").unwrap(), Opcode::And64Imm);
        assert_eq!(Opcode::from_str("neg64").unwrap(), Opcode::Neg64);
        assert_eq!(Opcode::from_str("mov64").unwrap(), Opcode::Mov64Imm);
        assert_eq!(Opcode::from_str("lsh64").unwrap(), Opcode::Lsh64Imm);
        assert_eq!(Opcode::from_str("rsh64").unwrap(), Opcode::Rsh64Imm);
        assert_eq!(Opcode::from_str("mod64").unwrap(), Opcode::Mod64Imm);
        assert_eq!(Opcode::from_str("xor64").unwrap(), Opcode::Xor64Imm);
        assert_eq!(Opcode::from_str("arsh64").unwrap(), Opcode::Arsh64Imm);
        assert_eq!(Opcode::from_str("hor64").unwrap(), Opcode::Hor64Imm);
        assert_eq!(Opcode::from_str("lmul64").unwrap(), Opcode::Lmul64Imm);
        assert_eq!(Opcode::from_str("uhmul64").unwrap(), Opcode::Uhmul64Imm);
        assert_eq!(Opcode::from_str("udiv64").unwrap(), Opcode::Udiv64Imm);
        assert_eq!(Opcode::from_str("urem64").unwrap(), Opcode::Urem64Imm);
        assert_eq!(Opcode::from_str("shmul64").unwrap(), Opcode::Shmul64Imm);
        assert_eq!(Opcode::from_str("sdiv64").unwrap(), Opcode::Sdiv64Imm);
        assert_eq!(Opcode::from_str("srem64").unwrap(), Opcode::Srem64Imm);
    }

    #[test]
    fn test_opcode_from_str_be_le() {
        assert_eq!(Opcode::from_str("le").unwrap(), Opcode::Le);
        assert_eq!(Opcode::from_str("be").unwrap(), Opcode::Be);
    }

    #[test]
    fn test_opcode_from_str_jump_ops() {
        assert_eq!(Opcode::from_str("ja").unwrap(), Opcode::Ja);
        assert_eq!(Opcode::from_str("jeq").unwrap(), Opcode::JeqImm);
        assert_eq!(Opcode::from_str("jgt").unwrap(), Opcode::JgtImm);
        assert_eq!(Opcode::from_str("jge").unwrap(), Opcode::JgeImm);
        assert_eq!(Opcode::from_str("jlt").unwrap(), Opcode::JltImm);
        assert_eq!(Opcode::from_str("jne").unwrap(), Opcode::JneImm);
        assert_eq!(Opcode::from_str("jle").unwrap(), Opcode::JleImm);
        assert_eq!(Opcode::from_str("jset").unwrap(), Opcode::JsetImm);
        assert_eq!(Opcode::from_str("jsgt").unwrap(), Opcode::JsgtImm);
        assert_eq!(Opcode::from_str("jsge").unwrap(), Opcode::JsgeImm);
        assert_eq!(Opcode::from_str("jslt").unwrap(), Opcode::JsltImm);
        assert_eq!(Opcode::from_str("jsle").unwrap(), Opcode::JsleImm);
        assert_eq!(Opcode::from_str("jeq32").unwrap(), Opcode::Jeq32Imm);
        assert_eq!(Opcode::from_str("jgt32").unwrap(), Opcode::Jgt32Imm);
        assert_eq!(Opcode::from_str("jge32").unwrap(), Opcode::Jge32Imm);
        assert_eq!(Opcode::from_str("jlt32").unwrap(), Opcode::Jlt32Imm);
        assert_eq!(Opcode::from_str("jle32").unwrap(), Opcode::Jle32Imm);
        assert_eq!(Opcode::from_str("jset32").unwrap(), Opcode::Jset32Imm);
        assert_eq!(Opcode::from_str("jne32").unwrap(), Opcode::Jne32Imm);
        assert_eq!(Opcode::from_str("jsgt32").unwrap(), Opcode::Jsgt32Imm);
        assert_eq!(Opcode::from_str("jsge32").unwrap(), Opcode::Jsge32Imm);
        assert_eq!(Opcode::from_str("jslt32").unwrap(), Opcode::Jslt32Imm);
        assert_eq!(Opcode::from_str("jsle32").unwrap(), Opcode::Jsle32Imm);
    }

    #[test]
    fn test_opcode_from_str_call_and_exit_ops() {
        assert!(Opcode::from_str("invalid").is_err());
        assert!(Opcode::from_str("").is_err());
        assert!(Opcode::from_str("xyz").is_err());
        assert_eq!(Opcode::from_str("call").unwrap(), Opcode::Call);
        assert_eq!(Opcode::from_str("callx").unwrap(), Opcode::Callx);
        assert_eq!(Opcode::from_str("exit").unwrap(), Opcode::Exit);
    }

    #[test]
    fn test_opcode_from_str_invalid() {
        assert!(Opcode::from_str("invalid").is_err());
        assert!(Opcode::from_str("").is_err());
        assert!(Opcode::from_str("xyz").is_err());
    }

    #[test]
    fn test_all_load_memory_ops() {
        for &op in Opcode::by_group(OperationType::LoadMemory) {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_all_bin_imm_ops() {
        for &op in Opcode::by_group(OperationType::BinaryImmediate) {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_all_jump_imm_ops() {
        for &op in Opcode::by_group(OperationType::JumpImmediate) {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_all_store_imm_ops() {
        for &op in Opcode::by_group(OperationType::StoreImmediate) {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_all_store_reg_ops() {
        for &op in Opcode::by_group(OperationType::StoreRegister) {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_all_bin_reg_ops() {
        for &op in Opcode::by_group(OperationType::BinaryRegister) {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_all_unary_ops() {
        for &op in Opcode::by_group(OperationType::Unary) {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_all_endian_ops() {
        for &op in Opcode::by_group(OperationType::Endian) {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_all_jump_ops() {
        for &op in Opcode::by_group(OperationType::Jump) {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_all_jump_reg_ops() {
        for &op in Opcode::by_group(OperationType::JumpRegister) {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_all_jump32_imm_ops() {
        for &op in Opcode::by_group(OperationType::Jump32Immediate) {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from_sbpf_v3(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_all_jump32_reg_ops() {
        for &op in Opcode::by_group(OperationType::Jump32Register) {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from_sbpf_v3(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_all_call_ops() {
        for &op in Opcode::by_group(OperationType::CallImmediate) {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
        for &op in Opcode::by_group(OperationType::CallRegister) {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_exit_op() {
        for &op in Opcode::by_group(OperationType::Exit) {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_to_str_all_load_ops() {
        assert_eq!(Opcode::Lddw.to_str(), "lddw");
        assert_eq!(Opcode::Ldxb.to_str(), "ldxb");
        assert_eq!(Opcode::Ldxh.to_str(), "ldxh");
        assert_eq!(Opcode::Ldxw.to_str(), "ldxw");
        assert_eq!(Opcode::Ldxdw.to_str(), "ldxdw");
    }

    #[test]
    fn test_to_str_all_store_ops() {
        assert_eq!(Opcode::Stb.to_str(), "stb");
        assert_eq!(Opcode::Sth.to_str(), "sth");
        assert_eq!(Opcode::Stw.to_str(), "stw");
        assert_eq!(Opcode::Stdw.to_str(), "stdw");
        assert_eq!(Opcode::Stxb.to_str(), "stxb");
        assert_eq!(Opcode::Stxh.to_str(), "stxh");
        assert_eq!(Opcode::Stxw.to_str(), "stxw");
        assert_eq!(Opcode::Stxdw.to_str(), "stxdw");
    }

    #[test]
    fn test_to_str_all_alu32_ops() {
        assert_eq!(Opcode::Add32Imm.to_str(), "add32");
        assert_eq!(Opcode::Add32Reg.to_str(), "add32");
        assert_eq!(Opcode::Sub32Imm.to_str(), "sub32");
        assert_eq!(Opcode::Mul32Imm.to_str(), "mul32");
        assert_eq!(Opcode::Div32Imm.to_str(), "div32");
        assert_eq!(Opcode::Or32Imm.to_str(), "or32");
        assert_eq!(Opcode::And32Imm.to_str(), "and32");
        assert_eq!(Opcode::Lsh32Imm.to_str(), "lsh32");
        assert_eq!(Opcode::Rsh32Imm.to_str(), "rsh32");
        assert_eq!(Opcode::Neg32.to_str(), "neg32");
        assert_eq!(Opcode::Mod32Imm.to_str(), "mod32");
        assert_eq!(Opcode::Xor32Imm.to_str(), "xor32");
        assert_eq!(Opcode::Mov32Imm.to_str(), "mov32");
        assert_eq!(Opcode::Arsh32Imm.to_str(), "arsh32");
        assert_eq!(Opcode::Lmul32Imm.to_str(), "lmul32");
        assert_eq!(Opcode::Lmul32Reg.to_str(), "lmul32");
        assert_eq!(Opcode::Udiv32Imm.to_str(), "udiv32");
        assert_eq!(Opcode::Urem32Imm.to_str(), "urem32");
        assert_eq!(Opcode::Sdiv32Imm.to_str(), "sdiv32");
        assert_eq!(Opcode::Srem32Imm.to_str(), "srem32");
    }

    #[test]
    fn test_to_str_all_alu64_ops() {
        assert_eq!(Opcode::Add64Imm.to_str(), "add64");
        assert_eq!(Opcode::Sub64Imm.to_str(), "sub64");
        assert_eq!(Opcode::Mul64Imm.to_str(), "mul64");
        assert_eq!(Opcode::Div64Imm.to_str(), "div64");
        assert_eq!(Opcode::Or64Imm.to_str(), "or64");
        assert_eq!(Opcode::And64Imm.to_str(), "and64");
        assert_eq!(Opcode::Lsh64Imm.to_str(), "lsh64");
        assert_eq!(Opcode::Rsh64Imm.to_str(), "rsh64");
        assert_eq!(Opcode::Neg64.to_str(), "neg64");
        assert_eq!(Opcode::Mod64Imm.to_str(), "mod64");
        assert_eq!(Opcode::Xor64Imm.to_str(), "xor64");
        assert_eq!(Opcode::Mov64Imm.to_str(), "mov64");
        assert_eq!(Opcode::Arsh64Imm.to_str(), "arsh64");
        assert_eq!(Opcode::Hor64Imm.to_str(), "hor64");
        assert_eq!(Opcode::Lmul64Imm.to_str(), "lmul64");
        assert_eq!(Opcode::Uhmul64Imm.to_str(), "uhmul64");
        assert_eq!(Opcode::Udiv64Imm.to_str(), "udiv64");
        assert_eq!(Opcode::Urem64Imm.to_str(), "urem64");
        assert_eq!(Opcode::Shmul64Imm.to_str(), "shmul64");
        assert_eq!(Opcode::Sdiv64Imm.to_str(), "sdiv64");
        assert_eq!(Opcode::Srem64Imm.to_str(), "srem64");
    }

    #[test]
    fn test_to_str_be_le_ops() {
        assert_eq!(Opcode::Be.to_str(), "be");
        assert_eq!(Opcode::Le.to_str(), "le");
    }

    #[test]
    fn test_to_str_all_jump_ops() {
        assert_eq!(Opcode::Ja.to_str(), "ja");
        assert_eq!(Opcode::JeqImm.to_str(), "jeq");
        assert_eq!(Opcode::JeqReg.to_str(), "jeq");
        assert_eq!(Opcode::JgtImm.to_str(), "jgt");
        assert_eq!(Opcode::JgeImm.to_str(), "jge");
        assert_eq!(Opcode::JltImm.to_str(), "jlt");
        assert_eq!(Opcode::JleImm.to_str(), "jle");
        assert_eq!(Opcode::JsetImm.to_str(), "jset");
        assert_eq!(Opcode::JneImm.to_str(), "jne");
        assert_eq!(Opcode::JsgtImm.to_str(), "jsgt");
        assert_eq!(Opcode::JsgeImm.to_str(), "jsge");
        assert_eq!(Opcode::JsltImm.to_str(), "jslt");
        assert_eq!(Opcode::JsleImm.to_str(), "jsle");
        assert_eq!(Opcode::Jeq32Imm.to_str(), "jeq32");
        assert_eq!(Opcode::Jeq32Reg.to_str(), "jeq32");
        assert_eq!(Opcode::Jgt32Imm.to_str(), "jgt32");
        assert_eq!(Opcode::Jgt32Reg.to_str(), "jgt32");
        assert_eq!(Opcode::Jge32Imm.to_str(), "jge32");
        assert_eq!(Opcode::Jge32Reg.to_str(), "jge32");
        assert_eq!(Opcode::Jlt32Imm.to_str(), "jlt32");
        assert_eq!(Opcode::Jlt32Reg.to_str(), "jlt32");
        assert_eq!(Opcode::Jle32Imm.to_str(), "jle32");
        assert_eq!(Opcode::Jle32Reg.to_str(), "jle32");
        assert_eq!(Opcode::Jset32Imm.to_str(), "jset32");
        assert_eq!(Opcode::Jset32Reg.to_str(), "jset32");
        assert_eq!(Opcode::Jne32Imm.to_str(), "jne32");
        assert_eq!(Opcode::Jne32Reg.to_str(), "jne32");
        assert_eq!(Opcode::Jsgt32Imm.to_str(), "jsgt32");
        assert_eq!(Opcode::Jsgt32Reg.to_str(), "jsgt32");
        assert_eq!(Opcode::Jsge32Imm.to_str(), "jsge32");
        assert_eq!(Opcode::Jsge32Reg.to_str(), "jsge32");
        assert_eq!(Opcode::Jslt32Imm.to_str(), "jslt32");
        assert_eq!(Opcode::Jslt32Reg.to_str(), "jslt32");
        assert_eq!(Opcode::Jsle32Imm.to_str(), "jsle32");
        assert_eq!(Opcode::Jsle32Reg.to_str(), "jsle32");
    }

    #[test]
    fn test_to_str_call_and_exit_ops() {
        assert_eq!(Opcode::Call.to_str(), "call");
        assert_eq!(Opcode::Callx.to_str(), "callx");
        assert_eq!(Opcode::Exit.to_str(), "exit");
    }
}
