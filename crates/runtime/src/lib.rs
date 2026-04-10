pub mod config;
pub mod cpi;
pub mod elf;
pub mod errors;
pub mod runtime;
pub mod serialize;
pub mod syscalls;

pub use {
    runtime::{ElfSource, ExecutionResult, LogCollector, Runtime},
    sbpf_common::instruction::Instruction,
    sbpf_vm::vm::CallFrame,
};

#[cfg(test)]
mod tests {
    use {
        crate::{Runtime, config::RuntimeConfig},
        mollusk_svm::{Mollusk, result::Check},
        solana_account::Account,
        solana_address::Address,
        solana_instruction::{AccountMeta, Instruction},
        solana_program_pack::Pack,
        std::{error::Error, path::PathBuf},
    };

    pub const ESCROW_SEED: &[u8] = b"escrow";
    pub const PROGRAM_ID: Address =
        Address::from_str_const("22222222222222222222222222222222222222222222");

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    fn setup_mollusk() -> Mollusk {
        let fixtures = fixtures_dir();
        let escrow_elf_path = fixtures.join("libupstream_pinocchio_escrow");
        let mut mollusk = Mollusk::new(&PROGRAM_ID, escrow_elf_path.to_str().unwrap());
        mollusk_svm_programs_token::token::add_program(&mut mollusk);
        mollusk_svm_programs_token::associated_token::add_program(&mut mollusk);
        mollusk
    }

    fn setup_sbpf_runtime() -> Runtime {
        let fixtures = fixtures_dir();
        let escrow_elf_path = fixtures.join("libupstream_pinocchio_escrow.so");
        let token_elf_path = fixtures.join("token.so");
        let associated_token_elf_path = fixtures.join("associated_token.so");
        let config = RuntimeConfig {
            compute_budget: 1_400_000,
            max_cpi_depth: 4,
            ..RuntimeConfig::default()
        };
        let mut runtime =
            Runtime::new(PROGRAM_ID, escrow_elf_path.to_str().unwrap(), config).unwrap();
        runtime.add_program(&spl_token_interface::ID, token_elf_path.to_str().unwrap());
        runtime.add_program(
            &Address::from_str_const("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"),
            associated_token_elf_path.to_str().unwrap(),
        );
        runtime
    }

