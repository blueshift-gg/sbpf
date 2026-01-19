use crate::syscalls_map::{SyscallMap, compute_syscall_entries_const, murmur3_32};

/// Libcalls are functions that LLVM emits when expanding intrinsics that the target
/// doesn't support natively. 
/// Each entry maps a libcall name to its corresponding Solana syscall name.
/// - libcall_name: The libcall function name (e.g., "__multi3" for 128-bit multiplication)
/// - syscall_name: The Solana syscall that implements the libcall (e.g., "sol_multi3")
pub const REGISTERED_LIBCALLS: &[(&str, &str)] = &[
    ("__multi3", "sol_multi3"), // 128-bit multiplication
];

/// Check if a name is a registered libcall
pub fn is_registered_libcall(name: &str) -> bool {
    REGISTERED_LIBCALLS
        .iter()
        .any(|(libcall, _)| *libcall == name)
}

/// Get the syscall hash for a libcall by computing murmur3_32 of the mapped syscall name.
/// Returns None if not a registered libcall.
pub fn libcall_hash(name: &str) -> Option<u32> {
    REGISTERED_LIBCALLS
        .iter()
        .find(|(libcall, _)| *libcall == name)
        .map(|(_, syscall)| murmur3_32(syscall))
}

pub const REGISTERED_SYSCALLS: &[&str] = &[
    "abort",
    "sol_panic_",
    "sol_log_",
    "sol_log_64_",
    "sol_log_compute_units_",
    "sol_log_pubkey",
    "sol_create_program_address",
    "sol_try_find_program_address",
    "sol_sha256",
    "sol_keccak256",
    "sol_secp256k1_recover",
    "sol_blake3",
    "sol_curve_validate_point",
    "sol_curve_group_op",
    "sol_get_clock_sysvar",
    "sol_get_epoch_schedule_sysvar",
    "sol_get_fees_sysvar",
    "sol_get_rent_sysvar",
    "sol_memcpy_",
    "sol_memmove_",
    "sol_memcmp_",
    "sol_memset_",
    "sol_invoke_signed_c",
    "sol_invoke_signed_rust",
    "sol_alloc_free_",
    "sol_set_return_data",
    "sol_get_return_data",
    "sol_log_data",
    "sol_get_processed_sibling_instruction",
    "sol_get_stack_height",
];

pub static SYSCALLS: SyscallMap<'static> =
    SyscallMap::from_entries(&compute_syscall_entries_const::<
        { REGISTERED_SYSCALLS.len() },
    >(REGISTERED_SYSCALLS));
