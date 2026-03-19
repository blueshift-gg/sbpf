use {
    crate::cpi::request::CallerAccountInfo,
    sbpf_vm::{errors::SbpfVmResult, memory::Memory},
    solana_account::Account,
    solana_address::Address,
    std::collections::HashMap,
};

/// Maximum bytes an account's data can grow during CPI.
const MAX_PERMITTED_DATA_INCREASE: usize = 10240;

/// Sync current account state from the caller's VM memory into the account store before CPI.
pub fn sync_from_caller(
    memory: &Memory,
    caller_accounts: &[CallerAccountInfo],
    accounts: &mut HashMap<Address, Account>,
) -> SbpfVmResult<()> {
    for info in caller_accounts {
        let lamports = memory.read_u64(info.lamports_addr)?;
        let data = memory
            .read_bytes(info.data_addr, info.data_len as usize)?
            .to_vec();
        let owner_bytes = memory.read_bytes(info.owner_addr, 32)?;
        let owner = Address::new_from_array(owner_bytes.try_into().unwrap());

        let account = accounts.entry(info.pubkey).or_default();
        account.lamports = lamports;
        account.data = data;
        account.owner = owner;
    }
    Ok(())
}

/// Sync updated account state back to the caller's VM memory after CPI.
pub fn sync_to_caller(
    memory: &mut Memory,
    caller_accounts: &[CallerAccountInfo],
    accounts: &HashMap<Address, Account>,
) -> SbpfVmResult<()> {
    for info in caller_accounts {
        if !info.is_writable {
            continue;
        }
        let Some(account) = accounts.get(&info.pubkey) else {
            continue;
        };

        // Sync lamports and owner.
        memory.write_u64(info.lamports_addr, account.lamports)?;
        memory.write_bytes(info.owner_addr, account.owner.as_ref())?;

        let prev_len = info.data_len as usize;
        let post_len = account.data.len();
        let max_allowed = prev_len.saturating_add(MAX_PERMITTED_DATA_INCREASE);

        if post_len > max_allowed {
            return Err(sbpf_vm::errors::SbpfVmError::SyscallError(format!(
                "Account data realloc limited to {}",
                MAX_PERMITTED_DATA_INCREASE
            )));
        }

        // Handle change in data length.
        if prev_len != post_len {
            if post_len < prev_len {
                let zeros = vec![0u8; prev_len - post_len];
                memory.write_bytes(info.data_addr + post_len as u64, &zeros)?;
            }
            memory.write_u64(info.vm_data_len_addr, post_len as u64)?;
            memory.write_u64(info.data_addr.saturating_sub(8), post_len as u64)?;
        }

        // Copy the actual data bytes.
        memory.write_bytes(info.data_addr, &account.data)?;
    }
    Ok(())
}
