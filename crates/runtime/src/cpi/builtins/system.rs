use {
    crate::{
        cpi::request::{CpiAccountMeta, CpiRequest},
        errors::{RuntimeError, RuntimeResult},
    },
    solana_account::Account,
    solana_address::Address,
    solana_system_interface::{instruction::SystemInstruction, program as system_program},
    std::collections::HashMap,
};

const SYSTEM_PROGRAM_COMPUTE_UNITS: u64 = 150;
const MAX_PERMITTED_DATA_LENGTH: u64 = 10 * 1024 * 1024;

pub fn process(
    accounts: &mut HashMap<Address, Account>,
    instruction: &CpiRequest,
    signers: &[Address],
    compute_remaining: u64,
) -> RuntimeResult<u64> {
    if compute_remaining < SYSTEM_PROGRAM_COMPUTE_UNITS {
        return Err(RuntimeError::BuiltinError(
            "insufficient compute units".into(),
        ));
    }

    let ix: SystemInstruction = bincode::deserialize(&instruction.data)
        .map_err(|_| RuntimeError::BuiltinError("invalid system instruction data".into()))?;

    match ix {
        SystemInstruction::CreateAccount {
            lamports,
            space,
            owner,
        } => {
            check_accounts(instruction, 2)?;
            create_account(accounts, instruction, signers, lamports, space, &owner)?;
        }
        SystemInstruction::Transfer { lamports } => {
            check_accounts(instruction, 2)?;
            transfer(accounts, instruction, signers, lamports)?;
        }
        SystemInstruction::Assign { owner } => {
            check_accounts(instruction, 1)?;
            assign(accounts, instruction, signers, &owner, None)?;
        }
        SystemInstruction::Allocate { space } => {
            check_accounts(instruction, 1)?;
            allocate(accounts, instruction, signers, space, None)?;
        }
        SystemInstruction::CreateAccountWithSeed {
            base,
            seed,
            lamports,
            space,
            owner,
        } => {
            check_accounts(instruction, 2)?;
            let params = WithSeedParams {
                base: &base,
                seed: &seed,
                lamports,
                space,
                owner: &owner,
            };
            create_account_with_seed(accounts, instruction, signers, &params)?;
        }
        SystemInstruction::TransferWithSeed {
            lamports,
            from_seed,
            from_owner,
        } => {
            check_accounts(instruction, 3)?;
            transfer_with_seed(
                accounts,
                instruction,
                signers,
                lamports,
                &from_seed,
                &from_owner,
            )?;
        }
        SystemInstruction::AssignWithSeed { base, seed, owner } => {
            check_accounts(instruction, 1)?;
            assign_with_seed(accounts, instruction, signers, &base, &seed, &owner)?;
        }
        SystemInstruction::AllocateWithSeed {
            base,
            seed,
            space,
            owner,
        } => {
            check_accounts(instruction, 1)?;
            allocate_with_seed(accounts, instruction, signers, &base, &seed, space, &owner)?;
        }
        SystemInstruction::AdvanceNonceAccount
        | SystemInstruction::WithdrawNonceAccount(_)
        | SystemInstruction::InitializeNonceAccount(_)
        | SystemInstruction::AuthorizeNonceAccount(_)
        | SystemInstruction::UpgradeNonceAccount => {}
        _ => {
            return Err(RuntimeError::BuiltinError(
                "unsupported system instruction".into(),
            ));
        }
    }

    Ok(SYSTEM_PROGRAM_COMPUTE_UNITS)
}

fn check_accounts(instruction: &CpiRequest, required: usize) -> RuntimeResult<()> {
    if instruction.accounts.len() < required {
        return Err(RuntimeError::BuiltinError("not enough account keys".into()));
    }
    Ok(())
}

fn is_signer(pubkey: &Address, meta: &CpiAccountMeta, derived_signers: &[Address]) -> bool {
    meta.is_signer || derived_signers.contains(pubkey)
}

struct WithSeedParams<'a> {
    base: &'a Address,
    seed: &'a str,
    lamports: u64,
    space: u64,
    owner: &'a Address,
}

