use {
    crate::errors::SBPFError,
    core::{fmt, str::FromStr},
    num_derive::FromPrimitive,
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OperationType {
    LoadImmediate,
    LoadMemory,
    StoreImmediate,
    StoreRegister,
    BinaryImmediate,
    BinaryRegister,
    Unary,
    Jump,
    JumpImmediate,
    JumpRegister,
    CallImmediate,
    CallRegister,
    Exit,
}

pub const LOAD_IMM_OPS: &[Opcode] = &[Opcode::Lddw]; // OperationType::LoadImmediate

pub const LOAD_MEMORY_OPS: &[Opcode] = &[
    Opcode::Ldxb, // OperationType::LoadMemory
    Opcode::Ldxh,
    Opcode::Ldxw,
    Opcode::Ldxdw,
];

pub const STORE_IMM_OPS: &[Opcode] = &[
    Opcode::Stb, // OperationType::StoreImmediate
    Opcode::Sth,
    Opcode::Stw,
    Opcode::Stdw,
];

pub const STORE_REG_OPS: &[Opcode] = &[
    Opcode::Stxb, // OperationType::StoreRegister
    Opcode::Stxh,
    Opcode::Stxw,
    Opcode::Stxdw,
];

pub const BIN_IMM_OPS: &[Opcode] = &[
    Opcode::Add32Imm, // OperationType::BinaryImmediate
    Opcode::Sub32Imm,
    Opcode::Mul32Imm,
    Opcode::Div32Imm,
    Opcode::Or32Imm,
    Opcode::And32Imm,
    Opcode::Lsh32Imm,
    Opcode::Rsh32Imm,
    Opcode::Mod32Imm,
    Opcode::Xor32Imm,
    Opcode::Mov32Imm,
    Opcode::Arsh32Imm,
    Opcode::Lmul32Imm,
    Opcode::Udiv32Imm,
    Opcode::Urem32Imm,
    Opcode::Sdiv32Imm,
    Opcode::Srem32Imm,
    Opcode::Le,
    Opcode::Be,
    Opcode::Add64Imm,
    Opcode::Sub64Imm,
    Opcode::Mul64Imm,
    Opcode::Div64Imm,
    Opcode::Or64Imm,
    Opcode::And64Imm,
    Opcode::Lsh64Imm,
    Opcode::Rsh64Imm,
    Opcode::Mod64Imm,
    Opcode::Xor64Imm,
    Opcode::Mov64Imm,
    Opcode::Arsh64Imm,
    Opcode::Hor64Imm,
    Opcode::Lmul64Imm,
    Opcode::Uhmul64Imm,
    Opcode::Udiv64Imm,
    Opcode::Urem64Imm,
    Opcode::Shmul64Imm,
    Opcode::Sdiv64Imm,
    Opcode::Srem64Imm,
];

pub const BIN_REG_OPS: &[Opcode] = &[
    Opcode::Add32Reg, // OperationType::BinaryRegister
    Opcode::Sub32Reg,
    Opcode::Mul32Reg,
    Opcode::Div32Reg,
    Opcode::Or32Reg,
    Opcode::And32Reg,
    Opcode::Lsh32Reg,
    Opcode::Rsh32Reg,
    Opcode::Mod32Reg,
    Opcode::Xor32Reg,
    Opcode::Mov32Reg,
    Opcode::Arsh32Reg,
    Opcode::Lmul32Reg,
    Opcode::Udiv32Reg,
    Opcode::Urem32Reg,
    Opcode::Sdiv32Reg,
    Opcode::Srem32Reg,
    Opcode::Add64Reg,
    Opcode::Sub64Reg,
    Opcode::Mul64Reg,
    Opcode::Div64Reg,
    Opcode::Or64Reg,
    Opcode::And64Reg,
    Opcode::Lsh64Reg,
    Opcode::Rsh64Reg,
    Opcode::Mod64Reg,
    Opcode::Xor64Reg,
    Opcode::Mov64Reg,
    Opcode::Arsh64Reg,
    Opcode::Lmul64Reg,
    Opcode::Uhmul64Reg,
    Opcode::Udiv64Reg,
    Opcode::Urem64Reg,
    Opcode::Shmul64Reg,
    Opcode::Sdiv64Reg,
    Opcode::Srem64Reg,
];

pub const UNARY_OPS: &[Opcode] = &[
    Opcode::Neg32, // OperationType::Unary
    Opcode::Neg64,
];

pub const JUMP_OPS: &[Opcode] = &[Opcode::Ja]; // OperationType::Jump

pub const JUMP_IMM_OPS: &[Opcode] = &[
    Opcode::JeqImm, // OperationType::JumpImmediate
    Opcode::JgtImm,
    Opcode::JgeImm,
    Opcode::JltImm,
    Opcode::JleImm,
    Opcode::JsetImm,
    Opcode::JneImm,
    Opcode::JsgtImm,
    Opcode::JsgeImm,
    Opcode::JsltImm,
    Opcode::JsleImm,
];

pub const JUMP_REG_OPS: &[Opcode] = &[
    Opcode::JeqReg, // OperationType::JumpRegister
    Opcode::JgtReg,
    Opcode::JgeReg,
    Opcode::JltReg,
    Opcode::JleReg,
    Opcode::JsetReg,
    Opcode::JneReg,
    Opcode::JsgtReg,
    Opcode::JsgeReg,
    Opcode::JsltReg,
    Opcode::JsleReg,
];

pub const CALL_IMM_OPS: &[Opcode] = &[Opcode::Call]; // OperationType::CallImmediate

pub const CALL_REG_OPS: &[Opcode] = &[Opcode::Callx]; // OperationType::CallRegister

pub const EXIT_OPS: &[Opcode] = &[Opcode::Exit]; // OperationType::Exit
//
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, FromPrimitive, Serialize, Deserialize)]
pub enum Opcode {
    Lddw,
    Ldxb,
    Ldxh,
    Ldxw,
    Ldxdw,
    Stb,
    Sth,
    Stw,
    Stdw,
    Stxb,
    Stxh,
    Stxw,
    Stxdw,
    Add32Imm,
    Add32Reg,
    Sub32Imm,
    Sub32Reg,
    Mul32Imm,
    Mul32Reg,
    Div32Imm,
    Div32Reg,
    Or32Imm,
    Or32Reg,
    And32Imm,
    And32Reg,
    Lsh32Imm,
    Lsh32Reg,
    Rsh32Imm,
    Rsh32Reg,
    Mod32Imm,
    Mod32Reg,
    Xor32Imm,
    Xor32Reg,
    Mov32Imm,
    Mov32Reg,
    Arsh32Imm,
    Arsh32Reg,
    Lmul32Imm,
    Lmul32Reg,
    Udiv32Imm,
    Udiv32Reg,
    Urem32Imm,
    Urem32Reg,
    Sdiv32Imm,
    Sdiv32Reg,
    Srem32Imm,
    Srem32Reg,
    Le,
    Be,
    Add64Imm,
    Add64Reg,
    Sub64Imm,
    Sub64Reg,
    Mul64Imm,
    Mul64Reg,
    Div64Imm,
    Div64Reg,
    Or64Imm,
    Or64Reg,
    And64Imm,
    And64Reg,
    Lsh64Imm,
    Lsh64Reg,
    Rsh64Imm,
    Rsh64Reg,
    Mod64Imm,
    Mod64Reg,
    Xor64Imm,
    Xor64Reg,
    Mov64Imm,
    Mov64Reg,
    Arsh64Imm,
    Arsh64Reg,
    Hor64Imm,
    Lmul64Imm,
    Lmul64Reg,
    Uhmul64Imm,
    Uhmul64Reg,
    Udiv64Imm,
    Udiv64Reg,
    Urem64Imm,
    Urem64Reg,
    Shmul64Imm,
    Shmul64Reg,
    Sdiv64Imm,
    Sdiv64Reg,
    Srem64Imm,
    Srem64Reg,
    Neg32,
    Neg64,
    Ja,
    JeqImm,
    JeqReg,
    JgtImm,
    JgtReg,
    JgeImm,
    JgeReg,
    JltImm,
    JltReg,
    JleImm,
    JleReg,
    JsetImm,
    JsetReg,
    JneImm,
    JneReg,
    JsgtImm,
    JsgtReg,
    JsgeImm,
    JsgeReg,
    JsltImm,
    JsltReg,
    JsleImm,
    JsleReg,
    Call,
    Callx,
    Exit,
}

