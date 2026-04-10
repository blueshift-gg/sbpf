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
