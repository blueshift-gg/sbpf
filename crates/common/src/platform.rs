/// Platform-specific BPF encoding/decoding
pub trait BPFPlatform {
    /// Transform callx instruction when encoding for this platform
    /// Returns (dst, imm) values adjusted for platform conventions
    ///
    /// BPF standard: callx uses dst register, imm is 0
    /// SBPF: callx uses imm field for register, dst is 0
    fn encode_callx(dst: u8, imm: i32) -> (u8, i32);

    /// Transform callx instruction when decoding from this platform
    /// Returns (dst, imm) in standard BPF convention
    fn decode_callx(dst: u8, imm: i32) -> (u8, i32);
}

pub struct SbpfV0;
pub struct Bpf;

impl BPFPlatform for SbpfV0 {
    /// SBPF encodes callx with register in imm field, dst=0
    fn encode_callx(dst: u8, _imm: i32) -> (u8, i32) {
        (0, dst as i32)
    }

    /// SBPF decodes callx by moving imm to dst
    fn decode_callx(_dst: u8, imm: i32) -> (u8, i32) {
        (imm as u8, 0)
    }
}

impl BPFPlatform for Bpf {
    /// Standard BPF uses dst register, imm=0
    fn encode_callx(dst: u8, imm: i32) -> (u8, i32) {
        (dst, imm)
    }

    /// Standard BPF already in correct format
    fn decode_callx(dst: u8, imm: i32) -> (u8, i32) {
        (dst, imm)
    }
}