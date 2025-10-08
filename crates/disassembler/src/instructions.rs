use std::io::Cursor;

use serde::{Deserialize, Serialize};

use crate::{cursor::ELFCursor, errors::EZBpfError, opcodes::OpCode};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Ix {
    pub op: OpCode,
    pub dst: u8,
    pub src: u8,
    pub off: i16,
    pub imm: i64,
}

impl Ix {
    pub fn off_str(&self) -> String {
        match self.off.is_negative() {
            true => self.off.to_string(),
            false => format!("+{}", self.off),
        }
    }

    pub fn dst_off(&self) -> String {
        format!("[r{}{}]", self.dst, self.off_str())
    }

    pub fn src_off(&self) -> String {
        format!("[r{}{}]", self.src, self.off_str())
    }

    pub fn op_imm_bits(&self) -> Result<String, EZBpfError> {
        Ok(match self.imm {
            16 => format!("{}16", self.op),
            32 => format!("{}32", self.op),
            64 => format!("{}64", self.op),
            _ => return Err(EZBpfError::InvalidImmediate),
        })
    }
}

impl Ix {
    pub fn from_bytes(b: &[u8]) -> Result<Self, EZBpfError> {
        let mut c = Cursor::new(b);
        c.read_ix()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = vec![self.op.clone() as u8, self.src << 4 | self.dst];
        b.extend_from_slice(&self.off.to_le_bytes());
        b.extend_from_slice(&self.imm.to_le_bytes()[..4]);
        if self.op == OpCode::Lddw {
            b.extend_from_slice(&[0; 4]);
            b.extend_from_slice(&self.imm.to_le_bytes()[4..]);
        }
        b
    }