fn verify_address_with_seed(
    address: &Address,
    base: &Address,
    seed: &str,
    owner: &Address,
) -> RuntimeResult<()> {
    let expected = Address::create_with_seed(base, seed, owner)
        .map_err(|_| RuntimeError::BuiltinError("invalid seeds".into()))?;
    if *address != expected {
        return Err(RuntimeError::BuiltinError(
            "address with seed mismatch".into(),
        ));
    }
    Ok(())
}

fn create_account(
    accounts: &mut HashMap<Address, Account>,
    instruction: &CpiRequest,
    signers: &[Address],
    lamports: u64,
    space: u64,
    owner: &Address,
) -> RuntimeResult<()> {
    let new_key = instruction.accounts[1].pubkey;

    if accounts.get(&new_key).is_some_and(|a| a.lamports > 0) {
        return Err(RuntimeError::BuiltinError("account already in use".into()));
    }

    allocate(accounts, instruction, signers, space, Some(1))?;
    assign(accounts, instruction, signers, owner, Some(1))?;
    transfer(accounts, instruction, signers, lamports)?;

    Ok(())
}

fn create_account_with_seed(
    accounts: &mut HashMap<Address, Account>,
    instruction: &CpiRequest,
    signers: &[Address],
    params: &WithSeedParams,
) -> RuntimeResult<()> {
    let new_key = instruction.accounts[1].pubkey;

    verify_address_with_seed(&new_key, params.base, params.seed, params.owner)?;

    if accounts.get(&new_key).is_some_and(|a| a.lamports > 0) {
        return Err(RuntimeError::BuiltinError("account already in use".into()));
    }

    allocate_inner(accounts, &new_key, params.space, signers, Some(params.base))?;
    assign_inner(accounts, &new_key, params.owner, signers, Some(params.base))?;
    transfer(accounts, instruction, signers, params.lamports)?;

    Ok(())
}

fn transfer(
    accounts: &mut HashMap<Address, Account>,
    instruction: &CpiRequest,
    signers: &[Address],
    lamports: u64,
) -> RuntimeResult<()> {
    let from_key = instruction.accounts[0].pubkey;
    let to_key = instruction.accounts[1].pubkey;

    if !is_signer(&from_key, &instruction.accounts[0], signers) {
        return Err(RuntimeError::BuiltinError(
            "missing required signature for transfer".into(),
        ));
    }

    let from_account = accounts
        .get_mut(&from_key)
        .ok_or_else(|| RuntimeError::MissingAccount(from_key.to_string()))?;
    if !from_account.data.is_empty() {
        return Err(RuntimeError::BuiltinError(
            "transfer: from account carries data".into(),
        ));
    }
    if from_account.lamports < lamports {
        return Err(RuntimeError::BuiltinError("insufficient funds".into()));
    }
    from_account.lamports -= lamports;

    let to_account = accounts
        .get_mut(&to_key)
        .ok_or_else(|| RuntimeError::MissingAccount(to_key.to_string()))?;
    to_account.lamports = to_account
        .lamports
        .checked_add(lamports)
        .ok_or_else(|| RuntimeError::BuiltinError("arithmetic overflow".into()))?;

    Ok(())
}

fn transfer_with_seed(
    accounts: &mut HashMap<Address, Account>,
    instruction: &CpiRequest,
    signers: &[Address],
    lamports: u64,
    from_seed: &str,
    from_owner: &Address,
) -> RuntimeResult<()> {
    let from_key = instruction.accounts[0].pubkey;
    let base_key = instruction.accounts[1].pubkey;
    let to_key = instruction.accounts[2].pubkey;

    if !is_signer(&base_key, &instruction.accounts[1], signers) {
        return Err(RuntimeError::BuiltinError(
            "missing required signature for transfer_with_seed".into(),
        ));
    }

    verify_address_with_seed(&from_key, &base_key, from_seed, from_owner)?;

    let from_account = accounts
        .get_mut(&from_key)
        .ok_or_else(|| RuntimeError::MissingAccount(from_key.to_string()))?;
    if !from_account.data.is_empty() {
        return Err(RuntimeError::BuiltinError(
            "transfer: from account carries data".into(),
        ));
    }
    if from_account.lamports < lamports {
        return Err(RuntimeError::BuiltinError("insufficient funds".into()));
    }
    from_account.lamports -= lamports;

    let to_account = accounts
        .get_mut(&to_key)
        .ok_or_else(|| RuntimeError::MissingAccount(to_key.to_string()))?;
    to_account.lamports = to_account
        .lamports
        .checked_add(lamports)
        .ok_or_else(|| RuntimeError::BuiltinError("arithmetic overflow".into()))?;

    Ok(())
}