impl FromStr for Opcode {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "lddw" => Ok(Opcode::Lddw),
            "ldxb" => Ok(Opcode::Ldxb),
            "ldxh" => Ok(Opcode::Ldxh),
            "ldxw" => Ok(Opcode::Ldxw),
            "ldxdw" => Ok(Opcode::Ldxdw),
            "stb" => Ok(Opcode::Stb),
            "sth" => Ok(Opcode::Sth),
            "stw" => Ok(Opcode::Stw),
            "stdw" => Ok(Opcode::Stdw),
            "stxb" => Ok(Opcode::Stxb),
            "stxh" => Ok(Opcode::Stxh),
            "stxw" => Ok(Opcode::Stxw),
            "stxdw" => Ok(Opcode::Stxdw),
            "add32" => Ok(Opcode::Add32Imm),
            "sub32" => Ok(Opcode::Sub32Imm),
            "mul32" => Ok(Opcode::Mul32Imm),
            "div32" => Ok(Opcode::Div32Imm),
            "or32" => Ok(Opcode::Or32Imm),
            "and32" => Ok(Opcode::And32Imm),
            "lsh32" => Ok(Opcode::Lsh32Imm),
            "rsh32" => Ok(Opcode::Rsh32Imm),
            "neg32" => Ok(Opcode::Neg32),
            "mod32" => Ok(Opcode::Mod32Imm),
            "xor32" => Ok(Opcode::Xor32Imm),
            "mov32" => Ok(Opcode::Mov32Imm),
            "arsh32" => Ok(Opcode::Arsh32Imm),
            "lmul32" => Ok(Opcode::Lmul32Imm),
            "udiv32" => Ok(Opcode::Udiv32Imm),
            "urem32" => Ok(Opcode::Urem32Imm),
            "sdiv32" => Ok(Opcode::Sdiv32Imm),
            "srem32" => Ok(Opcode::Srem32Imm),
            "le" => Ok(Opcode::Le),
            "be" => Ok(Opcode::Be),
            "add64" => Ok(Opcode::Add64Imm),
            "sub64" => Ok(Opcode::Sub64Imm),
            "mul64" => Ok(Opcode::Mul64Imm),
            "div64" => Ok(Opcode::Div64Imm),
            "or64" => Ok(Opcode::Or64Imm),
            "and64" => Ok(Opcode::And64Imm),
            "lsh64" => Ok(Opcode::Lsh64Imm),
            "rsh64" => Ok(Opcode::Rsh64Imm),
            "neg64" => Ok(Opcode::Neg64),
            "mod64" => Ok(Opcode::Mod64Imm),
            "xor64" => Ok(Opcode::Xor64Imm),
            "mov64" => Ok(Opcode::Mov64Imm),
            "arsh64" => Ok(Opcode::Arsh64Imm),
            "hor64" => Ok(Opcode::Hor64Imm),
            "lmul64" => Ok(Opcode::Lmul64Imm),
            "uhmul64" => Ok(Opcode::Uhmul64Imm),
            "udiv64" => Ok(Opcode::Udiv64Imm),
            "urem64" => Ok(Opcode::Urem64Imm),
            "shmul64" => Ok(Opcode::Shmul64Imm),
            "sdiv64" => Ok(Opcode::Sdiv64Imm),
            "srem64" => Ok(Opcode::Srem64Imm),
            "ja" => Ok(Opcode::Ja),
            "jeq" => Ok(Opcode::JeqImm),
            "jgt" => Ok(Opcode::JgtImm),
            "jge" => Ok(Opcode::JgeImm),
            "jlt" => Ok(Opcode::JltImm),
            "jle" => Ok(Opcode::JleImm),
            "jset" => Ok(Opcode::JsetImm),
            "jne" => Ok(Opcode::JneImm),
            "jsgt" => Ok(Opcode::JsgtImm),
            "jsge" => Ok(Opcode::JsgeImm),
            "jslt" => Ok(Opcode::JsltImm),
            "jsle" => Ok(Opcode::JsleImm),
            "call" => Ok(Opcode::Call),
            "callx" => Ok(Opcode::Callx),
            "exit" => Ok(Opcode::Exit),
            _ => Err("Invalid opcode"),
        }
    }
}

