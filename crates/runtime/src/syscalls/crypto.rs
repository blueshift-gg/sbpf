use {
    crate::config::ExecutionCost,
    blake3::Hasher as Blake3Hasher,
    sbpf_vm::{
        compute::ComputeMeter,
        errors::{SbpfVmError, SbpfVmResult},
        memory::Memory,
    },
    sha2::Sha256,
    sha3::Keccak256,
};

trait Hasher {
    fn new() -> Self;
    fn update(&mut self, data: &[u8]);
    fn finalize(self) -> Vec<u8>;
}

impl Hasher for Sha256 {
    fn new() -> Self {
        sha2::Digest::new()
    }
    fn update(&mut self, data: &[u8]) {
        sha2::Digest::update(self, data);
    }
    fn finalize(self) -> Vec<u8> {
        sha2::Digest::finalize(self).to_vec()
    }
}

impl Hasher for Keccak256 {
    fn new() -> Self {
        sha3::Digest::new()
    }
    fn update(&mut self, data: &[u8]) {
        sha3::Digest::update(self, data);
    }
    fn finalize(self) -> Vec<u8> {
        sha3::Digest::finalize(self).to_vec()
    }
}

impl Hasher for Blake3Hasher {
    fn new() -> Self {
        blake3::Hasher::new()
    }
    fn update(&mut self, data: &[u8]) {
        blake3::Hasher::update(self, data);
    }
    fn finalize(self) -> Vec<u8> {
        blake3::Hasher::finalize(&self).as_bytes().to_vec()
    }
}

fn read_slices(memory: &Memory, vals_addr: u64, vals_len: u64) -> SbpfVmResult<Vec<(u64, u64)>> {
    let mut slices = Vec::with_capacity(vals_len as usize);
    for i in 0..vals_len {
        let slice_addr = vals_addr.saturating_add(i.saturating_mul(16));
        let ptr = memory.read_u64(slice_addr)?;
        let len = memory.read_u64(slice_addr.saturating_add(8))?;
        slices.push((ptr, len));
    }
    Ok(slices)
}

fn hash_slices<H: Hasher>(
    memory: &mut Memory,
    compute: &ComputeMeter,
    costs: &ExecutionCost,
    vals_addr: u64,
    vals_len: u64,
    result_addr: u64,
) -> SbpfVmResult<u64> {
    if vals_len > costs.sha256_max_slices {
        return Err(SbpfVmError::TooManySlices);
    }

    compute.consume(costs.sha256_base_cost)?;

    let mut hasher = H::new();
    if vals_len > 0 {
        for (ptr, len) in read_slices(memory, vals_addr, vals_len)? {
            let cost = costs
                .mem_op_base_cost
                .max(costs.sha256_byte_cost.saturating_mul(len / 2));
            compute.consume(cost)?;
            hasher.update(memory.read_bytes(ptr, len as usize)?);
        }
    }

    memory.write_bytes(result_addr, &hasher.finalize())?;
    Ok(0)
}

pub fn sol_sha256(
    registers: [u64; 5],
    memory: &mut Memory,
    compute: &ComputeMeter,
    costs: &ExecutionCost,
) -> SbpfVmResult<u64> {
    hash_slices::<Sha256>(
        memory,
        compute,
        costs,
        registers[0],
        registers[1],
        registers[2],
    )
}

pub fn sol_keccak256(
    registers: [u64; 5],
    memory: &mut Memory,
    compute: &ComputeMeter,
    costs: &ExecutionCost,
) -> SbpfVmResult<u64> {
    hash_slices::<Keccak256>(
        memory,
        compute,
        costs,
        registers[0],
        registers[1],
        registers[2],
    )
}