fn assign(
    accounts: &mut HashMap<Address, Account>,
    instruction: &CpiRequest,
    signers: &[Address],
    owner: &Address,
    account_index: Option<usize>,
) -> RuntimeResult<()> {
    let idx = account_index.unwrap_or(0);
    let account_key = instruction.accounts[idx].pubkey;
    assign_inner(accounts, &account_key, owner, signers, None)
}

fn assign_inner(
    accounts: &mut HashMap<Address, Account>,
    account_key: &Address,
    owner: &Address,
    signers: &[Address],
    base: Option<&Address>,
) -> RuntimeResult<()> {
    let account = accounts
        .get_mut(account_key)
        .ok_or_else(|| RuntimeError::MissingAccount(account_key.to_string()))?;
    if account.owner == *owner {
        return Ok(());
    }

    let signer_key = base.unwrap_or(account_key);
    if !signers.contains(signer_key) {
        return Err(RuntimeError::BuiltinError(
            "missing required signature for assign".into(),
        ));
    }
    account.owner = *owner;
    Ok(())
}

fn assign_with_seed(
    accounts: &mut HashMap<Address, Account>,
    instruction: &CpiRequest,
    signers: &[Address],
    base: &Address,
    seed: &str,
    owner: &Address,
) -> RuntimeResult<()> {
    let account_key = instruction.accounts[0].pubkey;
    verify_address_with_seed(&account_key, base, seed, owner)?;
    assign_inner(accounts, &account_key, owner, signers, Some(base))
}

fn allocate(
    accounts: &mut HashMap<Address, Account>,
    instruction: &CpiRequest,
    signers: &[Address],
    space: u64,
    account_index: Option<usize>,
) -> RuntimeResult<()> {
    let idx = account_index.unwrap_or(0);
    let account_key = instruction.accounts[idx].pubkey;
    allocate_inner(accounts, &account_key, space, signers, None)
}

fn allocate_inner(
    accounts: &mut HashMap<Address, Account>,
    account_key: &Address,
    space: u64,
    signers: &[Address],
    base: Option<&Address>,
) -> RuntimeResult<()> {
    let signer_key = base.unwrap_or(account_key);
    if !signers.contains(signer_key) {
        return Err(RuntimeError::BuiltinError(
            "missing required signature for allocate".into(),
        ));
    }

    if space > MAX_PERMITTED_DATA_LENGTH {
        return Err(RuntimeError::BuiltinError(
            "invalid account data: exceeds max length".into(),
        ));
    }

    let account = accounts
        .get_mut(account_key)
        .ok_or_else(|| RuntimeError::MissingAccount(account_key.to_string()))?;

    if !account.data.is_empty() || !system_program::check_id(&account.owner) {
        return Err(RuntimeError::BuiltinError("account already in use".into()));
    }

    account.data = vec![0u8; space as usize];
    Ok(())
}

fn allocate_with_seed(
    accounts: &mut HashMap<Address, Account>,
    instruction: &CpiRequest,
    signers: &[Address],
    base: &Address,
    seed: &str,
    space: u64,
    owner: &Address,
) -> RuntimeResult<()> {
    let account_key = instruction.accounts[0].pubkey;
    verify_address_with_seed(&account_key, base, seed, owner)?;
    allocate_inner(accounts, &account_key, space, signers, Some(base))?;
    assign_inner(accounts, &account_key, owner, signers, Some(base))
}

#[cfg(test)]
mod tests {
    use {super::*, solana_system_interface::instruction::SystemInstruction};

    const COMPUTE_BUDGET: u64 = 200_000;

    fn addr(seed: u8) -> Address {
        Address::new_from_array([seed; 32])
    }

