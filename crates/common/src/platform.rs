use crate::{errors::SBPFError, opcode::Opcode};

/// Platform-specific BPF encoding/decoding
pub trait BpfPlatform {
    /// Transform raw instruction bytes during deserialization
    /// Returns (opcode, dst, src, off, imm) tuple
    ///
    /// This allows platforms to:
    /// - Remap raw opcode bytes to different opcodes
    /// - Transform instruction parameters (registers, offsets, immediates)
    /// - Handle platform-specific encodings
    ///
    /// Default implementation: parse standard BPF format with no transformation
    fn decode_instruction(bytes: &[u8]) -> Result<(Opcode, u8, u8, i16, i32), SBPFError> {
        if bytes.len() < 8 {
            return Err(SBPFError::BytecodeError {
                error: "Instruction must be at least 8 bytes".to_string(),
                span: 0..bytes.len(),
                custom_label: Some("Invalid instruction length".to_string()),
            });
        }

        let opcode: Opcode = bytes[0].try_into()?;
        let dst = bytes[1] & 0x0F;
        let src = (bytes[1] >> 4) & 0x0F;
        let off = i16::from_le_bytes([bytes[2], bytes[3]]);
        let imm = i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);

        Ok((opcode, dst, src, off, imm))
    }

    /// Transform instruction components during serialization
    /// Returns (raw_opcode, dst, src, off, imm) tuple
    ///
    /// This is the inverse of decode_instruction, allowing platforms to
    /// encode instructions in their specific format.
    ///
    /// Default implementation: standard BPF format with no transformation
    fn encode_instruction(opcode: Opcode, dst: u8, src: u8, off: i16, imm: i32) -> (u8, u8, u8, i16, i32) {
        (opcode.into(), dst, src, off, imm)
    }
}

pub struct SbpfV0;
pub struct SbpfV2;
pub struct Bpf;

impl BpfPlatform for SbpfV0 {
    /// SBPF V0: callx uses imm field for register instead of dst
    fn decode_instruction(bytes: &[u8]) -> Result<(Opcode, u8, u8, i16, i32), SBPFError> {
        if bytes.len() < 8 {
            return Err(SBPFError::BytecodeError {
                error: "Instruction must be at least 8 bytes".to_string(),
                span: 0..bytes.len(),
                custom_label: Some("Invalid instruction length".to_string()),
            });
        }

        let opcode: Opcode = bytes[0].try_into()?;
        let mut dst = bytes[1] & 0x0F;
        let src = (bytes[1] >> 4) & 0x0F;
        let off = i16::from_le_bytes([bytes[2], bytes[3]]);
        let mut imm = i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);

        // SBPF callx: register in imm field, move to dst
        if opcode == Opcode::Callx {
            dst = imm as u8;
            imm = 0;
        }

        Ok((opcode, dst, src, off, imm))
    }

    fn encode_instruction(opcode: Opcode, mut dst: u8, src: u8, off: i16, mut imm: i32) -> (u8, u8, u8, i16, i32) {
        // SBPF callx: register in imm field instead of dst
        if opcode == Opcode::Callx {
            imm = dst as i32;
            dst = 0;
        }

        (opcode.into(), dst, src, off, imm)
    }
}

impl BpfPlatform for SbpfV2 {
    /// SBPF V2: Includes callx transformation from V0 plus additional opcode translations
    fn decode_instruction(bytes: &[u8]) -> Result<(Opcode, u8, u8, i16, i32), SBPFError> {
        if bytes.len() < 8 {
            return Err(SBPFError::BytecodeError {
                error: "Instruction must be at least 8 bytes".to_string(),
                span: 0..bytes.len(),
                custom_label: Some("Invalid instruction length".to_string()),
            });
        }

        let raw_opcode = bytes[0];
        let mut dst = bytes[1] & 0x0F;
        let src = (bytes[1] >> 4) & 0x0F;
        let off = i16::from_le_bytes([bytes[2], bytes[3]]);
        let mut imm = i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);

        // First, handle new opcode mappings (raw byte -> opcode)
        let mut opcode = match raw_opcode {
            0x8C => Opcode::Ldxw,
            0x8F => Opcode::Stxw,
            0xF7 => Opcode::Hor64Imm,
            _ => raw_opcode.try_into()?,
        };

        // Then apply opcode translations (opcode -> different opcode)
        opcode = match opcode {
            Opcode::Mul32Reg => Opcode::Ldxb,
            Opcode::Div32Reg => Opcode::Ldxh,
            Opcode::Mod32Reg => Opcode::Ldxdw,
            Opcode::Mul64Imm => Opcode::Stb,
            Opcode::Mul64Reg => Opcode::Stxb,
            Opcode::Div64Imm => Opcode::Sth,
            Opcode::Div64Reg => Opcode::Stxh,
            Opcode::Neg64 => Opcode::Stw,
            Opcode::Mod64Imm => Opcode::Stdw,
            Opcode::Mod64Reg => Opcode::Stxdw,
            _ => opcode,
        };

        // Finally, apply callx transformation (same as V0)
        if opcode == Opcode::Callx {
            dst = imm as u8;
            imm = 0;
        }

        Ok((opcode, dst, src, off, imm))
    }

    fn encode_instruction(opcode: Opcode, mut dst: u8, src: u8, off: i16, mut imm: i32) -> (u8, u8, u8, i16, i32) {
        // Apply reverse translations (opcode -> raw opcode for SBPF V2)
        let raw_opcode = match opcode {
            Opcode::Ldxb => 0x2c,  // mul32 reg
            Opcode::Ldxh => 0x3c,  // div32 reg
            Opcode::Ldxdw => 0x9c, // mod32 reg
            Opcode::Stb => 0x27,   // mul64 imm
            Opcode::Stxb => 0x2f,  // mul64 reg
            Opcode::Sth => 0x37,   // div64 imm
            Opcode::Stxh => 0x3f,  // div64 reg
            Opcode::Stw => 0x87,   // neg64
            Opcode::Stdw => 0x97,  // mod64 imm
            Opcode::Stxdw => 0x9f, // mod64 reg
            Opcode::Hor64Imm => 0xF7,
            // Handle reverse of new opcode mappings
            Opcode::Ldxw if src != 0 => 0x8C,  // Special SBPF V2 encoding
            Opcode::Stxw if src != 0 => 0x8F,  // Special SBPF V2 encoding
            _ => {
                // Apply callx transformation (same as V0)
                if opcode == Opcode::Callx {
                    imm = dst as i32;
                    dst = 0;
                }
                opcode.into()
            }
        };

        // Apply callx transformation if not already handled
        if opcode == Opcode::Callx {
            imm = dst as i32;
            dst = 0;
        }

        (raw_opcode, dst, src, off, imm)
    }
}

impl BpfPlatform for Bpf {
    // Uses default implementation (standard BPF convention)
}