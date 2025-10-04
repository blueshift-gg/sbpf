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
        let program_id_keypair_bytes = std::fs::read("deploy/sbpf-asm-cpi-keypair.json").unwrap()
            [..32]
            .try_into()
            .expect("slice with incorrect length");
        Pubkey::new_from_array(program_id_keypair_bytes)
    }

    #[test]
    fn test_invalid_num_accounts() {
        let program_id = get_program_id();
        let mollusk = Mollusk::new(&program_id, "deploy/sbpf-asm-cpi");
        let (system_program, system_account) = program::keyed_account_for_system_program();

        let sender_pubkey = Pubkey::new_unique();
        let receiver_pubkey = Pubkey::new_unique();
        let extra_pubkey = Pubkey::new_unique();

        // Less than 3 accounts.
        let instruction = Instruction::new_with_bytes(
            program_id,
            &[],
            vec![AccountMeta::new(sender_pubkey, true)],
        );
        mollusk.process_and_validate_instruction(
            &instruction,
            &[(
                sender_pubkey,
                Account::new(BASE_LAMPORTS, 0, &system_program),
            )],
            &[Check::err(ProgramError::Custom(1))],
        );

        // More than 3 accounts.
        let instruction = Instruction::new_with_bytes(
            program_id,
            &[],
            vec![
                AccountMeta::new(sender_pubkey, true),
                AccountMeta::new(receiver_pubkey, true),
                AccountMeta::new(extra_pubkey, true),
                AccountMeta::new_readonly(system_program, false),
            ],
        );
        mollusk.process_and_validate_instruction(
            &instruction,
            &[
                (
                    sender_pubkey,
                    Account::new(BASE_LAMPORTS, 0, &system_program),
                ),
                (
                    receiver_pubkey,
                    Account::new(BASE_LAMPORTS, 0, &system_program),
                ),
                (
                    extra_pubkey,
                    Account::new(BASE_LAMPORTS, 0, &system_program),
                ),
                (system_program, system_account.clone()),
            ],
            &[Check::err(ProgramError::Custom(1))],
        );
    }

    #[test]
    fn test_duplicate_accounts() {
        let program_id = get_program_id();
        let mollusk = Mollusk::new(&program_id, "deploy/sbpf-asm-cpi");
        let (system_program, system_account) = program::keyed_account_for_system_program();

        let sender_pubkey = Pubkey::new_unique();

        let instruction = Instruction::new_with_bytes(
            program_id,
            &[],
            // duplicate accounts
            vec![
                AccountMeta::new(sender_pubkey, true),
                AccountMeta::new(sender_pubkey, true),
                AccountMeta::new_readonly(system_program, false),
            ],
        );
        mollusk.process_and_validate_instruction(
            &instruction,
            &[
                (
                    sender_pubkey,
                    Account::new(BASE_LAMPORTS, 0, &system_program),
                ),
                (system_program, system_account.clone()),
            ],
            &[Check::err(ProgramError::Custom(2))],
        );
    }

    #[test]
    fn test_invalid_instruction_data() {
        let program_id = get_program_id();
        let mollusk = Mollusk::new(&program_id, "deploy/sbpf-asm-cpi");
        let (system_program, system_account) = program::keyed_account_for_system_program();

        let sender_pubkey = Pubkey::new_unique();
        let receiver_pubkey = Pubkey::new_unique();

        let instruction = Instruction::new_with_bytes(
            program_id,
            // empty instruction data
            &[],
            vec![
                AccountMeta::new(sender_pubkey, true),
                AccountMeta::new(receiver_pubkey, true),
                AccountMeta::new_readonly(system_program, false),
            ],
        );
        mollusk.process_and_validate_instruction(
            &instruction,
            &[
                (
                    sender_pubkey,
                    Account::new(BASE_LAMPORTS, 0, &system_program),
                ),
                (
                    receiver_pubkey,
                    Account::new(BASE_LAMPORTS, 0, &system_program),
                ),
                (system_program, system_account.clone()),
            ],
            &[Check::err(ProgramError::Custom(3))],
        );
    }

    #[test]
    fn test_insufficient_lamports() {
        let program_id = get_program_id();
        let mollusk = Mollusk::new(&program_id, "deploy/sbpf-asm-cpi");
        let (system_program, system_account) = program::keyed_account_for_system_program();

        let sender_pubkey = Pubkey::new_unique();
        let receiver_pubkey = Pubkey::new_unique();

        let amount = 20 * LAMPORTS_PER_SOL;
        let instruction_data = amount.to_le_bytes();
        let instruction = Instruction::new_with_bytes(
            program_id,
            &instruction_data,
            vec![
                AccountMeta::new(sender_pubkey, true),
                AccountMeta::new(receiver_pubkey, true),
                AccountMeta::new_readonly(system_program, false),
            ],
        );
        mollusk.process_and_validate_instruction(
            &instruction,
            &[
                (
                    sender_pubkey,
                    Account::new(BASE_LAMPORTS, 0, &system_program),
                ),
                (
                    receiver_pubkey,
                    Account::new(BASE_LAMPORTS, 0, &system_program),
                ),
                (system_program, system_account.clone()),
            ],
            &[Check::err(ProgramError::Custom(4))],
        );
    }

    #[test]
    fn test_transfer_lamports() {
        let program_id = get_program_id();
        let mollusk = Mollusk::new(&program_id, "deploy/sbpf-asm-cpi");
        let (system_program, system_account) = program::keyed_account_for_system_program();

        let sender_pubkey = Pubkey::new_unique();
        let receiver_pubkey = Pubkey::new_unique();

        let instruction_data = DEPOSIT_LAMPORTS.to_le_bytes();
        let instruction = Instruction::new_with_bytes(
            program_id,
            &instruction_data,
            vec![
                AccountMeta::new(sender_pubkey, true),
                AccountMeta::new(receiver_pubkey, false),
                AccountMeta::new_readonly(system_program, false),
            ],
        );
        mollusk.process_and_validate_instruction(
            &instruction,
            &[
                (
                    sender_pubkey,
                    Account::new(BASE_LAMPORTS, 0, &system_program),
                ),
                (
                    receiver_pubkey,
                    Account::new(BASE_LAMPORTS, 0, &system_program),
                ),
                (system_program, system_account.clone()),
            ],
            &[
                Check::success(),
                Check::account(&sender_pubkey)
                    .lamports(BASE_LAMPORTS - DEPOSIT_LAMPORTS)
                    .build(),
                Check::account(&receiver_pubkey)
                    .lamports(BASE_LAMPORTS + DEPOSIT_LAMPORTS)
                    .build(),
            ],
        );
    }
}
