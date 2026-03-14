use {
    sbpf_vm::{errors::SbpfVmResult, memory::Memory},
    solana_address::Address,
};

const MAX_SIGNERS: u64 = 16;
const MAX_SEEDS: u64 = 16;
const MAX_SEED_LEN: u64 = 32;

pub struct CpiRequest {
    pub program_id: Address,
    pub accounts: Vec<CpiAccountMeta>,
    pub data: Vec<u8>,
    pub caller_accounts: Vec<CallerAccountInfo>,
    pub signers: Vec<Address>,
}

pub struct CpiAccountMeta {
    pub pubkey: Address,
    pub is_signer: bool,
    pub is_writable: bool,
}

/// Caller's VM memory pointers for an account.
pub struct CallerAccountInfo {
    pub pubkey: Address,
    pub lamports_addr: u64,
    pub data_addr: u64,
    pub data_len: u64,
    pub owner_addr: u64,
    pub vm_data_len_addr: u64,
    pub is_writable: bool,
}

/// Parse sol_invoke_signed_c arguments from memory.
pub fn parse_cpi_c(
    registers: [u64; 5],
    memory: &Memory,
    caller_program_id: &Address,
) -> SbpfVmResult<CpiRequest> {
    let instruction_addr = registers[0];
    let account_infos_addr = registers[1];
    let account_infos_len = registers[2];
    let signers_seeds_addr = registers[3];
    let signers_seeds_len = registers[4];

    let program_id_ptr = memory.read_u64(instruction_addr)?;
    let accounts_ptr = memory.read_u64(instruction_addr + 8)?;
    let accounts_len = memory.read_u64(instruction_addr + 16)?;
    let data_ptr = memory.read_u64(instruction_addr + 24)?;
    let data_len = memory.read_u64(instruction_addr + 32)?;

    let program_id_bytes = memory.read_bytes(program_id_ptr, 32)?;
    let program_id = Address::new_from_array(program_id_bytes.try_into().unwrap());

    let mut accounts = Vec::with_capacity(accounts_len as usize);
    for i in 0..accounts_len {
        let meta_addr = accounts_ptr + i * 16;
        let pubkey_ptr = memory.read_u64(meta_addr)?;
        let pubkey_bytes = memory.read_bytes(pubkey_ptr, 32)?;
        let is_writable = memory.read_u8(meta_addr + 8)? != 0;
        let is_signer = memory.read_u8(meta_addr + 9)? != 0;
        accounts.push(CpiAccountMeta {
            pubkey: Address::new_from_array(pubkey_bytes.try_into().unwrap()),
            is_signer,
            is_writable,
        });
    }

    let data = memory.read_bytes(data_ptr, data_len as usize)?.to_vec();

    let caller_accounts = parse_account_infos_c(memory, account_infos_addr, account_infos_len)?;

    let signers = parse_signers(
        memory,
        caller_program_id,
        signers_seeds_addr,
        signers_seeds_len,
    )?;

    Ok(CpiRequest {
        program_id,
        accounts,
        data,
        caller_accounts,
        signers,
    })
}

/// Parse sol_invoke_signed_rust arguments from memory.
pub fn parse_cpi_rust(
    registers: [u64; 5],
    memory: &Memory,
    caller_program_id: &Address,
) -> SbpfVmResult<CpiRequest> {
    let instruction_addr = registers[0];
    let account_infos_addr = registers[1];
    let account_infos_len = registers[2];
    let signers_seeds_addr = registers[3];
    let signers_seeds_len = registers[4];

    let accounts_ptr = memory.read_u64(instruction_addr)?;
    let _accounts_cap = memory.read_u64(instruction_addr + 8)?;
    let accounts_len = memory.read_u64(instruction_addr + 16)?;
    let data_ptr = memory.read_u64(instruction_addr + 24)?;
    let _data_cap = memory.read_u64(instruction_addr + 32)?;
    let data_len = memory.read_u64(instruction_addr + 40)?;
    let program_id_bytes = memory.read_bytes(instruction_addr + 48, 32)?;
    let program_id = Address::new_from_array(program_id_bytes.try_into().unwrap());

    let mut accounts = Vec::with_capacity(accounts_len as usize);
    for i in 0..accounts_len {
        let meta_addr = accounts_ptr + i * 34;
        let pubkey_bytes = memory.read_bytes(meta_addr, 32)?;
        let is_signer = memory.read_u8(meta_addr + 32)? != 0;
        let is_writable = memory.read_u8(meta_addr + 33)? != 0;
        accounts.push(CpiAccountMeta {
            pubkey: Address::new_from_array(pubkey_bytes.try_into().unwrap()),
            is_signer,
            is_writable,
        });
    }

    let data = memory.read_bytes(data_ptr, data_len as usize)?.to_vec();
    let caller_accounts = parse_account_infos_rust(memory, account_infos_addr, account_infos_len)?;
    let signers = parse_signers(
        memory,
        caller_program_id,
        signers_seeds_addr,
        signers_seeds_len,
    )?;

    Ok(CpiRequest {
        program_id,
        accounts,
        data,
        caller_accounts,
        signers,
    })
}

