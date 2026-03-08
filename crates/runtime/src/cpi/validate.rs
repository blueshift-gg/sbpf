use {
    crate::{
        cpi::request::CpiRequest,
        errors::{RuntimeError, RuntimeResult},
    },
    solana_account::Account,
    solana_address::Address,
    solana_instruction::AccountMeta,
    std::collections::HashMap,
};

const SYSTEM_PROGRAM_ID: Address = Address::new_from_array([0; 32]);

/// Validates that the CPI request doesn't escalate privileges.
pub fn check_privileges(
    request: &CpiRequest,
    caller_account_metas: &[AccountMeta],
) -> RuntimeResult<()> {
    for cpi_meta in &request.accounts {
        let caller_meta = caller_account_metas
            .iter()
            .find(|m| m.pubkey == cpi_meta.pubkey);

        match caller_meta {
            Some(cm) => {
                // Check signer.
                if cpi_meta.is_signer
                    && !cm.is_signer
                    && !request.signers.contains(&cpi_meta.pubkey)
                {
                    return Err(RuntimeError::PrivilegeEscalation(
                        "signer".to_string(),
                        cpi_meta.pubkey.to_string(),
                    ));
                }

                // Check writable.
                if cpi_meta.is_writable && !cm.is_writable {
                    return Err(RuntimeError::PrivilegeEscalation(
                        "writable".to_string(),
                        cpi_meta.pubkey.to_string(),
                    ));
                }
            }
            None => {
                return Err(RuntimeError::MissingAccount(cpi_meta.pubkey.to_string()));
            }
        }
    }
    Ok(())
}

/// Validates that post-CPI account changes follow ownership rules.
pub fn check_account_changes(
    callee_program_id: &Address,
    account_metas: &[AccountMeta],
    pre_accounts: &[(Address, Account)],
    post_accounts: &HashMap<Address, Account>,
) -> RuntimeResult<()> {
    for (pubkey, pre) in pre_accounts {
        let Some(post) = post_accounts.get(pubkey) else {
            continue;
        };

        let is_writable = account_metas
            .iter()
            .any(|m| m.pubkey == *pubkey && m.is_writable);

        // Read-only accounts cannot be modified.
        if !is_writable {
            if pre.lamports != post.lamports || pre.data != post.data || pre.owner != post.owner {
                return Err(RuntimeError::PrivilegeEscalation(
                    "read-only account modified".to_string(),
                    pubkey.to_string(),
                ));
            }
            continue;
        }

        let is_owner = pre.owner == *callee_program_id;

        // Only the owner can modify data.
        if pre.data != post.data && !is_owner {
            return Err(RuntimeError::PrivilegeEscalation(
                "non-owner modified data".to_string(),
                pubkey.to_string(),
            ));
        }

        // Only the owner can debit lamports.
        if post.lamports < pre.lamports && !is_owner {
            return Err(RuntimeError::PrivilegeEscalation(
                "non-owner debited lamports".to_string(),
                pubkey.to_string(),
            ));
        }

        // Only the system program can change the owner.
        if pre.owner != post.owner && *callee_program_id != SYSTEM_PROGRAM_ID {
            return Err(RuntimeError::PrivilegeEscalation(
                "non-system-program changed owner".to_string(),
                pubkey.to_string(),
            ));
        }

        // Data is immutable for executable accounts.
        if pre.executable && pre.data != post.data {
            return Err(RuntimeError::PrivilegeEscalation(
                "executable account modified".to_string(),
                pubkey.to_string(),
            ));
        }
    }
    Ok(())
}