    pub fn to_asm(&self) -> Result<String, EZBpfError> {
        Ok(match self.op {
            // lddw - (load double word) takes up two instructions. The 64 bit value
            // is made up of two halves with the upper half being the immediate
            // of the lddw value and the lower half being the immediate of the
            // following instruction
            OpCode::Lddw => format!("{} r{}, {}", self.op, self.dst, self.imm),
            // ldx - (load x) store a 8/16/32/64 bit (byte/half/word/double word)
            // value in a register
            OpCode::Ldxb |
            OpCode::Ldxh |
            OpCode::Ldxw |
            OpCode::Ldxdw => format!("{} r{}, {}", self.op, self.dst, self.src_off()),
            // stb - these instructions are deprecated
            OpCode::Stb |
            OpCode::Sth |
            OpCode::Stw |
            OpCode::Stdw => format!("{} {}, {}", self.op, self.dst_off(), self.imm),
            // stx - store a 8/16/32/64 bit value from a source register into the offset
            // of the destination register
            OpCode::Stxb |
            OpCode::Stxh |
            OpCode::Stxw |
            OpCode::Stxdw => format!("{} {}, r{}", self.op, self.dst_off(), self.src),
            // Math
            OpCode::Neg32 | // Deprecated in SBFv2
            OpCode::Neg64 => format!("{} r{}", self.op, self.dst),
            // LE and BE OpCodes act a little differently to others. In assembly form, they are
            // notated as be16, be32 and b64. In byte form, the bit length of the operation is 
            // determined by the immedate value of its parent instruction, 0x10, 0x20 and 0x40
            // accordingly (the hex of 16/32/64)
            OpCode::Le |
            OpCode::Be => format!("{}{}", self.op_imm_bits()?, self.dst), // Docs for this seem wrong //DC01000010000000 DC01000020000000 DC01000040000000
            // Immedate
            OpCode::Add32Imm |
            OpCode::Sub32Imm |
            OpCode::Mul32Imm |
            OpCode::Div32Imm |
            OpCode::Or32Imm |
            OpCode::And32Imm |
            OpCode::Lsh32Imm |
            OpCode::Rsh32Imm |
            OpCode::Mod32Imm |
            OpCode::Xor32Imm |
            OpCode::Arsh32Imm |
            OpCode::Mov32Imm |
            OpCode::Lmul32Imm |
            OpCode::Udiv32Imm |
            OpCode::Urem32Imm |
            OpCode::Sdiv32Imm |
            OpCode::Srem32Imm |
            OpCode::Add64Imm |
            OpCode::Sub64Imm |
            OpCode::Mul64Imm |
            OpCode::Div64Imm |
            OpCode::Or64Imm |
            OpCode::And64Imm |
            OpCode::Lsh64Imm |
            OpCode::Rsh64Imm |
            OpCode::Mod64Imm |
            OpCode::Xor64Imm |
            OpCode::Mov64Imm |
            OpCode::Arsh64Imm |
            OpCode::Hor64Imm |
            OpCode::Lmul64Imm |
            OpCode::Uhmul64Imm |
            OpCode::Udiv64Imm |
            OpCode::Urem64Imm |
            OpCode::Shmul64Imm |
            OpCode::Sdiv64Imm |
            OpCode::Srem64Imm => format!("{} r{}, {}", self.op, self.dst, self.imm),
            // Register
            OpCode::Add32Reg |
            OpCode::Sub32Reg |
            OpCode::Mul32Reg |
            OpCode::Div32Reg |
            OpCode::Or32Reg |
            OpCode::And32Reg |
            OpCode::Lsh32Reg |
            OpCode::Rsh32Reg |
            OpCode::Mod32Reg |
            OpCode::Xor32Reg |
            OpCode::Mov32Reg |
            OpCode::Arsh32Reg |
            OpCode::Lmul32Reg |
            OpCode::Udiv32Reg |
            OpCode::Urem32Reg |
            OpCode::Sdiv32Reg |
            OpCode::Srem32Reg |
            OpCode::Add64Reg |
            OpCode::Sub64Reg |
            OpCode::Mul64Reg |
            OpCode::Div64Reg |
            OpCode::Or64Reg |
            OpCode::And64Reg |
            OpCode::Lsh64Reg |
            OpCode::Rsh64Reg |
            OpCode::Mod64Reg |
            OpCode::Xor64Reg |
            OpCode::Mov64Reg |
            OpCode::Arsh64Reg |
            OpCode::Lmul64Reg |
            OpCode::Uhmul64Reg |
            OpCode::Udiv64Reg |
            OpCode::Urem64Reg |
            OpCode::Shmul64Reg |
            OpCode::Sdiv64Reg |
            OpCode::Srem64Reg => format!("{} r{}, r{}", self.op, self.dst, self.src),

            // Jumps
            OpCode::Ja => format!("{} {}", self.op, self.off_str()),

            // Immediates
            OpCode::JeqImm |
            OpCode::JgtImm |
            OpCode::JgeImm |
            OpCode::JltImm |
            OpCode::JleImm |
            OpCode::JsetImm |
            OpCode::JneImm |
            OpCode::JsgtImm |
            OpCode::JsgeImm |
            OpCode::JsltImm |
            OpCode::JsleImm => format!("{} r{}, {}, {}", self.op, self.dst, self.imm, self.off_str()),
            // Registers
            OpCode::JeqReg |
            OpCode::JgtReg |
            OpCode::JgeReg |
            OpCode::JltReg |
            OpCode::JleReg |
            OpCode::JsetReg |
            OpCode::JneReg |
            OpCode::JsgtReg |
            OpCode::JsgeReg |
            OpCode::JsltReg |
            OpCode::JsleReg => format!("{} r{}, r{}, {}", self.op, self.dst, self.src, self.off_str()),


            // Calls
            OpCode::Call => format!("call {}", self.imm),
            OpCode::Callx => format!("call r{}", self.src),
            OpCode::Exit => format!("{}", self.op),
        })
    }
}

#[cfg(test)]
mod test {
    use hex_literal::hex;

    use crate::instructions::Ix;

    #[test]
    fn serialize_e2e() {
        let b = hex!("9700000000000000");
        let i = Ix::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes(), &b);
    }

    #[test]
    fn serialize_e2e_lddw() {
        let b = hex!("18010000000000000000000000000000");
        let i = Ix::from_bytes(&b).unwrap();
        assert_eq!(i.to_bytes(), &b);
    }
}