    fn make_account(lamports: u64) -> Account {
        Account {
            lamports,
            data: vec![],
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        }
    }

    fn make_request(accounts: Vec<CpiAccountMeta>, ix: &SystemInstruction) -> CpiRequest {
        CpiRequest {
            program_id: system_program::ID,
            accounts,
            data: bincode::serialize(ix).unwrap(),
            caller_accounts: Vec::new(),
            signers: Vec::new(),
        }
    }

    fn meta(pubkey: Address, is_signer: bool, is_writable: bool) -> CpiAccountMeta {
        CpiAccountMeta {
            pubkey,
            is_signer,
            is_writable,
        }
    }

    #[test]
    fn test_transfer() {
        let from = addr(1);
        let to = addr(2);
        let mut accounts = HashMap::from([(from, make_account(1_000_000)), (to, make_account(0))]);
        let request = make_request(
            vec![meta(from, true, true), meta(to, false, true)],
            &SystemInstruction::Transfer { lamports: 500_000 },
        );

        let cu = process(&mut accounts, &request, &[from], COMPUTE_BUDGET).unwrap();

        assert_eq!(cu, SYSTEM_PROGRAM_COMPUTE_UNITS);
        assert_eq!(accounts[&from].lamports, 500_000);
        assert_eq!(accounts[&to].lamports, 500_000);
    }

    #[test]
    fn test_transfer_insufficient_funds() {
        let from = addr(1);
        let to = addr(2);
        let mut accounts = HashMap::from([(from, make_account(100)), (to, make_account(0))]);
        let request = make_request(
            vec![meta(from, true, true), meta(to, false, true)],
            &SystemInstruction::Transfer { lamports: 500 },
        );

        let err = process(&mut accounts, &request, &[from], COMPUTE_BUDGET).unwrap_err();
        assert!(err.to_string().contains("insufficient funds"));
    }

    #[test]
    fn test_transfer_missing_signer() {
        let from = addr(1);
        let to = addr(2);
        let mut accounts = HashMap::from([(from, make_account(1_000_000)), (to, make_account(0))]);
        let request = make_request(
            vec![meta(from, false, true), meta(to, false, true)],
            &SystemInstruction::Transfer { lamports: 500 },
        );

        // from is not a signer and not in the signers list
        let err = process(&mut accounts, &request, &[], COMPUTE_BUDGET).unwrap_err();
        assert!(err.to_string().contains("missing required signature"));
    }

    #[test]
    fn test_create_account() {
        let payer = addr(1);
        let new_account = addr(2);
        let owner = addr(3);
        let mut accounts = HashMap::from([
            (payer, make_account(1_000_000)),
            (new_account, make_account(0)),
        ]);
        let request = make_request(
            vec![meta(payer, true, true), meta(new_account, true, true)],
            &SystemInstruction::CreateAccount {
                lamports: 100_000,
                space: 128,
                owner,
            },
        );

        process(
            &mut accounts,
            &request,
            &[payer, new_account],
            COMPUTE_BUDGET,
        )
        .unwrap();

        assert_eq!(accounts[&payer].lamports, 900_000);
        assert_eq!(accounts[&new_account].lamports, 100_000);
        assert_eq!(accounts[&new_account].data.len(), 128);
        assert_eq!(accounts[&new_account].owner, owner);
    }

    #[test]
    fn test_assign() {
        let account_key = addr(1);
        let new_owner = addr(2);
        let mut accounts = HashMap::from([(account_key, make_account(0))]);
        let request = make_request(
            vec![meta(account_key, true, true)],
            &SystemInstruction::Assign { owner: new_owner },
        );

        process(&mut accounts, &request, &[account_key], COMPUTE_BUDGET).unwrap();

        assert_eq!(accounts[&account_key].owner, new_owner);
    }

    #[test]
    fn test_allocate() {
        let account_key = addr(1);
        let mut accounts = HashMap::from([(account_key, make_account(0))]);
        let request = make_request(
            vec![meta(account_key, true, true)],
            &SystemInstruction::Allocate { space: 256 },
        );

        process(&mut accounts, &request, &[account_key], COMPUTE_BUDGET).unwrap();

        assert_eq!(accounts[&account_key].data.len(), 256);
    }