pub fn sol_blake3(
    registers: [u64; 5],
    memory: &mut Memory,
    compute: &ComputeMeter,
    costs: &ExecutionCost,
) -> SbpfVmResult<u64> {
    hash_slices::<Blake3Hasher>(
        memory,
        compute,
        costs,
        registers[0],
        registers[1],
        registers[2],
    )
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::syscalls::tests::test_helpers::{costs, make_memory, meter},
        sbpf_vm::{errors::SbpfVmError, memory::Memory},
    };

    fn setup_single_slice(memory: &mut Memory, data: &[u8]) -> (u64, u64) {
        let data_addr = Memory::HEAP_START;
        memory.write_bytes(data_addr, data).unwrap();

        let slices_addr = Memory::HEAP_START + 64;
        memory.write_u64(slices_addr, data_addr).unwrap();
        memory
            .write_u64(slices_addr + 8, data.len() as u64)
            .unwrap();

        let result_addr = Memory::HEAP_START + 128;
        (slices_addr, result_addr)
    }

    fn reference_sha256(data: &[u8]) -> Vec<u8> {
        use sha2::Digest;
        sha2::Sha256::digest(data).to_vec()
    }

    fn reference_keccak256(data: &[u8]) -> Vec<u8> {
        use sha3::Digest;
        sha3::Keccak256::digest(data).to_vec()
    }

    fn reference_blake3(data: &[u8]) -> Vec<u8> {
        blake3::hash(data).as_bytes().to_vec()
    }

    #[test]
    fn test_sha256_known_input() {
        let mut memory = make_memory();
        let (slices_addr, result_addr) = setup_single_slice(&mut memory, b"hello");

        let registers = [slices_addr, 1, result_addr, 0, 0];
        sol_sha256(registers, &mut memory, &meter(1_000_000), &costs()).unwrap();

        let result = memory.read_bytes(result_addr, 32).unwrap();
        assert_eq!(result, reference_sha256(b"hello").as_slice());
    }

    #[test]
    fn test_sha256_empty_slices() {
        let mut memory = make_memory();
        let result_addr = Memory::HEAP_START + 128;

        let registers = [0, 0, result_addr, 0, 0];
        sol_sha256(registers, &mut memory, &meter(1_000_000), &costs()).unwrap();

        let result = memory.read_bytes(result_addr, 32).unwrap();
        assert_eq!(result, reference_sha256(b"").as_slice());
    }

    #[test]
    fn test_sha256_multiple_slices_concatenated() {
        let mut memory = make_memory();
        memory.write_bytes(Memory::HEAP_START, b"he").unwrap();
        memory.write_bytes(Memory::HEAP_START + 8, b"llo").unwrap();

        let slices_addr = Memory::HEAP_START + 64;
        memory.write_u64(slices_addr, Memory::HEAP_START).unwrap();
        memory.write_u64(slices_addr + 8, 2).unwrap();
        memory
            .write_u64(slices_addr + 16, Memory::HEAP_START + 8)
            .unwrap();
        memory.write_u64(slices_addr + 24, 3).unwrap();

        let result_addr = Memory::HEAP_START + 128;
        let registers = [slices_addr, 2, result_addr, 0, 0];
        sol_sha256(registers, &mut memory, &meter(1_000_000), &costs()).unwrap();

        let result = memory.read_bytes(result_addr, 32).unwrap();
        assert_eq!(result, reference_sha256(b"hello").as_slice());
    }

    #[test]
    fn test_sha256_too_many_slices() {
        let mut memory = make_memory();
        let result_addr = Memory::HEAP_START + 128;
        let registers = [Memory::HEAP_START, 20_001, result_addr, 0, 0];
        assert!(matches!(
            sol_sha256(registers, &mut memory, &meter(1_000_000), &costs()),
            Err(SbpfVmError::TooManySlices)
        ));
    }

    #[test]
    fn test_sha256_compute_exhausted() {
        let mut memory = make_memory();
        let result_addr = Memory::HEAP_START + 128;
        let registers = [0, 0, result_addr, 0, 0];
        assert!(matches!(
            sol_sha256(registers, &mut memory, &meter(84), &costs()),
            Err(SbpfVmError::ComputeBudgetExceeded { .. })
        ));
    }

    #[test]
    fn test_keccak256_known_input() {
        let mut memory = make_memory();
        let (slices_addr, result_addr) = setup_single_slice(&mut memory, b"hello");

        let registers = [slices_addr, 1, result_addr, 0, 0];
        sol_keccak256(registers, &mut memory, &meter(1_000_000), &costs()).unwrap();

        let result = memory.read_bytes(result_addr, 32).unwrap();
        assert_eq!(result, reference_keccak256(b"hello").as_slice());
    }

    #[test]
    fn test_keccak256_empty_slices() {
        let mut memory = make_memory();
        let result_addr = Memory::HEAP_START + 128;
        let registers = [0, 0, result_addr, 0, 0];
        sol_keccak256(registers, &mut memory, &meter(1_000_000), &costs()).unwrap();

        let result = memory.read_bytes(result_addr, 32).unwrap();
        assert_eq!(result, reference_keccak256(b"").as_slice());
    }

    #[test]
    fn test_keccak256_differs_from_sha256() {
        let mut memory = make_memory();
        let (slices_addr, result_addr) = setup_single_slice(&mut memory, b"hello");

        let registers = [slices_addr, 1, result_addr, 0, 0];
        sol_keccak256(registers, &mut memory, &meter(1_000_000), &costs()).unwrap();

        let keccak_result = memory.read_bytes(result_addr, 32).unwrap().to_vec();
        assert_ne!(keccak_result, reference_sha256(b"hello"));
    }

    #[test]
    fn test_blake3_known_input() {
        let mut memory = make_memory();
        let (slices_addr, result_addr) = setup_single_slice(&mut memory, b"hello");

        let registers = [slices_addr, 1, result_addr, 0, 0];
        sol_blake3(registers, &mut memory, &meter(1_000_000), &costs()).unwrap();

        let result = memory.read_bytes(result_addr, 32).unwrap();
        assert_eq!(result, reference_blake3(b"hello").as_slice());
    }

    #[test]
    fn test_blake3_empty_slices() {
        let mut memory = make_memory();
        let result_addr = Memory::HEAP_START + 128;
        let registers = [0, 0, result_addr, 0, 0];
        sol_blake3(registers, &mut memory, &meter(1_000_000), &costs()).unwrap();

        let result = memory.read_bytes(result_addr, 32).unwrap();
        assert_eq!(result, reference_blake3(b"").as_slice());
    }

    #[test]
    fn test_all_three_hashes_differ_on_same_input() {
        let mut memory = make_memory();

        let (slices_addr, result_addr) = setup_single_slice(&mut memory, b"test");
        let registers = [slices_addr, 1, result_addr, 0, 0];

        sol_sha256(registers, &mut memory, &meter(1_000_000), &costs()).unwrap();
        let sha_out = memory.read_bytes(result_addr, 32).unwrap().to_vec();

        sol_keccak256(registers, &mut memory, &meter(1_000_000), &costs()).unwrap();
        let keccak_out = memory.read_bytes(result_addr, 32).unwrap().to_vec();

        sol_blake3(registers, &mut memory, &meter(1_000_000), &costs()).unwrap();
        let blake_out = memory.read_bytes(result_addr, 32).unwrap().to_vec();

        assert_ne!(sha_out, keccak_out);
        assert_ne!(sha_out, blake_out);
        assert_ne!(keccak_out, blake_out);
    }
}