impl fmt::Display for Opcode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_str())
    }
}

impl TryFrom<u8> for Opcode {
    type Error = SBPFError;

    fn try_from(opcode: u8) -> Result<Self, Self::Error> {
        match opcode {
            0x18 => Ok(Opcode::Lddw),
            0x71 => Ok(Opcode::Ldxb),
            0x69 => Ok(Opcode::Ldxh),
            0x61 => Ok(Opcode::Ldxw),
            0x79 => Ok(Opcode::Ldxdw),
            0x72 => Ok(Opcode::Stb),
            0x6a => Ok(Opcode::Sth),
            0x62 => Ok(Opcode::Stw),
            0x7a => Ok(Opcode::Stdw),
            0x73 => Ok(Opcode::Stxb),
            0x6b => Ok(Opcode::Stxh),
            0x63 => Ok(Opcode::Stxw),
            0x7b => Ok(Opcode::Stxdw),
            0x04 => Ok(Opcode::Add32Imm),
            0x0c => Ok(Opcode::Add32Reg),
            0x14 => Ok(Opcode::Sub32Imm),
            0x1c => Ok(Opcode::Sub32Reg),
            0x24 => Ok(Opcode::Mul32Imm),
            0x2c => Ok(Opcode::Mul32Reg),
            0x34 => Ok(Opcode::Div32Imm),
            0x3c => Ok(Opcode::Div32Reg),
            0x44 => Ok(Opcode::Or32Imm),
            0x4c => Ok(Opcode::Or32Reg),
            0x54 => Ok(Opcode::And32Imm),
            0x5c => Ok(Opcode::And32Reg),
            0x64 => Ok(Opcode::Lsh32Imm),
            0x6c => Ok(Opcode::Lsh32Reg),
            0x74 => Ok(Opcode::Rsh32Imm),
            0x7c => Ok(Opcode::Rsh32Reg),
            0x94 => Ok(Opcode::Mod32Imm),
            0x9c => Ok(Opcode::Mod32Reg),
            0xa4 => Ok(Opcode::Xor32Imm),
            0xac => Ok(Opcode::Xor32Reg),
            0xb4 => Ok(Opcode::Mov32Imm),
            0xbc => Ok(Opcode::Mov32Reg),
            0xc4 => Ok(Opcode::Arsh32Imm),
            0xcc => Ok(Opcode::Arsh32Reg),
            0x86 => Ok(Opcode::Lmul32Imm),
            0x8e => Ok(Opcode::Lmul32Reg),
            0x46 => Ok(Opcode::Udiv32Imm),
            0x4e => Ok(Opcode::Udiv32Reg),
            0x66 => Ok(Opcode::Urem32Imm),
            0x6e => Ok(Opcode::Urem32Reg),
            0xc6 => Ok(Opcode::Sdiv32Imm),
            0xce => Ok(Opcode::Sdiv32Reg),
            0xe6 => Ok(Opcode::Srem32Imm),
            0xee => Ok(Opcode::Srem32Reg),
            0xd4 => Ok(Opcode::Le),
            0xdc => Ok(Opcode::Be),
            0x07 => Ok(Opcode::Add64Imm),
            0x0f => Ok(Opcode::Add64Reg),
            0x17 => Ok(Opcode::Sub64Imm),
            0x1f => Ok(Opcode::Sub64Reg),
            0x27 => Ok(Opcode::Mul64Imm),
            0x2f => Ok(Opcode::Mul64Reg),
            0x37 => Ok(Opcode::Div64Imm),
            0x3f => Ok(Opcode::Div64Reg),
            0x47 => Ok(Opcode::Or64Imm),
            0x4f => Ok(Opcode::Or64Reg),
            0x57 => Ok(Opcode::And64Imm),
            0x5f => Ok(Opcode::And64Reg),
            0x67 => Ok(Opcode::Lsh64Imm),
            0x6f => Ok(Opcode::Lsh64Reg),
            0x77 => Ok(Opcode::Rsh64Imm),
            0x7f => Ok(Opcode::Rsh64Reg),
            0x97 => Ok(Opcode::Mod64Imm),
            0x9f => Ok(Opcode::Mod64Reg),
            0xa7 => Ok(Opcode::Xor64Imm),
            0xaf => Ok(Opcode::Xor64Reg),
            0xb7 => Ok(Opcode::Mov64Imm),
            0xbf => Ok(Opcode::Mov64Reg),
            0xc7 => Ok(Opcode::Arsh64Imm),
            0xcf => Ok(Opcode::Arsh64Reg),
            0xf7 => Ok(Opcode::Hor64Imm),
            0x96 => Ok(Opcode::Lmul64Imm),
            0x9e => Ok(Opcode::Lmul64Reg),
            0x36 => Ok(Opcode::Uhmul64Imm),
            0x3e => Ok(Opcode::Uhmul64Reg),
            0x56 => Ok(Opcode::Udiv64Imm),
            0x5e => Ok(Opcode::Udiv64Reg),
            0x76 => Ok(Opcode::Urem64Imm),
            0x7e => Ok(Opcode::Urem64Reg),
            0xb6 => Ok(Opcode::Shmul64Imm),
            0xbe => Ok(Opcode::Shmul64Reg),
            0xd6 => Ok(Opcode::Sdiv64Imm),
            0xde => Ok(Opcode::Sdiv64Reg),
            0xf6 => Ok(Opcode::Srem64Imm),
            0xfe => Ok(Opcode::Srem64Reg),
            0x84 => Ok(Opcode::Neg32),
            0x87 => Ok(Opcode::Neg64),
            0x05 => Ok(Opcode::Ja),
            0x15 => Ok(Opcode::JeqImm),
            0x1d => Ok(Opcode::JeqReg),
            0x25 => Ok(Opcode::JgtImm),
            0x2d => Ok(Opcode::JgtReg),
            0x35 => Ok(Opcode::JgeImm),
            0x3d => Ok(Opcode::JgeReg),
            0xa5 => Ok(Opcode::JltImm),
            0xad => Ok(Opcode::JltReg),
            0xb5 => Ok(Opcode::JleImm),
            0xbd => Ok(Opcode::JleReg),
            0x45 => Ok(Opcode::JsetImm),
            0x4d => Ok(Opcode::JsetReg),
            0x55 => Ok(Opcode::JneImm),
            0x5d => Ok(Opcode::JneReg),
            0x65 => Ok(Opcode::JsgtImm),
            0x6d => Ok(Opcode::JsgtReg),
            0x75 => Ok(Opcode::JsgeImm),
            0x7d => Ok(Opcode::JsgeReg),
            0xc5 => Ok(Opcode::JsltImm),
            0xcd => Ok(Opcode::JsltReg),
            0xd5 => Ok(Opcode::JsleImm),
            0xdd => Ok(Opcode::JsleReg),
            0x85 => Ok(Opcode::Call),
            0x8d => Ok(Opcode::Callx),
            0x95 => Ok(Opcode::Exit),
            _ => Err(SBPFError::BytecodeError {
                error: format!("no decode handler for opcode 0x{:02x}", opcode),
                span: 0..1,
                custom_label: Some("Invalid opcode".to_string()),
            }),
        }
    }
}