    #[test]
    fn test_allocate_already_in_use() {
        let account_key = addr(1);
        let mut accounts = HashMap::from([(
            account_key,
            Account {
                lamports: 0,
                data: vec![1, 2, 3],
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            },
        )]);
        let request = make_request(
            vec![meta(account_key, true, true)],
            &SystemInstruction::Allocate { space: 256 },
        );

        let err = process(&mut accounts, &request, &[account_key], COMPUTE_BUDGET).unwrap_err();
        assert!(err.to_string().contains("account already in use"));
    }

    #[test]
    fn test_insufficient_compute_units() {
        let from = addr(1);
        let to = addr(2);
        let mut accounts = HashMap::from([(from, make_account(1_000_000)), (to, make_account(0))]);
        let request = make_request(
            vec![meta(from, true, true), meta(to, false, true)],
            &SystemInstruction::Transfer { lamports: 100 },
        );

        let err = process(&mut accounts, &request, &[from], 10).unwrap_err();
        assert!(err.to_string().contains("insufficient compute units"));
    }

    #[test]
    fn test_invalid_instruction_data() {
        let mut accounts: HashMap<Address, Account> = HashMap::new();
        let request = CpiRequest {
            program_id: system_program::ID,
            accounts: vec![],
            data: vec![0xff, 0xff, 0xff, 0xff, 0xff],
            caller_accounts: Vec::new(),
            signers: Vec::new(),
        };
        let err = process(&mut accounts, &request, &[], COMPUTE_BUDGET).unwrap_err();
        assert!(err.to_string().contains("invalid system instruction data"));
    }

    #[test]
    fn test_not_enough_accounts() {
        let from = addr(1);
        let mut accounts = HashMap::from([(from, make_account(1_000_000))]);
        let request = make_request(
            vec![meta(from, true, true)],
            &SystemInstruction::Transfer { lamports: 100 },
        );
        let err = process(&mut accounts, &request, &[from], COMPUTE_BUDGET).unwrap_err();
        assert!(err.to_string().contains("not enough account keys"));
    }

    #[test]
    fn test_nonce_instruction_is_accepted_as_noop() {
        let mut accounts: HashMap<Address, Account> = HashMap::new();
        let request = make_request(vec![], &SystemInstruction::AdvanceNonceAccount);
        let cu = process(&mut accounts, &request, &[], COMPUTE_BUDGET).unwrap();
        assert_eq!(cu, SYSTEM_PROGRAM_COMPUTE_UNITS);
        assert!(accounts.is_empty());
    }

    #[test]
    fn test_transfer_from_missing_account() {
        let from = addr(1);
        let to = addr(2);
        let mut accounts = HashMap::from([(to, make_account(0))]);
        let request = make_request(
            vec![meta(from, true, true), meta(to, false, true)],
            &SystemInstruction::Transfer { lamports: 100 },
        );
        let err = process(&mut accounts, &request, &[from], COMPUTE_BUDGET).unwrap_err();
        assert!(matches!(err, RuntimeError::MissingAccount(_)));
    }

    #[test]
    fn test_transfer_to_missing_account() {
        let from = addr(1);
        let to = addr(2);
        let mut accounts = HashMap::from([(from, make_account(1_000_000))]);
        let request = make_request(
            vec![meta(from, true, true), meta(to, false, true)],
            &SystemInstruction::Transfer { lamports: 100 },
        );
        let err = process(&mut accounts, &request, &[from], COMPUTE_BUDGET).unwrap_err();
        assert!(matches!(err, RuntimeError::MissingAccount(_)));
    }

    #[test]
    fn test_transfer_from_carries_data() {
        let from = addr(1);
        let to = addr(2);
        let mut from_acc = make_account(1_000_000);
        from_acc.data = vec![1, 2, 3];
        let mut accounts = HashMap::from([(from, from_acc), (to, make_account(0))]);
        let request = make_request(
            vec![meta(from, true, true), meta(to, false, true)],
            &SystemInstruction::Transfer { lamports: 100 },
        );
        let err = process(&mut accounts, &request, &[from], COMPUTE_BUDGET).unwrap_err();
        assert!(err.to_string().contains("from account carries data"));
    }

