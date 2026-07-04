pub mod system;

use {
    crate::{
        cpi::request::CpiRequest,
        errors::{RuntimeError, RuntimeResult},
    },
    solana_account::Account,
    solana_address::Address,
    solana_system_interface::program as system_program,
    std::collections::HashMap,
};

pub const SYSTEM_PROGRAM_ID: Address = system_program::ID;

pub fn is_builtin(program_id: &Address) -> bool {
    *program_id == SYSTEM_PROGRAM_ID
}

pub fn execute_builtin(
    program_id: &Address,
    accounts: &mut HashMap<Address, Account>,
    instruction: &CpiRequest,
    signers: &[Address],
    compute_remaining: u64,
) -> RuntimeResult<u64> {
    if *program_id == SYSTEM_PROGRAM_ID {
        system::process(accounts, instruction, signers, compute_remaining)
    } else {
        Err(RuntimeError::BuiltinError(format!(
            "unsupported builtin program: {}",
            program_id
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(seed: u8) -> Address {
        Address::new_from_array([seed; 32])
    }

    #[test]
    fn is_builtin_recognizes_system_program() {
        assert!(is_builtin(&SYSTEM_PROGRAM_ID));
    }

    #[test]
    fn is_builtin_rejects_other_programs() {
        assert!(!is_builtin(&addr(9)));
    }

    #[test]
    fn execute_builtin_rejects_unsupported_program() {
        let mut accounts: HashMap<Address, Account> = HashMap::new();
        let request = CpiRequest {
            program_id: addr(9),
            accounts: Vec::new(),
            data: Vec::new(),
            caller_accounts: Vec::new(),
            signers: Vec::new(),
        };
        let err = execute_builtin(&addr(9), &mut accounts, &request, &[], 200_000).unwrap_err();
        assert!(matches!(err, RuntimeError::BuiltinError(_)));
        assert!(err.to_string().contains("unsupported builtin program"));
    }
}
