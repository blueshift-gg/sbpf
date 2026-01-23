use syscall_map::murmur3_32;

// Simple const hashmap implementation using binary search on sorted array
// Supports both static (compile-time) and dynamic (runtime) syscall lists via lifetimes
pub struct SyscallMap<'a> {
    entries: &'a [(u32, &'a str)],
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

/// Runtime-mutable syscall map that owns its data
/// This allows for dynamic updates at runtime
pub struct DynamicSyscallMap {
    // Store entries as (hash, name) pairs, kept sorted by hash
    entries: Vec<(u32, String)>,
}

impl DynamicSyscallMap {
    /// Create a new dynamic syscall map from owned strings
    pub fn new(syscalls: Vec<String>) -> Result<Self, String> {
        let mut entries: Vec<(u32, String)> = syscalls
            .into_iter()
            .map(|name| (murmur3_32(&name), name))
            .collect();

        entries.sort_by_key(|(hash, _)| *hash);

        // Check for conflicts
        for i in 0..entries.len().saturating_sub(1) {
            if entries[i].0 == entries[i + 1].0 {
                return Err(format!(
                    "Hash conflict detected between syscalls '{}' and '{}'",
                    entries[i].1,
                    entries[i + 1].1
                ));
            }
        }

        Ok(Self { entries })
    }

    /// Create from string slices (convenience method)
    pub fn from_names(names: &[&str]) -> Result<Self, String> {
        Self::new(names.iter().map(|&s| s.to_string()).collect())
    }

    /// Look up a syscall by hash
    pub fn get(&self, hash: u32) -> Option<&str> {
        match self.entries.binary_search_by_key(&hash, |(h, _)| *h) {
            Ok(idx) => Some(&self.entries[idx].1),
            Err(_) => None,
        }
    }