    #[test]
    fn test_transfer_signer_via_derived_signers() {
        let from = addr(1);
        let to = addr(2);
        let mut accounts = HashMap::from([(from, make_account(1_000_000)), (to, make_account(0))]);
        let request = make_request(
            vec![meta(from, false, true), meta(to, false, true)],
            &SystemInstruction::Transfer { lamports: 500_000 },
        );
        process(&mut accounts, &request, &[from], COMPUTE_BUDGET).unwrap();
        assert_eq!(accounts[&from].lamports, 500_000);
        assert_eq!(accounts[&to].lamports, 500_000);
    }

    #[test]
    fn test_create_account_already_in_use() {
        let payer = addr(1);
        let new_account = addr(2);
        let owner = addr(3);
        let mut accounts = HashMap::from([
            (payer, make_account(1_000_000)),
            (new_account, make_account(42)),
        ]);
        let request = make_request(
            vec![meta(payer, true, true), meta(new_account, true, true)],
            &SystemInstruction::CreateAccount {
                lamports: 100_000,
                space: 128,
                owner,
            },
        );
        let err = process(
            &mut accounts,
            &request,
            &[payer, new_account],
            COMPUTE_BUDGET,
        )
        .unwrap_err();
        assert!(err.to_string().contains("account already in use"));
    }

    #[test]
    fn test_assign_already_owned() {
        let account_key = addr(1);
        let mut accounts = HashMap::from([(account_key, make_account(0))]);
        let request = make_request(
            vec![meta(account_key, false, true)],
            &SystemInstruction::Assign {
                owner: system_program::ID,
            },
        );
        process(&mut accounts, &request, &[], COMPUTE_BUDGET).unwrap();
        assert_eq!(accounts[&account_key].owner, system_program::ID);
    }

    #[test]
    fn test_assign_missing_signer() {
        let account_key = addr(1);
        let new_owner = addr(2);
        let mut accounts = HashMap::from([(account_key, make_account(0))]);
        let request = make_request(
            vec![meta(account_key, false, true)],
            &SystemInstruction::Assign { owner: new_owner },
        );
        let err = process(&mut accounts, &request, &[], COMPUTE_BUDGET).unwrap_err();
        assert!(
            err.to_string()
                .contains("missing required signature for assign")
        );
    }

    #[test]
    fn test_allocate_missing_signer() {
        let account_key = addr(1);
        let mut accounts = HashMap::from([(account_key, make_account(0))]);
        let request = make_request(
            vec![meta(account_key, false, true)],
            &SystemInstruction::Allocate { space: 64 },
        );
        let err = process(&mut accounts, &request, &[], COMPUTE_BUDGET).unwrap_err();
        assert!(
            err.to_string()
                .contains("missing required signature for allocate")
        );
    }

    #[test]
    fn test_allocate_exceeds_max_length() {
        let account_key = addr(1);
        let mut accounts = HashMap::from([(account_key, make_account(0))]);
        let request = make_request(
            vec![meta(account_key, true, true)],
            &SystemInstruction::Allocate {
                space: MAX_PERMITTED_DATA_LENGTH + 1,
            },
        );
        let err = process(&mut accounts, &request, &[account_key], COMPUTE_BUDGET).unwrap_err();
        assert!(err.to_string().contains("exceeds max length"));
    }

    #[test]
    fn test_create_account_with_seed() {
        let payer = addr(1);
        let base = addr(4);
        let owner = addr(3);
        let seed = "vault";
        let derived = Address::create_with_seed(&base, seed, &owner).unwrap();
        let mut accounts =
            HashMap::from([(payer, make_account(1_000_000)), (derived, make_account(0))]);
        let request = make_request(
            vec![meta(payer, true, true), meta(derived, false, true)],
            &SystemInstruction::CreateAccountWithSeed {
                base,
                seed: seed.to_string(),
                lamports: 100_000,
                space: 64,
                owner,
            },
        );
        process(&mut accounts, &request, &[payer, base], COMPUTE_BUDGET).unwrap();
        assert_eq!(accounts[&payer].lamports, 900_000);
        assert_eq!(accounts[&derived].lamports, 100_000);
        assert_eq!(accounts[&derived].data.len(), 64);
        assert_eq!(accounts[&derived].owner, owner);
    }

