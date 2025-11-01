/// Platform-specific BPF encoding/decoding
pub trait BpfPlatform {
    /// Transform callx instruction when encoding for this platform
    /// Returns (dst, imm) values adjusted for platform conventions
    ///
    /// BPF standard: callx uses dst register, imm is 0
    /// SBPF: callx uses imm field for register, dst is 0
    ///
    /// Default implementation uses standard BPF convention (no transformation)
    fn encode_callx(dst: u8, imm: i32) -> (u8, i32) {
        (dst, imm)
    }

    /// Transform callx instruction when decoding from this platform
    /// Returns (dst, imm) in standard BPF convention
    ///
    /// Default implementation uses standard BPF convention (no transformation)
    fn decode_callx(dst: u8, imm: i32) -> (u8, i32) {
        (dst, imm)
    }
}

pub struct SbpfV0;
pub struct Bpf;

impl BpfPlatform for SbpfV0 {
    /// SBPF encodes callx with register in imm field, dst=0
    fn encode_callx(dst: u8, _imm: i32) -> (u8, i32) {
        (0, dst as i32)
    }

    /// SBPF decodes callx by moving imm to dst
    fn decode_callx(_dst: u8, imm: i32) -> (u8, i32) {
        (imm as u8, 0)
    }
}

impl BpfPlatform for Bpf {
    // Uses default implementation (standard BPF convention)
}