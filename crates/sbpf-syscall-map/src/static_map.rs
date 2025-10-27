use crate::murmur3_32;

/// Static syscall map using lifetimes for compile-time and borrowed data
/// Supports both static (compile-time) and dynamic (runtime) syscall lists via lifetimes
pub struct SyscallMap<'a> {
    pub(crate) entries: &'a [(u32, &'a str)],
}

impl<'a> SyscallMap<'a> {
    /// Create from a pre-sorted slice of (hash, name) pairs
    /// Works for both static and dynamic lifetimes
    pub const fn from_entries(entries: &'a [(u32, &'a str)]) -> Self {
        // Check for hash conflicts
        let mut i = 0;
        while i < entries.len() - 1 {
            if entries[i].0 == entries[i + 1].0 {
                panic!("Hash conflict detected between syscalls");
            }
            i += 1;
        }

        Self { entries }
    }

    pub const fn get(&self, hash: u32) -> Option<&'a str> {
        // Binary search in const context
        let mut left = 0;
        let mut right = self.entries.len();

        while left < right {
            let mid = (left + right) / 2;
            if self.entries[mid].0 == hash {
                return Some(self.entries[mid].1);
            } else if self.entries[mid].0 < hash {
                left = mid + 1;
            } else {
                right = mid;
            }
        }
        None
    }

    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Helper function for compile-time syscall map creation
/// Computes hashes and sorts entries at compile time
pub const fn compute_syscall_entries_const<'a, const N: usize>(
    syscalls: &'a [&'a str; N],
) -> [(u32, &'a str); N] {
    let mut entries: [(u32, &str); N] = [(0, ""); N];
    let mut i = 0;
    while i < N {
        entries[i] = (murmur3_32(syscalls[i]), syscalls[i]);
        i += 1;
    }

    // Sort the entries at compile time using bubble sort
    let mut i = 0;
    while i < N {
        let mut j = 0;
        while j < N - i - 1 {
            if entries[j].0 > entries[j + 1].0 {
                let temp = entries[j];
                entries[j] = entries[j + 1];
                entries[j + 1] = temp;
            }
            j += 1;
        }
        i += 1;
    }

    entries
}

/// Runtime helper for dynamic syscall lists
/// Computes hashes and sorts entries, borrowing from the input
///
/// The caller must own the string data (e.g., Vec<String>) and pass references.
/// This function returns references to those owned strings.
pub fn compute_syscall_entries<'a, T: AsRef<str>>(syscalls: &'a [T]) -> Vec<(u32, &'a str)> {
    let mut entries: Vec<(u32, &'a str)> = syscalls
        .iter()
        .map(|name| (murmur3_32(name.as_ref()), name.as_ref()))
        .collect();

    entries.sort_by_key(|(hash, _)| *hash);

    // Check for conflicts
    for i in 0..entries.len().saturating_sub(1) {
        if entries[i].0 == entries[i + 1].0 {
            panic!(
                "Hash conflict detected between syscalls '{}' and '{}'",
                entries[i].1,
                entries[i + 1].1
            );
        }
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_const_evaluation() {
        // Verify const evaluation works at compile time
        const ABORT_HASH: u32 = murmur3_32("abort");
        const SOL_LOG_HASH: u32 = murmur3_32("sol_log_");

        // Create a test map
        const TEST_SYSCALLS: &[&str; 2] = &["abort", "sol_log_"];
        const TEST_ENTRIES: &[(u32, &str); 2] = &compute_syscall_entries_const(TEST_SYSCALLS);
        const TEST_MAP: SyscallMap<'static> = SyscallMap::from_entries(TEST_ENTRIES);

        // Verify the hashes are computed correctly and can look up syscalls
        assert_eq!(TEST_MAP.get(ABORT_HASH), Some("abort"));
        assert_eq!(TEST_MAP.get(SOL_LOG_HASH), Some("sol_log_"));
    }

    #[test]
    fn test_nonexistent_syscall() {
        // Test that non-existent syscalls return None
        const TEST_SYSCALLS: &[&str; 1] = &["test"];
        const TEST_ENTRIES: &[(u32, &str); 1] = &compute_syscall_entries_const(TEST_SYSCALLS);
        const TEST_MAP: SyscallMap<'static> = SyscallMap::from_entries(TEST_ENTRIES);

        assert_eq!(TEST_MAP.get(0xDEADBEEF), None);
    }

    #[test]
    fn test_dynamic_syscalls() {
        // Example: Create a dynamic syscall map with owned strings
        // The caller owns the strings (e.g., from user input, config file, etc.)
        let owned_syscalls: Vec<String> = vec![
            String::from("my_custom_syscall"),
            String::from("another_syscall"),
        ];

        // Compute entries - they borrow from owned_syscalls
        let entries = compute_syscall_entries(&owned_syscalls);

        // Create the map - it borrows from entries
        let map = SyscallMap::from_entries(&entries);

        // Verify lookups work
        let hash1 = murmur3_32("my_custom_syscall");
        let hash2 = murmur3_32("another_syscall");

        assert_eq!(map.get(hash1), Some("my_custom_syscall"));
        assert_eq!(map.get(hash2), Some("another_syscall"));

        // The lifetimes ensure owned_syscalls outlives both entries and map
    }

    #[test]
    fn test_dynamic_syscalls_with_str_slices() {
        // Also works with &str slices
        let syscalls: Vec<&str> = vec!["syscall_a", "syscall_b", "syscall_c"];

        let entries = compute_syscall_entries(&syscalls);
        let map = SyscallMap::from_entries(&entries);

        assert_eq!(map.get(murmur3_32("syscall_a")), Some("syscall_a"));
        assert_eq!(map.get(murmur3_32("syscall_b")), Some("syscall_b"));
        assert_eq!(map.get(murmur3_32("syscall_c")), Some("syscall_c"));
    }

    #[test]
    fn test_static_custom_map() {
        // Example: Create a static custom syscall map at compile time
        const CUSTOM_SYSCALLS: &[&str; 2] = &["test1", "test2"];
        const CUSTOM_ENTRIES: &[(u32, &str); 2] = &compute_syscall_entries_const(CUSTOM_SYSCALLS);
        const CUSTOM_MAP: SyscallMap<'static> = SyscallMap::from_entries(CUSTOM_ENTRIES);

        assert_eq!(CUSTOM_MAP.get(murmur3_32("test1")), Some("test1"));
        assert_eq!(CUSTOM_MAP.get(murmur3_32("test2")), Some("test2"));
    }
}