    /// Add a new syscall at runtime
    pub fn add(&mut self, name: String) -> Result<(), String> {
        let hash = murmur3_32(&name);

        // Check if it already exists or would conflict
        match self.entries.binary_search_by_key(&hash, |(h, _)| *h) {
            Ok(_) => Err(format!(
                "Hash conflict: '{}' conflicts with existing syscall",
                name
            )),
            Err(pos) => {
                self.entries.insert(pos, (hash, name));
                Ok(())
            }
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Convert a static SyscallMap to a dynamic one
impl<'a> From<&SyscallMap<'a>> for DynamicSyscallMap {
    fn from(static_map: &SyscallMap<'a>) -> Self {
        let entries = static_map
            .entries
            .iter()
            .map(|(hash, name)| (*hash, name.to_string()))
            .collect();

        Self { entries }
    }
}

/// Helper function for compile-time syscall map creation
/// Computes hashes and sorts entries at compile time
pub const fn compute_syscall_entries_const<'a, const N: usize>(
    syscalls: &'a [&'a str],
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
    use {
        super::*,
        crate::syscalls::{REGISTERED_SYSCALLS, SYSCALLS},
        syscall_map::murmur3_32,
    };

    #[test]
    fn test_syscall_lookup() {
        // Test that all syscalls can be found
        for &name in REGISTERED_SYSCALLS.iter() {
            let hash = murmur3_32(name);
            assert_eq!(
                SYSCALLS.get(hash),
                Some(name),
                "Failed to find syscall: {}",
                name
            );
        }
    }

    #[test]
    fn test_const_evaluation() {
        // Verify const evaluation works at compile time
        const ABORT_HASH: u32 = murmur3_32("abort");
        const SOL_LOG_HASH: u32 = murmur3_32("sol_log_");

        // Verify the hashes are computed correctly and can look up syscalls
        assert_eq!(SYSCALLS.get(ABORT_HASH), Some("abort"));
        assert_eq!(SYSCALLS.get(SOL_LOG_HASH), Some("sol_log_"));
    }

    #[test]
    fn test_nonexistent_syscall() {
        // Test that non-existent syscalls return None
        assert_eq!(SYSCALLS.get(0xDEADBEEF), None);
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

    #[test]
    fn test_dynamic_mutable_map() {
        // Example: Create a fully dynamic, mutable syscall map
        let mut map = DynamicSyscallMap::from_names(&["initial_syscall"]).unwrap();

        // Initial lookup works
        assert_eq!(
            map.get(murmur3_32("initial_syscall")),
            Some("initial_syscall")
        );

        // Add new syscalls at runtime
        map.add("runtime_syscall_1".to_string()).unwrap();
        map.add("runtime_syscall_2".to_string()).unwrap();

        // All lookups work
        assert_eq!(
            map.get(murmur3_32("initial_syscall")),
            Some("initial_syscall")
        );
        assert_eq!(
            map.get(murmur3_32("runtime_syscall_1")),
            Some("runtime_syscall_1")
        );
        assert_eq!(
            map.get(murmur3_32("runtime_syscall_2")),
            Some("runtime_syscall_2")
        );

        // Non-existent syscall returns None
        assert_eq!(map.get(0xDEADBEEF), None);

        // Verify count
        assert_eq!(map.len(), 3);
    }

    #[test]
    fn test_dynamic_map_with_owned_strings() {
        // Create from owned strings directly
        let syscalls = vec![
            String::from("custom_1"),
            String::from("custom_2"),
            String::from("custom_3"),
        ];

        let mut map = DynamicSyscallMap::new(syscalls).unwrap();

        assert_eq!(map.get(murmur3_32("custom_1")), Some("custom_1"));
        assert_eq!(map.get(murmur3_32("custom_2")), Some("custom_2"));
        assert_eq!(map.get(murmur3_32("custom_3")), Some("custom_3"));

        // Add more at runtime
        map.add("custom_4".to_string()).unwrap();
        assert_eq!(map.get(murmur3_32("custom_4")), Some("custom_4"));
    }

    #[test]
    fn test_convert_static_to_dynamic() {
        // Start with the static syscall map
        let dynamic = DynamicSyscallMap::from(&SYSCALLS);

        // Verify all static syscalls are present
        for &name in REGISTERED_SYSCALLS.iter() {
            let hash = murmur3_32(name);
            assert_eq!(
                dynamic.get(hash),
                Some(name),
                "Failed to find syscall: {}",
                name
            );
        }

        // Verify we can add new syscalls to it
        let mut dynamic_mut = dynamic;
        dynamic_mut.add("my_custom_syscall".to_string()).unwrap();

        assert_eq!(
            dynamic_mut.get(murmur3_32("my_custom_syscall")),
            Some("my_custom_syscall")
        );

        // Original static syscalls still work
        assert_eq!(dynamic_mut.get(murmur3_32("abort")), Some("abort"));

        // Count should be original + 1
        assert_eq!(dynamic_mut.len(), REGISTERED_SYSCALLS.len() + 1);
    }

    #[test]
    fn test_syscall_map_len_and_is_empty() {
        // Test static map
        assert!(!SYSCALLS.is_empty());

        // Test map with single element
        const SINGLE_ENTRIES: &[(u32, &str)] = &[(123, "test")];
        const SINGLE_MAP: SyscallMap = SyscallMap::from_entries(SINGLE_ENTRIES);
        assert_eq!(SINGLE_MAP.len(), 1);
        assert!(!SINGLE_MAP.is_empty());
    }

    #[test]
    fn test_dynamic_map_len_and_is_empty() {
        // Test empty dynamic map
        let empty_map = DynamicSyscallMap::new(vec![]).unwrap();
        assert_eq!(empty_map.len(), 0);
        assert!(empty_map.is_empty());

        // Test non-empty dynamic map
        let map = DynamicSyscallMap::from_names(&["test"]).unwrap();
        assert_eq!(map.len(), 1);
        assert!(!map.is_empty());
    }

    #[test]
    fn test_dynamic_map_add_duplicate() {
        let mut map = DynamicSyscallMap::from_names(&["existing"]).unwrap();
        let result = map.add("existing".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_dynamic_map_hash_conflict_in_creation() {
        let syscalls = vec![String::from("test"), String::from("test")];
        let result = DynamicSyscallMap::new(syscalls);
        // Should error due to duplicate hash
        assert!(result.is_err());
        if let Err(msg) = result {
            assert!(msg.contains("Hash conflict"));
            assert!(msg.contains("test"));
        }
    }
}
