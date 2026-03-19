use {
    crate::config::ExecutionCost,
    blake3::Hasher as Blake3Hasher,
    sbpf_vm::{
        compute::ComputeMeter,
        errors::{SbpfVmError, SbpfVmResult},
        memory::Memory,
    },
    sha2::{Digest, Sha256},
    sha3::Keccak256,
};

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

fn hash_slices<H: Digest>(
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