    #[test]
    fn test_create_account_with_seed_mismatch() {
        let payer = addr(1);
        let base = addr(4);
        let owner = addr(3);
        let wrong = addr(9);
        let mut accounts =
            HashMap::from([(payer, make_account(1_000_000)), (wrong, make_account(0))]);
        let request = make_request(
            vec![meta(payer, true, true), meta(wrong, false, true)],
            &SystemInstruction::CreateAccountWithSeed {
                base,
                seed: "vault".to_string(),
                lamports: 100_000,
                space: 64,
                owner,
            },
        );
        let err = process(&mut accounts, &request, &[payer, base], COMPUTE_BUDGET).unwrap_err();
        assert!(err.to_string().contains("address with seed mismatch"));
    }

    #[test]
    fn test_transfer_with_seed() {
        let base = addr(4);
        let from_owner = addr(5);
        let to = addr(2);
        let seed = "wallet";
        let from = Address::create_with_seed(&base, seed, &from_owner).unwrap();
        let mut accounts = HashMap::from([
            (from, make_account(1_000_000)),
            (base, make_account(0)),
            (to, make_account(0)),
        ]);
        let request = make_request(
            vec![
                meta(from, false, true),
                meta(base, true, true),
                meta(to, false, true),
            ],
            &SystemInstruction::TransferWithSeed {
                lamports: 400_000,
                from_seed: seed.to_string(),
                from_owner,
            },
        );
        process(&mut accounts, &request, &[base], COMPUTE_BUDGET).unwrap();
        assert_eq!(accounts[&from].lamports, 600_000);
        assert_eq!(accounts[&to].lamports, 400_000);
    }

    #[test]
    fn test_transfer_with_seed_missing_signer() {
        let base = addr(4);
        let from_owner = addr(5);
        let to = addr(2);
        let seed = "wallet";
        let from = Address::create_with_seed(&base, seed, &from_owner).unwrap();
        let mut accounts = HashMap::from([
            (from, make_account(1_000_000)),
            (base, make_account(0)),
            (to, make_account(0)),
        ]);
        let request = make_request(
            vec![
                meta(from, false, true),
                meta(base, false, true),
                meta(to, false, true),
            ],
            &SystemInstruction::TransferWithSeed {
                lamports: 400_000,
                from_seed: seed.to_string(),
                from_owner,
            },
        );
        let err = process(&mut accounts, &request, &[], COMPUTE_BUDGET).unwrap_err();
        assert!(
            err.to_string()
                .contains("missing required signature for transfer_with_seed")
        );
    }

    #[test]
    fn test_assign_with_seed() {
        let base = addr(4);
        let owner = addr(3);
        let seed = "state";
        let account_key = Address::create_with_seed(&base, seed, &owner).unwrap();
        let mut accounts = HashMap::from([(account_key, make_account(0))]);
        let request = make_request(
            vec![meta(account_key, false, true)],
            &SystemInstruction::AssignWithSeed {
                base,
                seed: seed.to_string(),
                owner,
            },
        );
        process(&mut accounts, &request, &[base], COMPUTE_BUDGET).unwrap();
        assert_eq!(accounts[&account_key].owner, owner);
    }

    #[test]
    fn test_allocate_with_seed() {
        let base = addr(4);
        let owner = addr(3);
        let seed = "data";
        let account_key = Address::create_with_seed(&base, seed, &owner).unwrap();
        let mut accounts = HashMap::from([(account_key, make_account(0))]);
        let request = make_request(
            vec![meta(account_key, false, true)],
            &SystemInstruction::AllocateWithSeed {
                base,
                seed: seed.to_string(),
                space: 96,
                owner,
            },
        );
        process(&mut accounts, &request, &[base], COMPUTE_BUDGET).unwrap();
        assert_eq!(accounts[&account_key].data.len(), 96);
        assert_eq!(accounts[&account_key].owner, owner);
    }
}