/// Parse C `SolAccountInfo` array.
fn parse_account_infos_c(
    memory: &Memory,
    account_infos_addr: u64,
    account_infos_len: u64,
) -> SbpfVmResult<Vec<CallerAccountInfo>> {
    let mut caller_accounts = Vec::with_capacity(account_infos_len as usize);
    for i in 0..account_infos_len {
        let info_addr = account_infos_addr + i * 56;
        let key_ptr = memory.read_u64(info_addr)?;
        let lamports_ptr = memory.read_u64(info_addr + 8)?;
        let acct_data_len = memory.read_u64(info_addr + 16)?;
        let data_ptr = memory.read_u64(info_addr + 24)?;
        let owner_ptr = memory.read_u64(info_addr + 32)?;
        let _rent_epoch = memory.read_u64(info_addr + 40)?;
        let _is_signer = memory.read_u8(info_addr + 48)? != 0;
        let is_writable = memory.read_u8(info_addr + 49)? != 0;

        let key_bytes = memory.read_bytes(key_ptr, 32)?;
        caller_accounts.push(CallerAccountInfo {
            pubkey: Address::new_from_array(key_bytes.try_into().unwrap()),
            lamports_addr: lamports_ptr,
            data_addr: data_ptr,
            data_len: acct_data_len,
            owner_addr: owner_ptr,
            vm_data_len_addr: info_addr + 16,
            is_writable,
        });
    }
    Ok(caller_accounts)
}

/// Parse Rust `AccountInfo` array.
fn parse_account_infos_rust(
    memory: &Memory,
    account_infos_addr: u64,
    account_infos_len: u64,
) -> SbpfVmResult<Vec<CallerAccountInfo>> {
    const RUST_ACCOUNT_INFO_SIZE: u64 = 48;

    let mut caller_accounts = Vec::with_capacity(account_infos_len as usize);
    for i in 0..account_infos_len {
        let info_addr = account_infos_addr + i * RUST_ACCOUNT_INFO_SIZE;

        let key_ptr = memory.read_u64(info_addr)?;
        let key_bytes = memory.read_bytes(key_ptr, 32)?;

        let lamports_rc_ptr = memory.read_u64(info_addr + 8)?;
        let lamports_addr = memory.read_u64(lamports_rc_ptr + 24)?;

        let data_rc_ptr = memory.read_u64(info_addr + 16)?;
        let data_addr = memory.read_u64(data_rc_ptr + 24)?;
        let data_len = memory.read_u64(data_rc_ptr + 32)?;

        let vm_data_len_addr = data_rc_ptr + 32;

        let owner_ptr = memory.read_u64(info_addr + 24)?;
        let _is_signer = memory.read_u8(info_addr + 40)? != 0;
        let is_writable = memory.read_u8(info_addr + 41)? != 0;

        caller_accounts.push(CallerAccountInfo {
            pubkey: Address::new_from_array(key_bytes.try_into().unwrap()),
            lamports_addr,
            data_addr,
            data_len,
            owner_addr: owner_ptr,
            vm_data_len_addr,
            is_writable,
        });
    }
    Ok(caller_accounts)
}

/// Parse signers.
fn parse_signers(
    memory: &Memory,
    caller_program_id: &Address,
    signers_seeds_addr: u64,
    signers_seeds_len: u64,
) -> SbpfVmResult<Vec<Address>> {
    if signers_seeds_len == 0 {
        return Ok(Vec::new());
    }
    if signers_seeds_len > MAX_SIGNERS {
        return Err(sbpf_vm::errors::SbpfVmError::SyscallError(
            "Too many signers".to_string(),
        ));
    }

    let mut signers = Vec::with_capacity(signers_seeds_len as usize);

    for i in 0..signers_seeds_len {
        let entry_addr = signers_seeds_addr + i * 16;
        let seeds_ptr = memory.read_u64(entry_addr)?;
        let seeds_len = memory.read_u64(entry_addr + 8)?;

        if seeds_len > MAX_SEEDS {
            return Err(sbpf_vm::errors::SbpfVmError::SyscallError(
                "Max seed length exceeded".to_string(),
            ));
        }

        let mut seeds: Vec<Vec<u8>> = Vec::with_capacity(seeds_len as usize);
        for j in 0..seeds_len {
            let seed_entry_addr = seeds_ptr + j * 16;
            let seed_data_ptr = memory.read_u64(seed_entry_addr)?;
            let seed_data_len = memory.read_u64(seed_entry_addr + 8)?;

            if seed_data_len > MAX_SEED_LEN {
                return Err(sbpf_vm::errors::SbpfVmError::SyscallError(
                    "Max seed length exceeded".to_string(),
                ));
            }

            let seed_bytes = memory.read_bytes(seed_data_ptr, seed_data_len as usize)?;
            seeds.push(seed_bytes.to_vec());
        }

        let seed_refs: Vec<&[u8]> = seeds.iter().map(|s| s.as_slice()).collect();
        let pda = Address::create_program_address(&seed_refs, caller_program_id)
            .map_err(|_| sbpf_vm::errors::SbpfVmError::SyscallError("Invalid seeds".to_string()))?;

        signers.push(pda);
    }

    Ok(signers)
}
