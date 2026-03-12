use {
    crate::{
        cpi::request::CpiRequest,
        errors::{RuntimeError, RuntimeResult},
    },
    solana_account::Account,
    solana_address::Address,
    solana_instruction::AccountMeta,
};

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

/// Validates that a single account change follows ownership rules.
pub fn check_account_change(
    callee_program_id: &Address,
    pubkey: &Address,
    account: &Account,
    new_owner: &Address,
    new_lamports: u64,
    new_data: &[u8],
) -> RuntimeResult<()> {
    let is_owner = account.owner == *callee_program_id;

    // Only the owner can change the owner.
    if *new_owner != account.owner && !is_owner {
        return Err(RuntimeError::PrivilegeEscalation(
            "non-owner changed owner".to_string(),
            pubkey.to_string(),
        ));
    }

    // Only the owner can debit lamports.
    if new_lamports < account.lamports && !is_owner {
        return Err(RuntimeError::ExternalAccountLamportSpend(
            pubkey.to_string(),
        ));
    }

    // Only the owner can modify data.
    if new_data != account.data.as_slice() && !is_owner {
        return Err(RuntimeError::PrivilegeEscalation(
            "non-owner modified data".to_string(),
            pubkey.to_string(),
        ));
    }

    // Executable accounts cannot have their data modified.
    if account.executable && new_data != account.data.as_slice() {
        return Err(RuntimeError::PrivilegeEscalation(
            "executable account modified".to_string(),
            pubkey.to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::cpi::request::{CpiAccountMeta, CpiRequest},
    };

    const PROGRAM_A: Address = Address::new_from_array([1u8; 32]);
    const PROGRAM_B: Address = Address::new_from_array([2u8; 32]);
    const ACCT_1: Address = Address::new_from_array([10u8; 32]);
    const ACCT_2: Address = Address::new_from_array([11u8; 32]);
    const PDA: Address = Address::new_from_array([20u8; 32]);

    fn make_request(accounts: Vec<CpiAccountMeta>, signers: Vec<Address>) -> CpiRequest {
        CpiRequest {
            program_id: PROGRAM_B,
            accounts,
            data: vec![],
            caller_accounts: vec![],
            signers,
        }
    }

    fn make_account(owner: Address, lamports: u64, data: &[u8]) -> Account {
        Account {
            lamports,
            data: data.to_vec(),
            owner,
            executable: false,
            rent_epoch: 0,
        }
    }

    // Check privileges.

    #[test]
    fn check_privileges_ok() {
        let request = make_request(
            vec![CpiAccountMeta {
                pubkey: ACCT_1,
                is_signer: true,
                is_writable: true,
            }],
            vec![],
        );
        let caller_metas = vec![AccountMeta {
            pubkey: ACCT_1,
            is_signer: true,
            is_writable: true,
        }];
        assert!(check_privileges(&request, &caller_metas).is_ok());
    }

    #[test]
    fn check_privileges_signer_escalation() {
        let request = make_request(
            vec![CpiAccountMeta {
                pubkey: ACCT_1,
                is_signer: true,
                is_writable: false,
            }],
            vec![],
        );
        let caller_metas = vec![AccountMeta {
            pubkey: ACCT_1,
            is_signer: false,
            is_writable: false,
        }];
        assert!(check_privileges(&request, &caller_metas).is_err());
    }

    #[test]
    fn check_privileges_signer_via_pda() {
        let request = make_request(
            vec![CpiAccountMeta {
                pubkey: PDA,
                is_signer: true,
                is_writable: false,
            }],
            vec![PDA],
        );
        let caller_metas = vec![AccountMeta {
            pubkey: PDA,
            is_signer: false,
            is_writable: false,
        }];
        assert!(check_privileges(&request, &caller_metas).is_ok());
    }

    #[test]
    fn check_privileges_writable_escalation() {
        let request = make_request(
            vec![CpiAccountMeta {
                pubkey: ACCT_1,
                is_signer: false,
                is_writable: true,
            }],
            vec![],
        );
        let caller_metas = vec![AccountMeta {
            pubkey: ACCT_1,
            is_signer: false,
            is_writable: false,
        }];
        assert!(check_privileges(&request, &caller_metas).is_err());
    }

    #[test]
    fn check_privileges_missing_account() {
        let request = make_request(
            vec![CpiAccountMeta {
                pubkey: ACCT_2,
                is_signer: false,
                is_writable: false,
            }],
            vec![],
        );
        let caller_metas = vec![AccountMeta {
            pubkey: ACCT_1,
            is_signer: false,
            is_writable: false,
        }];
        assert!(check_privileges(&request, &caller_metas).is_err());
    }

    // Check account change.

    #[test]
    fn check_account_change_owner_modifies_data() {
        let acct = make_account(PROGRAM_A, 100, b"hello");
        assert!(
            check_account_change(&PROGRAM_A, &ACCT_1, &acct, &PROGRAM_A, 100, b"world").is_ok()
        );
    }

    #[test]
    fn check_account_change_non_owner_modifies_data() {
        let acct = make_account(PROGRAM_A, 100, b"hello");
        assert!(
            check_account_change(&PROGRAM_B, &ACCT_1, &acct, &PROGRAM_A, 100, b"world").is_err()
        );
    }

    #[test]
    fn check_account_change_owner_changes_owner() {
        let acct = make_account(PROGRAM_A, 100, b"");
        assert!(check_account_change(&PROGRAM_A, &ACCT_1, &acct, &PROGRAM_B, 100, b"").is_ok());
    }

    #[test]
    fn check_account_change_non_owner_changes_owner() {
        let acct = make_account(PROGRAM_A, 100, b"");
        assert!(check_account_change(&PROGRAM_B, &ACCT_1, &acct, &PROGRAM_B, 100, b"").is_err());
    }

    #[test]
    fn check_account_change_owner_debits_lamports() {
        let acct = make_account(PROGRAM_A, 100, b"");
        assert!(check_account_change(&PROGRAM_A, &ACCT_1, &acct, &PROGRAM_A, 50, b"").is_ok());
    }

    #[test]
    fn check_account_change_non_owner_debits_lamports() {
        let acct = make_account(PROGRAM_A, 100, b"");
        assert!(check_account_change(&PROGRAM_B, &ACCT_1, &acct, &PROGRAM_A, 50, b"").is_err());
    }

    #[test]
    fn check_account_change_non_owner_credits_lamports() {
        let acct = make_account(PROGRAM_A, 100, b"");
        assert!(check_account_change(&PROGRAM_B, &ACCT_1, &acct, &PROGRAM_A, 200, b"").is_ok());
    }

    #[test]
    fn check_account_change_executable_data_rejected() {
        let mut acct = make_account(PROGRAM_A, 100, b"code");
        acct.executable = true;
        assert!(
            check_account_change(&PROGRAM_A, &ACCT_1, &acct, &PROGRAM_A, 100, b"hack").is_err()
        );
    }

    #[test]
    fn check_account_change_no_modifications() {
        let acct = make_account(PROGRAM_A, 100, b"data");
        assert!(check_account_change(&PROGRAM_B, &ACCT_1, &acct, &PROGRAM_A, 100, b"data").is_ok());
    }
}
