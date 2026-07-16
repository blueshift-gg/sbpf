mod dynamic_map;
mod hash;
mod static_map;

pub use {
    dynamic_map::DynamicSyscallMap,
    hash::murmur3_32,
    static_map::{SyscallMap, compute_syscall_entries, compute_syscall_entries_const},
};