impl From<Opcode> for u8 {
    fn from(opcode: Opcode) -> u8 {
        match opcode {
            Opcode::Lddw => 0x18,
            Opcode::Ldxb => 0x71,
            Opcode::Ldxh => 0x69,
            Opcode::Ldxw => 0x61,
            Opcode::Ldxdw => 0x79,
            Opcode::Stb => 0x72,
            Opcode::Sth => 0x6a,
            Opcode::Stw => 0x62,
            Opcode::Stdw => 0x7a,
            Opcode::Stxb => 0x73,
            Opcode::Stxh => 0x6b,
            Opcode::Stxw => 0x63,
            Opcode::Stxdw => 0x7b,
            Opcode::Add32Imm => 0x04,
            Opcode::Add32Reg => 0x0c,
            Opcode::Sub32Imm => 0x14,
            Opcode::Sub32Reg => 0x1c,
            Opcode::Mul32Imm => 0x24,
            Opcode::Mul32Reg => 0x2c,
            Opcode::Div32Imm => 0x34,
            Opcode::Div32Reg => 0x3c,
            Opcode::Or32Imm => 0x44,
            Opcode::Or32Reg => 0x4c,
            Opcode::And32Imm => 0x54,
            Opcode::And32Reg => 0x5c,
            Opcode::Lsh32Imm => 0x64,
            Opcode::Lsh32Reg => 0x6c,
            Opcode::Rsh32Imm => 0x74,
            Opcode::Rsh32Reg => 0x7c,
            Opcode::Mod32Imm => 0x94,
            Opcode::Mod32Reg => 0x9c,
            Opcode::Xor32Imm => 0xa4,
            Opcode::Xor32Reg => 0xac,
            Opcode::Mov32Imm => 0xb4,
            Opcode::Mov32Reg => 0xbc,
            Opcode::Arsh32Imm => 0xc4,
            Opcode::Arsh32Reg => 0xcc,
            Opcode::Lmul32Imm => 0x86,
            Opcode::Lmul32Reg => 0x8e,
            Opcode::Udiv32Imm => 0x46,
            Opcode::Udiv32Reg => 0x4e,
            Opcode::Urem32Imm => 0x66,
            Opcode::Urem32Reg => 0x6e,
            Opcode::Sdiv32Imm => 0xc6,
            Opcode::Sdiv32Reg => 0xce,
            Opcode::Srem32Imm => 0xe6,
            Opcode::Srem32Reg => 0xee,
            Opcode::Le => 0xd4,
            Opcode::Be => 0xdc,
            Opcode::Add64Imm => 0x07,
            Opcode::Add64Reg => 0x0f,
            Opcode::Sub64Imm => 0x17,
            Opcode::Sub64Reg => 0x1f,
            Opcode::Mul64Imm => 0x27,
            Opcode::Mul64Reg => 0x2f,
            Opcode::Div64Imm => 0x37,
            Opcode::Div64Reg => 0x3f,
            Opcode::Or64Imm => 0x47,
            Opcode::Or64Reg => 0x4f,
            Opcode::And64Imm => 0x57,
            Opcode::And64Reg => 0x5f,
            Opcode::Lsh64Imm => 0x67,
            Opcode::Lsh64Reg => 0x6f,
            Opcode::Rsh64Imm => 0x77,
            Opcode::Rsh64Reg => 0x7f,
            Opcode::Mod64Imm => 0x97,
            Opcode::Mod64Reg => 0x9f,
            Opcode::Xor64Imm => 0xa7,
            Opcode::Xor64Reg => 0xaf,
            Opcode::Mov64Imm => 0xb7,
            Opcode::Mov64Reg => 0xbf,
            Opcode::Arsh64Imm => 0xc7,
            Opcode::Arsh64Reg => 0xcf,
            Opcode::Hor64Imm => 0xf7,
            Opcode::Lmul64Imm => 0x96,
            Opcode::Lmul64Reg => 0x9e,
            Opcode::Uhmul64Imm => 0x36,
            Opcode::Uhmul64Reg => 0x3e,
            Opcode::Udiv64Imm => 0x56,
            Opcode::Udiv64Reg => 0x5e,
            Opcode::Urem64Imm => 0x76,
            Opcode::Urem64Reg => 0x7e,
            Opcode::Shmul64Imm => 0xb6,
            Opcode::Shmul64Reg => 0xbe,
            Opcode::Sdiv64Imm => 0xd6,
            Opcode::Sdiv64Reg => 0xde,
            Opcode::Srem64Imm => 0xf6,
            Opcode::Srem64Reg => 0xfe,
            Opcode::Neg32 => 0x84,
            Opcode::Neg64 => 0x87,
            Opcode::Ja => 0x05,
            Opcode::JeqImm => 0x15,
            Opcode::JeqReg => 0x1d,
            Opcode::JgtImm => 0x25,
            Opcode::JgtReg => 0x2d,
            Opcode::JgeImm => 0x35,
            Opcode::JgeReg => 0x3d,
            Opcode::JltImm => 0xa5,
            Opcode::JltReg => 0xad,
            Opcode::JleImm => 0xb5,
            Opcode::JleReg => 0xbd,
            Opcode::JsetImm => 0x45,
            Opcode::JsetReg => 0x4d,
            Opcode::JneImm => 0x55,
            Opcode::JneReg => 0x5d,
            Opcode::JsgtImm => 0x65,
            Opcode::JsgtReg => 0x6d,
            Opcode::JsgeImm => 0x75,
            Opcode::JsgeReg => 0x7d,
            Opcode::JsltImm => 0xc5,
            Opcode::JsltReg => 0xcd,
            Opcode::JsleImm => 0xd5,
            Opcode::JsleReg => 0xdd,
            Opcode::Call => 0x85,
            Opcode::Callx => 0x8d,
            Opcode::Exit => 0x95,
        }
    }
}

