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

    // Consume once at start.
    let cost = costs.create_program_address_units;
    compute.consume(cost)?;

    let seeds = read_seeds(memory, seeds_addr, seeds_len)?;
    let program_id = Address::from(
        <[u8; 32]>::try_from(memory.read_bytes(program_id_addr, 32)?)
            .map_err(|_| SbpfVmError::InvalidSliceConversion)?,
    );

    let mut bump_seed = [u8::MAX];
    for _ in 0..u8::MAX {
        let mut seeds_with_bump: Vec<&[u8]> = seeds.iter().map(|s| s.as_slice()).collect();
        seeds_with_bump.push(&bump_seed);

        if let Ok(addr) = Address::create_program_address(&seeds_with_bump, &program_id) {
            memory.write_u8(bump_seed_addr, bump_seed[0])?;
            memory.write_bytes(address_addr, addr.as_ref())?;
            return Ok(0);
        }

        bump_seed[0] = bump_seed[0].checked_sub(1).unwrap();
        // Consume after each failed attempt
        compute.consume(cost)?;
    }
    Ok(1)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        sbpf_vm::{compute::ComputeMeter, errors::SbpfVmError, memory::Memory},
        solana_address::Address,
    };

    fn make_memory() -> Memory {
        Memory::new(vec![], vec![], 4096, 64 * 1024)
    }

    fn costs() -> ExecutionCost {
        ExecutionCost::default()
    }

    fn meter(limit: u64) -> ComputeMeter {
        ComputeMeter::new(limit)
    }

    fn setup_seeds(
        memory: &mut Memory,
        seeds: &[&[u8]],
        program_id: &Address,
    ) -> (u64, u64, u64, u64) {
        // Write each seed's bytes starting at offset 0, spaced 64 bytes apart
        let mut seed_data_addrs = Vec::new();
        for (i, seed) in seeds.iter().enumerate() {
            let addr = Memory::HEAP_START + (i as u64 * 64);
            memory.write_bytes(addr, seed).unwrap();
            seed_data_addrs.push((addr, seed.len() as u64));
        }

        // Write the slice-descriptor array after all seed data
        let seeds_addr = Memory::HEAP_START + 1024;
        for (i, (ptr, len)) in seed_data_addrs.iter().enumerate() {
            let desc = seeds_addr + (i as u64 * 16);
            memory.write_u64(desc, *ptr).unwrap();
            memory.write_u64(desc + 8, *len).unwrap();
        }

        let program_id_addr = Memory::HEAP_START + 2048;
        memory
            .write_bytes(program_id_addr, program_id.as_ref())
            .unwrap();

        let address_out_addr = Memory::HEAP_START + 2096;

        (
            seeds_addr,
            seeds.len() as u64,
            program_id_addr,
            address_out_addr,
        )
    }

    #[test]
    fn test_create_program_address_valid_pda() {
        let mut memory = make_memory();
        let program_id = Address::from([2u8; 32]);

        // find_program_address gives us seeds+bump that produce a valid PDA
        let (expected_addr, bump) = Address::find_program_address(&[b"test"], &program_id);
        let bump_bytes = [bump];

        let (seeds_addr, seeds_len, program_id_addr, address_out_addr) =
            setup_seeds(&mut memory, &[b"test", &bump_bytes], &program_id);

        let registers = [seeds_addr, seeds_len, program_id_addr, address_out_addr, 0];
        let ret = sol_create_program_address(registers, &mut memory, &meter(1_000_000), &costs())
            .unwrap();

        assert_eq!(ret, 0, "should return 0 for a valid PDA");
        let written_addr: [u8; 32] = memory
            .read_bytes(address_out_addr, 32)
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(Address::from(written_addr), expected_addr);
    }

    #[test]
    fn test_create_program_address_too_many_seeds() {
        let mut memory = make_memory();
        let program_id = Address::from([1u8; 32]);
        // 17 seeds exceeds MAX_SEEDS=16
        let seeds: Vec<&[u8]> = (0..17).map(|_| b"s".as_ref()).collect();
        let (seeds_addr, seeds_len, program_id_addr, address_out_addr) =
            setup_seeds(&mut memory, &seeds, &program_id);

        let registers = [seeds_addr, seeds_len, program_id_addr, address_out_addr, 0];
        assert!(matches!(
            sol_create_program_address(registers, &mut memory, &meter(1_000_000), &costs()),
            Err(SbpfVmError::MaxSeedLengthExceeded)
        ));
    }

    #[test]
    fn test_create_program_address_seed_too_long() {
        let mut memory = make_memory();
        let program_id = Address::from([1u8; 32]);
        // 33-byte seed exceeds MAX_SEED_LEN=32
        let long_seed = [0u8; 33];
        let (seeds_addr, seeds_len, program_id_addr, address_out_addr) =
            setup_seeds(&mut memory, &[&long_seed], &program_id);

        let registers = [seeds_addr, seeds_len, program_id_addr, address_out_addr, 0];
        assert!(matches!(
            sol_create_program_address(registers, &mut memory, &meter(1_000_000), &costs()),
            Err(SbpfVmError::MaxSeedLengthExceeded)
        ));
    }

    #[test]
    fn test_create_program_address_compute_exhausted() {
        let mut memory = make_memory();
        let program_id = Address::from([1u8; 32]);
        let (seeds_addr, seeds_len, program_id_addr, address_out_addr) =
            setup_seeds(&mut memory, &[b"seed"], &program_id);

        let registers = [seeds_addr, seeds_len, program_id_addr, address_out_addr, 0];
        // create_program_address_units = 1500; budget=1499 fails
        assert!(matches!(
            sol_create_program_address(registers, &mut memory, &meter(1499), &costs()),
            Err(SbpfVmError::ComputeBudgetExceeded { .. })
        ));
    }

    #[test]
    fn test_try_find_program_address_matches_library() {
        let mut memory = make_memory();
        let program_id = Address::from([3u8; 32]);

        let (seeds_addr, seeds_len, program_id_addr, address_out_addr) =
            setup_seeds(&mut memory, &[b"find-me"], &program_id);

        let bump_out_addr = Memory::HEAP_START + 2200;

        let registers = [
            seeds_addr,
            seeds_len,
            program_id_addr,
            address_out_addr,
            bump_out_addr,
        ];
        let ret = sol_try_find_program_address(registers, &mut memory, &meter(1_000_000), &costs())
            .unwrap();

        assert_eq!(ret, 0, "should find a valid PDA");

        let written_bump = memory.read_u8(bump_out_addr).unwrap();
        let written_addr: [u8; 32] = memory
            .read_bytes(address_out_addr, 32)
            .unwrap()
            .try_into()
            .unwrap();

        let (expected_addr, expected_bump) =
            Address::find_program_address(&[b"find-me"], &program_id);

        assert_eq!(written_bump, expected_bump);
        assert_eq!(Address::from(written_addr), expected_addr);
    }

    #[test]
    fn test_try_find_program_address_too_many_seeds() {
        let mut memory = make_memory();
        let program_id = Address::from([1u8; 32]);
        let seeds: Vec<&[u8]> = (0..17).map(|_| b"x".as_ref()).collect();
        let (seeds_addr, seeds_len, program_id_addr, address_out_addr) =
            setup_seeds(&mut memory, &seeds, &program_id);

        let bump_out_addr = Memory::HEAP_START + 2200;
        let registers = [
            seeds_addr,
            seeds_len,
            program_id_addr,
            address_out_addr,
            bump_out_addr,
        ];
        assert!(matches!(
            sol_try_find_program_address(registers, &mut memory, &meter(1_000_000), &costs()),
            Err(SbpfVmError::MaxSeedLengthExceeded)
        ));
    }
}
