use {
    sbpf_vm::{
        errors::{SbpfVmError, SbpfVmResult},
        memory::Memory,
    },
    solana_sdk::pubkey::Pubkey,
};

const MAX_SIGNERS: usize = 16;
const MAX_SEEDS: usize = 16;
const MAX_SEED_LEN: usize = 32;
const STABLE_SLICE_SIZE: u64 = 16;

/// Parsed CPI instruction
#[derive(Debug, Clone)]
pub struct CpiInstruction {
    pub program_id: Pubkey,
    pub accounts: Vec<CpiAccountMeta>,
    pub data: Vec<u8>,
}

/// Account metadata for CPI
#[derive(Debug, Clone)]
pub struct CpiAccountMeta {
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

/// Information about caller's account for syncing after CPI
#[derive(Debug, Clone)]
pub struct CallerAccountInfo {
    pub pubkey: Pubkey,
    pub lamports_addr: u64,
    pub data_addr: u64,
    pub data_len: u64,
    pub owner: Pubkey,
    pub rent_epoch: u64,
    pub is_executable: bool,
    pub is_writable: bool,
}

/// Parse C ABI instruction from memory
pub fn translate_c_instruction(
    memory: &Memory,
    instruction_addr: u64,
) -> SbpfVmResult<CpiInstruction> {
    let program_id_ptr = memory.read_u64(instruction_addr)?;
    let accounts_ptr = memory.read_u64(instruction_addr + 8)?;
    let accounts_len = memory.read_u64(instruction_addr + 16)?;
    let data_ptr = memory.read_u64(instruction_addr + 24)?;
    let data_len = memory.read_u64(instruction_addr + 32)?;

    let program_id = read_pubkey(memory, program_id_ptr)?;

    let mut accounts = Vec::with_capacity(accounts_len as usize);
    for i in 0..accounts_len {
        let meta_addr = accounts_ptr + (i * 16);
        let pubkey_ptr = memory.read_u64(meta_addr)?;
        let pubkey = read_pubkey(memory, pubkey_ptr)?;
        let is_writable = memory.read_u8(meta_addr + 8)? != 0;
        let is_signer = memory.read_u8(meta_addr + 9)? != 0;

        accounts.push(CpiAccountMeta {
            pubkey,
            is_signer,
            is_writable,
        });
    }

    let data = memory.read_bytes(data_ptr, data_len as usize)?.to_vec();

    Ok(CpiInstruction {
        program_id,
        accounts,
        data,
    })
}

/// Parse Rust ABI instruction from memory
pub fn translate_rust_instruction(
    memory: &Memory,
    instruction_addr: u64,
) -> SbpfVmResult<CpiInstruction> {
    let accounts_ptr = memory.read_u64(instruction_addr)?;
    let accounts_len = memory.read_u64(instruction_addr + 16)?;
    let data_ptr = memory.read_u64(instruction_addr + 24)?;
    let data_len = memory.read_u64(instruction_addr + 40)?;

    let program_id = read_pubkey(memory, instruction_addr + 48)?;

    let mut accounts = Vec::with_capacity(accounts_len as usize);
    for i in 0..accounts_len {
        let meta_addr = accounts_ptr + (i * 40);
        let pubkey = read_pubkey(memory, meta_addr)?;
        let is_signer = memory.read_u8(meta_addr + 32)? != 0;
        let is_writable = memory.read_u8(meta_addr + 33)? != 0;

        accounts.push(CpiAccountMeta {
            pubkey,
            is_signer,
            is_writable,
        });
    }

    let data = memory.read_bytes(data_ptr, data_len as usize)?.to_vec();

    Ok(CpiInstruction {
        program_id,
        accounts,
        data,
    })
}

/// Parse SolAccountInfo array from caller's memory
pub fn translate_account_infos(
    memory: &Memory,
    account_infos_addr: u64,
    account_infos_len: u64,
) -> SbpfVmResult<Vec<CallerAccountInfo>> {
    let mut results = Vec::with_capacity(account_infos_len as usize);

    for i in 0..account_infos_len {
        let info_addr = account_infos_addr + (i * 56);

        let key_ptr = memory.read_u64(info_addr)?;
        let lamports_ptr = memory.read_u64(info_addr + 8)?;
        let data_len = memory.read_u64(info_addr + 16)?;
        let data_ptr = memory.read_u64(info_addr + 24)?;
        let owner_ptr = memory.read_u64(info_addr + 32)?;
        let rent_epoch = memory.read_u64(info_addr + 40)?;
        let is_writable = memory.read_u8(info_addr + 49)? != 0;
        let is_executable = memory.read_u8(info_addr + 50)? != 0;

        let pubkey = read_pubkey(memory, key_ptr)?;
        let owner = read_pubkey(memory, owner_ptr)?;

        results.push(CallerAccountInfo {
            pubkey,
            lamports_addr: lamports_ptr,
            data_addr: data_ptr,
            data_len,
            owner,
            rent_epoch,
            is_executable,
            is_writable,
        });
    }

    Ok(results)
}

/// Parse signer seeds and derive PDAs (C ABI).
pub fn translate_signers_c(
    memory: &Memory,
    caller_program_id: &Pubkey,
    signers_seeds_addr: u64,
    signers_seeds_len: u64,
) -> SbpfVmResult<Vec<Pubkey>> {
    if signers_seeds_len == 0 {
        return Ok(Vec::new());
    }

    if signers_seeds_len > MAX_SIGNERS as u64 {
        return Err(SbpfVmError::TooManySigners);
    }

    if signers_seeds_addr == 0 {
        return Ok(Vec::new());
    }

    let mut signers = Vec::with_capacity(signers_seeds_len as usize);

    for i in 0..signers_seeds_len {
        let signer_entry_addr = signers_seeds_addr + (i * STABLE_SLICE_SIZE);
        let (seeds_ptr, seeds_len) = read_ptr_len(memory, signer_entry_addr)?;

        if seeds_len > MAX_SEEDS as u64 {
            return Err(SbpfVmError::MaxSeedLengthExceeded);
        }

        let mut seeds: Vec<Vec<u8>> = Vec::with_capacity(seeds_len as usize);

        for j in 0..seeds_len {
            let seed_entry_addr = seeds_ptr + (j * STABLE_SLICE_SIZE);
            let (seed_data_ptr, seed_data_len) = read_ptr_len(memory, seed_entry_addr)?;

            if seed_data_len > MAX_SEED_LEN as u64 {
                return Err(SbpfVmError::MaxSeedLengthExceeded);
            }

            let seed_bytes = memory.read_bytes(seed_data_ptr, seed_data_len as usize)?;
            seeds.push(seed_bytes.to_vec());
        }

        let seed_refs: Vec<&[u8]> = seeds.iter().map(|s| s.as_slice()).collect();
        let pda = Pubkey::create_program_address(&seed_refs, caller_program_id)
            .map_err(|_| SbpfVmError::InvalidSeeds)?;

        signers.push(pda);
    }

    Ok(signers)
}

/// Parse signer seeds and derive PDAs (Rust ABI)
pub fn translate_signers_rust(
    memory: &Memory,
    caller_program_id: &Pubkey,
    signers_seeds_addr: u64,
    _signers_seeds_len: u64,
) -> SbpfVmResult<Vec<Pubkey>> {
    if signers_seeds_addr == 0 {
        return Ok(Vec::new());
    }

    let signers_ptr = memory.read_u64(signers_seeds_addr)?;
    let signers_cap = memory.read_u64(signers_seeds_addr + 8)?;
    let signers_len = memory.read_u64(signers_seeds_addr + 16)?;

    if signers_cap < signers_len {
        return Err(SbpfVmError::InvalidSeeds);
    }

    if signers_len > MAX_SIGNERS as u64 {
        return Err(SbpfVmError::TooManySigners);
    }

    let mut signers = Vec::with_capacity(signers_len as usize);

    for i in 0..signers_len {
        let signer_entry_addr = signers_ptr + (i * STABLE_SLICE_SIZE);
        let (seeds_ptr, seeds_len) = read_ptr_len(memory, signer_entry_addr)?;

        if seeds_len > MAX_SEEDS as u64 {
            return Err(SbpfVmError::MaxSeedLengthExceeded);
        }

        let mut seeds: Vec<Vec<u8>> = Vec::with_capacity(seeds_len as usize);

        for j in 0..seeds_len {
            let seed_entry_addr = seeds_ptr + (j * STABLE_SLICE_SIZE);
            let (seed_data_ptr, seed_data_len) = read_ptr_len(memory, seed_entry_addr)?;

            if seed_data_len > MAX_SEED_LEN as u64 {
                return Err(SbpfVmError::MaxSeedLengthExceeded);
            }

            let seed_bytes = memory.read_bytes(seed_data_ptr, seed_data_len as usize)?;
            seeds.push(seed_bytes.to_vec());
        }

        let seed_refs: Vec<&[u8]> = seeds.iter().map(|s| s.as_slice()).collect();
        let pda = Pubkey::create_program_address(&seed_refs, caller_program_id)
            .map_err(|_| SbpfVmError::InvalidSeeds)?;

        signers.push(pda);
    }

    Ok(signers)
}

fn read_ptr_len(memory: &Memory, addr: u64) -> SbpfVmResult<(u64, u64)> {
    let ptr = memory.read_u64(addr)?;
    let len = memory.read_u64(addr + 8)?;
    Ok((ptr, len))
}

fn read_pubkey(memory: &Memory, addr: u64) -> SbpfVmResult<Pubkey> {
    let bytes = memory.read_bytes(addr, 32)?;
    Ok(Pubkey::from(
        <[u8; 32]>::try_from(bytes).map_err(|_| SbpfVmError::InvalidSliceConversion)?,
    ))
}
