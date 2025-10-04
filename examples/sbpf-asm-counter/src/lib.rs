#[cfg(test)]
mod tests {
    use mollusk_svm::program;
    use mollusk_svm::{result::Check, Mollusk};
    use solana_sdk::account::Account;
    use solana_sdk::instruction::{AccountMeta, Instruction};
    use solana_sdk::native_token::LAMPORTS_PER_SOL;
    use solana_sdk::pubkey::Pubkey;

    const BASE_LAMPORTS: u64 = 10 * LAMPORTS_PER_SOL;
    const COUNTER_SIZE: usize = 9;

    pub fn get_program_id() -> Pubkey {
        let program_id_keypair_bytes = std::fs::read("deploy/sbpf-asm-counter-keypair.json")
            .unwrap()[..32]
            .try_into()
            .expect("slice with incorrect length");
        Pubkey::new_from_array(program_id_keypair_bytes)
    }

    #[test]
    fn test_initialize() {
        let program_id = get_program_id();
        let mollusk = Mollusk::new(&program_id, "deploy/sbpf-asm-counter");
        let (system_program, system_account) = program::keyed_account_for_system_program();

        let owner_pubkey = Pubkey::new_unique();
        let owner_account = Account::new(BASE_LAMPORTS, 0, &system_program);

        let (counter_pda, counter_bump) =
            Pubkey::find_program_address(&[b"counter", &owner_pubkey.to_bytes()], &program_id);
        let counter_account = Account::new(0, 0, &system_program);

        let mut instruction_data = vec![0]; // 0 -> Initialize
        instruction_data.extend_from_slice(&counter_bump.to_le_bytes());

        let instruction = Instruction::new_with_bytes(
            program_id,
            &instruction_data,
            vec![
                AccountMeta::new(owner_pubkey, true),
                AccountMeta::new(counter_pda, false),
                AccountMeta::new_readonly(system_program, false),
            ],
        );

        let mut expected_data = Vec::with_capacity(9);
        expected_data.push(counter_bump);
        expected_data.extend_from_slice(&0u64.to_le_bytes());

        let expected_lamports = mollusk.sysvars.rent.minimum_balance(COUNTER_SIZE);

        mollusk.process_and_validate_instruction(
            &instruction,
            &[
                (owner_pubkey, owner_account),
                (counter_pda, counter_account),
                (system_program, system_account.clone()),
            ],
            &[
                Check::success(),
                // Check if account was initialized with minimum balance for rent exemption.
                Check::account(&counter_pda)
                    .lamports(expected_lamports)
                    .build(),
                // Check if account was initialized with expected data.
                Check::account(&counter_pda).data(&expected_data).build(),
            ],
        );
    }

    #[test]
    fn test_increment() {
        let program_id = get_program_id();
        let mollusk = Mollusk::new(&program_id, "deploy/sbpf-asm-counter");
        let (system_program, system_account) = program::keyed_account_for_system_program();

        let owner_pubkey = Pubkey::new_unique();
        let owner_account = Account::new(BASE_LAMPORTS, 0, &system_program);

        let (counter_pda, counter_bump) =
            Pubkey::find_program_address(&[b"counter", &owner_pubkey.to_bytes()], &program_id);
        let mut counter_account = Account::new(
            mollusk.sysvars.rent.minimum_balance(COUNTER_SIZE),
            COUNTER_SIZE,
            &&program_id.into(),
        );

        let mut counter_data = Vec::with_capacity(9);
        counter_data.push(counter_bump);
        counter_data.extend_from_slice(&0u64.to_le_bytes()); // Initial count -> 0
        counter_account.data = counter_data;

        let mut instruction_data = vec![1]; // 1 -> Increment
        instruction_data.extend_from_slice(&counter_bump.to_le_bytes());

        let instruction = Instruction::new_with_bytes(
            program_id,
            &instruction_data,
            vec![
                AccountMeta::new(owner_pubkey, true),
                AccountMeta::new(counter_pda, false),
                AccountMeta::new_readonly(system_program, false),
            ],
        );

        let mut expected_data = Vec::with_capacity(9);
        expected_data.push(counter_bump);
        expected_data.extend_from_slice(&1u64.to_le_bytes()); // Expected count -> 1

        mollusk.process_and_validate_instruction(
            &instruction,
            &[
                (owner_pubkey, owner_account),
                (counter_pda, counter_account),
                (system_program, system_account.clone()),
            ],
            &[
                Check::success(),
                Check::account(&counter_pda).data(&expected_data).build(),
            ],
        );
    }
}