    #[test]
    pub fn make_and_take() -> Result<(), Box<dyn Error>> {
        let mollusk = setup_mollusk();

        let maker = Address::new_unique();
        let taker = Address::new_unique();
        let mint_a = Address::new_unique();
        let mint_b = Address::new_unique();

        let mint_a_data = spl_token_interface::state::Mint {
            mint_authority: None.into(),
            supply: 1_000_000_000_000_000,
            decimals: 6,
            is_initialized: true,
            freeze_authority: None.into(),
        };

        let mint_b_data = spl_token_interface::state::Mint {
            mint_authority: None.into(),
            supply: 1_000_000_000_000_000,
            decimals: 6,
            is_initialized: true,
            freeze_authority: None.into(),
        };

        let maker_token_a_data = spl_token_interface::state::Account {
            mint: mint_a.to_bytes().into(),
            owner: maker.to_bytes().into(),
            amount: 100_000_000,
            delegate: None.into(),
            state: spl_token_interface::state::AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };

        let taker_token_b_data = spl_token_interface::state::Account {
            mint: mint_b.to_bytes().into(),
            owner: taker.to_bytes().into(),
            amount: 100_000_000,
            delegate: None.into(),
            state: spl_token_interface::state::AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };

        let (maker_token_a, maker_token_a_account) =
            mollusk_svm_programs_token::associated_token::create_account_for_associated_token_account(maker_token_a_data);
        let (taker_token_b, taker_token_b_account) =
            mollusk_svm_programs_token::associated_token::create_account_for_associated_token_account(taker_token_b_data);

        let mut mint_a_data_bytes = vec![0u8; spl_token_interface::state::Mint::LEN];
        let mut mint_b_data_bytes = vec![0u8; spl_token_interface::state::Mint::LEN];

        mint_a_data.pack_into_slice(&mut mint_a_data_bytes);
        mint_b_data.pack_into_slice(&mut mint_b_data_bytes);

        let maker_account = Account::new(10_000_000_000, 0, &Address::default());
        let taker_account = Account::new(10_000_000_000, 0, &Address::default());
        let mint_a_account = Account {
            lamports: mollusk
                .sysvars
                .rent
                .minimum_balance(spl_token_interface::state::Mint::LEN),
            data: mint_a_data_bytes,
            owner: mollusk_svm_programs_token::token::ID,
            executable: false,
            rent_epoch: 0,
        };

        let mint_b_account = Account {
            lamports: mollusk
                .sysvars
                .rent
                .minimum_balance(spl_token_interface::state::Mint::LEN),
            data: mint_b_data_bytes,
            owner: mollusk_svm_programs_token::token::ID,
            executable: false,
            rent_epoch: 0,
        };

        let (system_program, system_program_account) =
            mollusk_svm::program::keyed_account_for_system_program();
        let (token_program, token_program_account) =
            mollusk_svm_programs_token::token::keyed_account();
        let (associated_token_program, associated_token_program_account) =
            mollusk_svm_programs_token::associated_token::keyed_account();

        let seeds: &[&[u8]] = &[
            ESCROW_SEED.as_ref(),
            maker.as_ref(),
            mint_a.as_ref(),
            mint_b.as_ref(),
        ];
        let escrow = Address::find_program_address(seeds, &PROGRAM_ID).0;
        let escrow_account = Account::default();

        let vault = spl_associated_token_account_interface::address::get_associated_token_address(
            &escrow, &mint_a,
        );
        let vault_account = Account::default();

        let maker_token_b =
            spl_associated_token_account_interface::address::get_associated_token_address(
                &maker, &mint_b,
            );
        let maker_token_b_account = Account::default();
        let taker_token_a =
            spl_associated_token_account_interface::address::get_associated_token_address(
                &taker, &mint_a,
            );
        let taker_token_a_account = Account::default();

        let make_instruction = &Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(maker, true),
                AccountMeta::new_readonly(mint_a, false),
                AccountMeta::new_readonly(mint_b, false),
                AccountMeta::new(maker_token_a, false),
                AccountMeta::new(escrow, false),
                AccountMeta::new(vault, false),
                AccountMeta::new_readonly(token_program, false),
                AccountMeta::new_readonly(associated_token_program, false),
                AccountMeta::new_readonly(system_program, false),
            ],
            data: vec![0, 13, 37, 0, 0, 0, 0, 0, 0, 13, 37, 0, 0, 0, 0, 0, 0],
        };

        let take_instruction = &Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(taker, true),
                AccountMeta::new(maker, false),
                AccountMeta::new_readonly(mint_a, false),
                AccountMeta::new_readonly(mint_b, false),
                AccountMeta::new(maker_token_b, false),
                AccountMeta::new(taker_token_a, false),
                AccountMeta::new(taker_token_b, false),
                AccountMeta::new(escrow, false),
                AccountMeta::new(vault, false),
                AccountMeta::new_readonly(token_program, false),
                AccountMeta::new_readonly(associated_token_program, false),
                AccountMeta::new_readonly(system_program, false),
            ],
            data: vec![1],
        };

        let accounts = &vec![
            (maker, maker_account.clone()),
            (taker, taker_account.clone()),
            (mint_a, mint_a_account.clone()),
            (mint_b, mint_b_account.clone()),
            (maker_token_a, maker_token_a_account.clone()),
            (maker_token_b, maker_token_b_account.clone()),
            (taker_token_a, taker_token_a_account.clone()),
            (taker_token_b, taker_token_b_account.clone()),
            (escrow, escrow_account.clone()),
            (vault, vault_account.clone()),
            (token_program, token_program_account.clone()),
            (
                associated_token_program,
                associated_token_program_account.clone(),
            ),
            (system_program, system_program_account.clone()),
        ];

        // 1. Run tests using sbpf runtime.
        let mut runtime = setup_sbpf_runtime();

        // MAKE
        let result = runtime.run(&make_instruction, &accounts)?;
        let make_cus_consumed = result.compute_units_consumed;
        // Check program succeeded
        assert_eq!(result.exit_code, Some(0));
        let vault_acct = runtime.get_account(&vault).ok_or("vault not found")?;
        let escrow_acct = runtime.get_account(&escrow).ok_or("escrow not found")?;
        // Check vault is owned by token program
        assert_eq!(vault_acct.owner, token_program);
        // Check escrow is owned by our program
        assert_eq!(escrow_acct.owner, PROGRAM_ID);

        // TAKE
        let result = runtime.run(&take_instruction, &accounts)?;
        let take_cus_consumed = result.compute_units_consumed;
        // Check program succeeded
        assert_eq!(result.exit_code, Some(0));
        let vault_acct = runtime.get_account(&vault).ok_or("vault not found")?;
        let escrow_acct = runtime.get_account(&escrow).ok_or("escrow not found")?;
        // Check that our vault is closed
        assert_eq!(vault_acct.owner, system_program);
        assert_eq!(vault_acct.lamports, 0);
        // Check that our escrow is closed
        assert_eq!(escrow_acct.owner, system_program);
        assert_eq!(escrow_acct.lamports, 0);

        // 2. Run same tests using Mollusk.
        mollusk.process_and_validate_instruction_chain(
            &[
                (
                    &make_instruction,
                    &[
                        // Check tests all passed
                        Check::success(),
                        // Check vault is owned by token program
                        Check::account(&vault).owner(&token_program).build(),
                        // Check escrow is owned by our program
                        Check::account(&escrow).owner(&PROGRAM_ID).build(),
                        // Check consumed CUs match with runtime
                        Check::compute_units(make_cus_consumed),
                    ],
                ),
                (
                    &take_instruction,
                    &[
                        // Check tests all passed
                        Check::success(),
                        // Check that our vault is closed
                        Check::account(&vault).owner(&system_program).build(),
                        Check::account(&vault).lamports(0).build(),
                        // Check that our escrow is closed
                        Check::account(&escrow).owner(&system_program).build(),
                        Check::account(&escrow).lamports(0).build(),
                        // Check consumed CUs match with runtime
                        Check::compute_units(take_cus_consumed),
                    ],
                ),
            ],
            &accounts,
        );

        Ok(())
    }

    #[test]
    pub fn make_and_refund() -> Result<(), Box<dyn Error>> {
        let mollusk = setup_mollusk();

        let maker = Address::new_unique();
        let mint_a = Address::new_unique();
        let mint_b = Address::new_unique();

        let mint_a_data = spl_token_interface::state::Mint {
            mint_authority: None.into(),
            supply: 1_000_000_000_000_000,
            decimals: 6,
            is_initialized: true,
            freeze_authority: None.into(),
        };

        let mint_b_data = spl_token_interface::state::Mint {
            mint_authority: None.into(),
            supply: 1_000_000_000_000_000,
            decimals: 6,
            is_initialized: true,
            freeze_authority: None.into(),
        };

        let maker_token_a_data = spl_token_interface::state::Account {
            mint: mint_a.to_bytes().into(),
            owner: maker.to_bytes().into(),
            amount: 100_000_000,
            delegate: None.into(),
            state: spl_token_interface::state::AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };

        let (maker_token_a, maker_token_a_account) =
            mollusk_svm_programs_token::associated_token::create_account_for_associated_token_account(maker_token_a_data);

        let mut mint_a_data_bytes = vec![0u8; spl_token_interface::state::Mint::LEN];
        let mut mint_b_data_bytes = vec![0u8; spl_token_interface::state::Mint::LEN];

        mint_a_data.pack_into_slice(&mut mint_a_data_bytes);
        mint_b_data.pack_into_slice(&mut mint_b_data_bytes);

        let maker_account = Account::new(10_000_000_000, 0, &Address::default());
        let mint_a_account = Account {
            lamports: mollusk
                .sysvars
                .rent
                .minimum_balance(spl_token_interface::state::Mint::LEN),
            data: mint_a_data_bytes,
            owner: mollusk_svm_programs_token::token::ID,
            executable: false,
            rent_epoch: 0,
        };

        let mint_b_account = Account {
            lamports: mollusk
                .sysvars
                .rent
                .minimum_balance(spl_token_interface::state::Mint::LEN),
            data: mint_b_data_bytes,
            owner: mollusk_svm_programs_token::token::ID,
            executable: false,
            rent_epoch: 0,
        };

        let (system_program, system_program_account) =
            mollusk_svm::program::keyed_account_for_system_program();
        let (token_program, token_program_account) =
            mollusk_svm_programs_token::token::keyed_account();
        let (associated_token_program, associated_token_program_account) =
            mollusk_svm_programs_token::associated_token::keyed_account();

        let seeds: &[&[u8]] = &[
            ESCROW_SEED.as_ref(),
            maker.as_ref(),
            mint_a.as_ref(),
            mint_b.as_ref(),
        ];
        let escrow = Address::find_program_address(seeds, &PROGRAM_ID).0;
        let escrow_account = Account::default();

        let vault = spl_associated_token_account_interface::address::get_associated_token_address(
            &escrow, &mint_a,
        );
        let vault_account = Account::default();

        let make_instruction = &Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(maker, true),
                AccountMeta::new_readonly(mint_a, false),
                AccountMeta::new_readonly(mint_b, false),
                AccountMeta::new(maker_token_a, false),
                AccountMeta::new(escrow, false),
                AccountMeta::new(vault, false),
                AccountMeta::new_readonly(token_program, false),
                AccountMeta::new_readonly(associated_token_program, false),
                AccountMeta::new_readonly(system_program, false),
            ],
            data: vec![0, 13, 37, 0, 0, 0, 0, 0, 0, 13, 37, 0, 0, 0, 0, 0, 0],
        };

        let refund_instruction = &Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(maker, true),
                AccountMeta::new(maker_token_a, false),
                AccountMeta::new(escrow, false),
                AccountMeta::new(vault, false),
                AccountMeta::new_readonly(token_program, false),
                AccountMeta::new_readonly(system_program, false),
            ],
            data: vec![2],
        };

        let accounts = &vec![
            (maker, maker_account.clone()),
            (mint_a, mint_a_account.clone()),
            (mint_b, mint_b_account.clone()),
            (maker_token_a, maker_token_a_account.clone()),
            (escrow, escrow_account.clone()),
            (vault, vault_account.clone()),
            (token_program, token_program_account.clone()),
            (
                associated_token_program,
                associated_token_program_account.clone(),
            ),
            (system_program, system_program_account.clone()),
        ];

        // 1. Run tests using sbpf runtime.
        let mut runtime = setup_sbpf_runtime();

        // MAKE
        let result = runtime.run(&make_instruction, &accounts)?;
        let make_cus_consumed = result.compute_units_consumed;
        // Check program succeeded
        assert_eq!(result.exit_code, Some(0));
        let vault_acct = runtime.get_account(&vault).ok_or("vault not found")?;
        let escrow_acct = runtime.get_account(&escrow).ok_or("escrow not found")?;
        // Check vault is owned by token program
        assert_eq!(vault_acct.owner, token_program);
        // Check vault balance matches
        assert_eq!(&vault_acct.data[64..72], &[13, 37, 0, 0, 0, 0, 0, 0]);
        // Check escrow is owned by our program
        assert_eq!(escrow_acct.owner, PROGRAM_ID);
        // Check escrow amount_out matches
        assert_eq!(&escrow_acct.data[96..104], &[13, 37, 0, 0, 0, 0, 0, 0]);

        // REFUND
        let result = runtime.run(&refund_instruction, &accounts)?;
        let refund_cus_consumed = result.compute_units_consumed;
        // Check program succeeded
        assert_eq!(result.exit_code, Some(0));
        let vault_acct = runtime.get_account(&vault).ok_or("vault not found")?;
        let escrow_acct = runtime.get_account(&escrow).ok_or("escrow not found")?;
        // Check that our vault is closed
        assert_eq!(vault_acct.owner, system_program);
        assert_eq!(vault_acct.lamports, 0);
        // Check that our escrow is closed
        assert_eq!(escrow_acct.owner, system_program);
        assert_eq!(escrow_acct.lamports, 0);

        // 2. Run same tests using Mollusk.
        mollusk.process_and_validate_instruction_chain(
            &[
                (
                    &make_instruction,
                    &[
                        // Check tests all passed
                        Check::success(),
                        // Check vault is owned by token program
                        Check::account(&vault).owner(&token_program).build(),
                        // Check vault balance matches
                        Check::account(&vault)
                            .data_slice(64, &[13, 37, 0, 0, 0, 0, 0, 0])
                            .build(),
                        // Check escrow is owned by our program
                        Check::account(&escrow).owner(&PROGRAM_ID).build(),
                        // Check escrow amount_out matches
                        Check::account(&escrow)
                            .data_slice(96, &[13, 37, 0, 0, 0, 0, 0, 0])
                            .build(),
                        // Check consumed CUs match with runtime
                        Check::compute_units(make_cus_consumed),
                    ],
                ),
                (
                    &refund_instruction,
                    &[
                        // Check tests all passed
                        Check::success(),
                        // Check that our vault is closed
                        Check::account(&vault).owner(&system_program).build(),
                        Check::account(&vault).lamports(0).build(),
                        // Check that our escrow is closed
                        Check::account(&escrow).owner(&system_program).build(),
                        Check::account(&escrow).lamports(0).build(),
                        // Check consumed CUs match with runtime
                        Check::compute_units(refund_cus_consumed),
                    ],
                ),
            ],
            &accounts,
        );

        Ok(())
    }
}