impl Opcode {
    pub fn to_str(&self) -> &'static str {
        match self {
            Opcode::Lddw => "lddw",
            Opcode::Ldxb => "ldxb",
            Opcode::Ldxh => "ldxh",
            Opcode::Ldxw => "ldxw",
            Opcode::Ldxdw => "ldxdw",
            Opcode::Stb => "stb",
            Opcode::Sth => "sth",
            Opcode::Stw => "stw",
            Opcode::Stdw => "stdw",
            Opcode::Stxb => "stxb",
            Opcode::Stxh => "stxh",
            Opcode::Stxw => "stxw",
            Opcode::Stxdw => "stxdw",
            Opcode::Add32Imm | Opcode::Add32Reg => "add32",
            Opcode::Sub32Imm | Opcode::Sub32Reg => "sub32",
            Opcode::Mul32Imm | Opcode::Mul32Reg => "mul32",
            Opcode::Div32Imm | Opcode::Div32Reg => "div32",
            Opcode::Or32Imm | Opcode::Or32Reg => "or32",
            Opcode::And32Imm | Opcode::And32Reg => "and32",
            Opcode::Lsh32Imm | Opcode::Lsh32Reg => "lsh32",
            Opcode::Rsh32Imm | Opcode::Rsh32Reg => "rsh32",
            Opcode::Neg32 => "neg32",
            Opcode::Mod32Imm | Opcode::Mod32Reg => "mod32",
            Opcode::Xor32Imm | Opcode::Xor32Reg => "xor32",
            Opcode::Mov32Imm | Opcode::Mov32Reg => "mov32",
            Opcode::Arsh32Imm | Opcode::Arsh32Reg => "arsh32",
            Opcode::Lmul32Imm | Opcode::Lmul32Reg => "lmul32",
            Opcode::Udiv32Imm | Opcode::Udiv32Reg => "udiv32",
            Opcode::Urem32Imm | Opcode::Urem32Reg => "urem32",
            Opcode::Sdiv32Imm | Opcode::Sdiv32Reg => "sdiv32",
            Opcode::Srem32Imm | Opcode::Srem32Reg => "srem32",
            Opcode::Le => "le",
            Opcode::Be => "be",
            Opcode::Add64Imm | Opcode::Add64Reg => "add64",
            Opcode::Sub64Imm | Opcode::Sub64Reg => "sub64",
            Opcode::Mul64Imm | Opcode::Mul64Reg => "mul64",
            Opcode::Div64Imm | Opcode::Div64Reg => "div64",
            Opcode::Or64Imm | Opcode::Or64Reg => "or64",
            Opcode::And64Imm | Opcode::And64Reg => "and64",
            Opcode::Lsh64Imm | Opcode::Lsh64Reg => "lsh64",
            Opcode::Rsh64Imm | Opcode::Rsh64Reg => "rsh64",
            Opcode::Neg64 => "neg64",
            Opcode::Mod64Imm | Opcode::Mod64Reg => "mod64",
            Opcode::Xor64Imm | Opcode::Xor64Reg => "xor64",
            Opcode::Mov64Imm | Opcode::Mov64Reg => "mov64",
            Opcode::Arsh64Imm | Opcode::Arsh64Reg => "arsh64",
            Opcode::Hor64Imm => "hor64",
            Opcode::Lmul64Imm | Opcode::Lmul64Reg => "lmul64",
            Opcode::Uhmul64Imm | Opcode::Uhmul64Reg => "uhmul64",
            Opcode::Udiv64Imm | Opcode::Udiv64Reg => "udiv64",
            Opcode::Urem64Imm | Opcode::Urem64Reg => "urem64",
            Opcode::Shmul64Imm | Opcode::Shmul64Reg => "shmul64",
            Opcode::Sdiv64Imm | Opcode::Sdiv64Reg => "sdiv64",
            Opcode::Srem64Imm | Opcode::Srem64Reg => "srem64",
            Opcode::Ja => "ja",
            Opcode::JeqImm | Opcode::JeqReg => "jeq",
            Opcode::JgtImm | Opcode::JgtReg => "jgt",
            Opcode::JgeImm | Opcode::JgeReg => "jge",
            Opcode::JltImm | Opcode::JltReg => "jlt",
            Opcode::JleImm | Opcode::JleReg => "jle",
            Opcode::JsetImm | Opcode::JsetReg => "jset",
            Opcode::JneImm | Opcode::JneReg => "jne",
            Opcode::JsgtImm | Opcode::JsgtReg => "jsgt",
            Opcode::JsgeImm | Opcode::JsgeReg => "jsge",
            Opcode::JsltImm | Opcode::JsltReg => "jslt",
            Opcode::JsleImm | Opcode::JsleReg => "jsle",
            Opcode::Call => "call",
            Opcode::Callx => "callx",
            Opcode::Exit => "exit",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        for &op in LOAD_MEMORY_OPS {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_all_bin_imm_ops() {
        for &op in BIN_IMM_OPS {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_all_jump_imm_ops() {
        for &op in JUMP_IMM_OPS {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_all_store_imm_ops() {
        for &op in STORE_IMM_OPS {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_all_store_reg_ops() {
        for &op in STORE_REG_OPS {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_all_bin_reg_ops() {
        for &op in BIN_REG_OPS {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_all_unary_ops() {
        for &op in UNARY_OPS {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_all_jump_ops() {
        for &op in JUMP_OPS {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_all_jump_reg_ops() {
        for &op in JUMP_REG_OPS {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_all_call_ops() {
        for &op in CALL_IMM_OPS {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
        for &op in CALL_REG_OPS {
            let byte: u8 = op.into();
            let roundtrip = Opcode::try_from(byte).unwrap();
            assert_eq!(roundtrip, op);
        }
    }

    #[test]
    fn test_exit_op() {
        for &op in EXIT_OPS {
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
    }

    #[test]
    fn test_to_str_call_and_exit_ops() {
        assert_eq!(Opcode::Call.to_str(), "call");
        assert_eq!(Opcode::Callx.to_str(), "callx");
        assert_eq!(Opcode::Exit.to_str(), "exit");
    }
}
