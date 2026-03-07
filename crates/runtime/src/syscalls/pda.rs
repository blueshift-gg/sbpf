use {
    crate::config::ExecutionCost,
    sbpf_vm::{
        compute::ComputeMeter,
        errors::{SbpfVmError, SbpfVmResult},
        memory::Memory,
    },
    solana_address::Address,
};

const MAX_SEED_LEN: usize = 32;
const MAX_SEEDS: usize = 16;

fn read_seeds(memory: &Memory, seeds_addr: u64, seeds_len: u64) -> SbpfVmResult<Vec<Vec<u8>>> {
    if seeds_len as usize > MAX_SEEDS {
        return Err(SbpfVmError::MaxSeedLengthExceeded);
    }

    let mut seeds = Vec::with_capacity(seeds_len as usize);
    for i in 0..seeds_len {
        let slice_addr = seeds_addr.saturating_add(i.saturating_mul(16));
        let ptr = memory.read_u64(slice_addr)?;
        let len = memory.read_u64(slice_addr.saturating_add(8))?;

        if len as usize > MAX_SEED_LEN {
            return Err(SbpfVmError::MaxSeedLengthExceeded);
        }

        seeds.push(memory.read_bytes(ptr, len as usize)?.to_vec());
    }
    Ok(seeds)
}

pub fn sol_create_program_address(
    registers: [u64; 5],
    memory: &mut Memory,
    compute: &ComputeMeter,
    costs: &ExecutionCost,
) -> SbpfVmResult<u64> {
    let seeds_addr = registers[0];
    let seeds_len = registers[1];
    let program_id_addr = registers[2];
    let address_addr = registers[3];

    compute.consume(costs.create_program_address_units)?;

    let seeds = read_seeds(memory, seeds_addr, seeds_len)?;
    let program_id = Address::from(
        <[u8; 32]>::try_from(memory.read_bytes(program_id_addr, 32)?)
            .map_err(|_| SbpfVmError::InvalidSliceConversion)?,
    );

    let seed_slices: Vec<&[u8]> = seeds.iter().map(|s| s.as_slice()).collect();
    match Address::create_program_address(&seed_slices, &program_id) {
        Ok(addr) => {
            memory.write_bytes(address_addr, addr.as_ref())?;
            Ok(0)
        }
        Err(_) => Ok(1),
    }
}

pub fn sol_try_find_program_address(
    registers: [u64; 5],
    memory: &mut Memory,
    compute: &ComputeMeter,
    costs: &ExecutionCost,
) -> SbpfVmResult<u64> {
    let seeds_addr = registers[0];
    let seeds_len = registers[1];
    let program_id_addr = registers[2];
    let address_addr = registers[3];
    let bump_seed_addr = registers[4];

    compute.consume(costs.create_program_address_units)?;

    let seeds = read_seeds(memory, seeds_addr, seeds_len)?;
    let program_id = Address::from(
        <[u8; 32]>::try_from(memory.read_bytes(program_id_addr, 32)?)
            .map_err(|_| SbpfVmError::InvalidSliceConversion)?,
    );

    let seed_slices: Vec<&[u8]> = seeds.iter().map(|s| s.as_slice()).collect();
    match Address::try_find_program_address(&seed_slices, &program_id) {
        Some((addr, bump)) => {
            memory.write_u8(bump_seed_addr, bump)?;
            memory.write_bytes(address_addr, addr.as_ref())?;
            Ok(0)
        }
        None => Ok(1),
    }
}
