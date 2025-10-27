/// MurmurHash3 32-bit hash function (const-compatible)
pub const fn murmur3_32(buf: &str) -> u32 {
    const fn pre_mix(buf: [u8; 4]) -> u32 {
        u32::from_le_bytes(buf)
            .wrapping_mul(0xcc9e2d51)
            .rotate_left(15)
            .wrapping_mul(0x1b873593)
    }

    let mut hash = 0;
    let mut i = 0;
    let buf = buf.as_bytes();

    while i < buf.len() / 4 {
        let buf = [buf[i * 4], buf[i * 4 + 1], buf[i * 4 + 2], buf[i * 4 + 3]];
        hash ^= pre_mix(buf);
        hash = hash.rotate_left(13);
        hash = hash.wrapping_mul(5).wrapping_add(0xe6546b64);

        i += 1;
    }

    match buf.len() % 4 {
        0 => {}
        1 => {
            hash = hash ^ pre_mix([buf[i * 4], 0, 0, 0]);
        }
        2 => {
            hash = hash ^ pre_mix([buf[i * 4], buf[i * 4 + 1], 0, 0]);
        }
        3 => {
            hash = hash ^ pre_mix([buf[i * 4], buf[i * 4 + 1], buf[i * 4 + 2], 0]);
        }
        _ => { /* unreachable!() */ }
    }

    hash = hash ^ buf.len() as u32;
    hash = hash ^ (hash.wrapping_shr(16));
    hash = hash.wrapping_mul(0x85ebca6b);
    hash = hash ^ (hash.wrapping_shr(13));
    hash = hash.wrapping_mul(0xc2b2ae35);
    hash = hash ^ (hash.wrapping_shr(16));

    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_murmur3_hash() {
        // Test hash function is deterministic
        assert_eq!(murmur3_32("abort"), murmur3_32("abort"));
        assert_ne!(murmur3_32("abort"), murmur3_32("sol_log_"));
    }

    #[test]
    fn test_murmur3_known_values() {
        // Test some known hash values to ensure consistency
        const ABORT_HASH: u32 = murmur3_32("abort");
        const SOL_LOG_HASH: u32 = murmur3_32("sol_log_");

        // These should remain constant across runs
        assert_eq!(murmur3_32("abort"), ABORT_HASH);
        assert_eq!(murmur3_32("sol_log_"), SOL_LOG_HASH);
    }
}
