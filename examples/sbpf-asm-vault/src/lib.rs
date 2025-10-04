#[cfg(test)]
mod tests {
    use mollusk_svm::program;
    use mollusk_svm::{result::Check, Mollusk};
    use solana_sdk::account::Account;
    use solana_sdk::instruction::{AccountMeta, Instruction};
    use solana_sdk::native_token::LAMPORTS_PER_SOL;
    use solana_sdk::program_error::ProgramError;
    use solana_sdk::pubkey::Pubkey;

    const BASE_LAMPORTS: u64 = 10 * LAMPORTS_PER_SOL;
    const DEPOSIT_AMOUNT: u64 = 1;
    const DEPOSIT_LAMPORTS: u64 = DEPOSIT_AMOUNT * LAMPORTS_PER_SOL;

    pub fn get_program_id() -> Pubkey {
        let program_id_keypair_bytes = std::fs::read("deploy/sbpf-asm-vault-keypair.json").unwrap()
            [..32]
            .try_into()
            .expect("slice with incorrect length");
        Pubkey::new_from_array(program_id_keypair_bytes)
    }

    #[test]
    fn test_invalid_pda() {
        let program_id = get_program_id();
        let mollusk = Mollusk::new(&program_id, "deploy/sbpf-asm-vault");
        let (system_program, system_account) = program::keyed_account_for_system_program();

        let owner_pubkey = Pubkey::new_unique();
        let owner_account = Account::new(BASE_LAMPORTS, 0, &system_program);

        // Incorrect vault PDA.
        let (vault_pda, vault_bump) =
            Pubkey::find_program_address(&[b"wrong", &owner_pubkey.to_bytes()], &program_id);
        let vault_account = Account::new(0, 0, &system_program);
        println!("Vault PDA: {}, Bump: {}", vault_pda, vault_bump);

        let mut instruction_data = vec![0]; // 0 -> Deposit
        instruction_data.extend_from_slice(&vault_bump.to_le_bytes());
        instruction_data.extend_from_slice(&DEPOSIT_LAMPORTS.to_le_bytes());

        let instruction = Instruction::new_with_bytes(
            program_id,
            &instruction_data,
            vec![
                AccountMeta::new(owner_pubkey, true),
                AccountMeta::new(vault_pda, false),
                AccountMeta::new_readonly(system_program, false),
            ],
        );
        mollusk.process_and_validate_instruction(
            &instruction,
            &[
                (owner_pubkey, owner_account),
                (vault_pda, vault_account),
                (system_program, system_account.clone()),
            ],
            &[Check::err(ProgramError::Custom(12))],
        );
    }

    #[test]
    fn test_deposit() {
        let program_id = get_program_id();
        let mollusk = Mollusk::new(&program_id, "deploy/sbpf-asm-vault");
        let (system_program, system_account) = program::keyed_account_for_system_program();

        let owner_pubkey = Pubkey::new_unique();
        let owner_account = Account::new(BASE_LAMPORTS, 0, &system_program);

        let (vault_pda, vault_bump) =
            Pubkey::find_program_address(&[b"vault", &owner_pubkey.to_bytes()], &program_id);
        let vault_account = Account::new(0, 0, &system_program);

        let mut instruction_data = vec![0]; // 0 -> Deposit
        instruction_data.extend_from_slice(&vault_bump.to_le_bytes());
        instruction_data.extend_from_slice(&DEPOSIT_LAMPORTS.to_le_bytes());

        let instruction = Instruction::new_with_bytes(
            program_id,
            &instruction_data,
            vec![
                AccountMeta::new(owner_pubkey, true),
                AccountMeta::new(vault_pda, false),
                AccountMeta::new_readonly(system_program, false),
            ],
        );
        mollusk.process_and_validate_instruction(
            &instruction,
            &[
                (owner_pubkey, owner_account),
                (vault_pda, vault_account),
                (system_program, system_account.clone()),
            ],
            &[
                Check::success(),
                Check::account(&owner_pubkey)
                    .lamports(BASE_LAMPORTS - DEPOSIT_LAMPORTS)
                    .build(),
                Check::account(&vault_pda)
                    .lamports(DEPOSIT_LAMPORTS)
                    .build(),
            ],
        );
    }

    #[test]
    fn test_withdraw() {
        let program_id = get_program_id();
        let mollusk = Mollusk::new(&program_id, "deploy/sbpf-asm-vault");
        let (system_program, system_account) = program::keyed_account_for_system_program();

        let owner_pubkey = Pubkey::new_unique();
        let owner_account = Account::new(BASE_LAMPORTS, 0, &system_program);

        let (vault_pda, vault_bump) =
            Pubkey::find_program_address(&[b"vault", &owner_pubkey.to_bytes()], &program_id);
        let vault_account = Account::new(DEPOSIT_LAMPORTS, 0, &system_program);

        let mut instruction_data = vec![1]; // 1 -> Withdraw
        instruction_data.extend_from_slice(&vault_bump.to_le_bytes());
        instruction_data.extend_from_slice(&DEPOSIT_LAMPORTS.to_le_bytes()); // Withdraw all

        let instruction = Instruction::new_with_bytes(
            program_id,
            &instruction_data,
            vec![
                AccountMeta::new(owner_pubkey, true),
                AccountMeta::new(vault_pda, false),
                AccountMeta::new_readonly(system_program, false),
            ],
        );
        mollusk.process_and_validate_instruction(
            &instruction,
            &[
                (owner_pubkey, owner_account),
                (vault_pda, vault_account),
                (system_program, system_account.clone()),
            ],
            &[
                Check::success(),
                Check::account(&vault_pda).lamports(0).build(),
                Check::account(&owner_pubkey)
                    .lamports(BASE_LAMPORTS + DEPOSIT_LAMPORTS)
                    .build(),
            ],
        );
    }
}
