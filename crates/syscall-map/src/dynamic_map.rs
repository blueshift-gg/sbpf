use crate::{SyscallMap, murmur3_32};

/// Runtime-mutable syscall map that owns its data
/// This allows for dynamic updates at runtime
pub struct DynamicSyscallMap {
    // Store entries as (hash, name) pairs, kept sorted by hash
    pub entries: Vec<(u32, String)>,
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

#[cfg(test)]
mod tests {
    use super::*;

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
        use crate::compute_syscall_entries_const;

        // Create a test static map
        const TEST_SYSCALLS: &[&str; 3] = &["abort", "sol_log_", "sol_panic_"];
        const TEST_ENTRIES: &[(u32, &str); 3] = &compute_syscall_entries_const(TEST_SYSCALLS);
        const TEST_MAP: SyscallMap<'static> = SyscallMap::from_entries(TEST_ENTRIES);

        // Convert to dynamic
        let dynamic = DynamicSyscallMap::from(&TEST_MAP);

        // Verify all static syscalls are present
        for &name in TEST_SYSCALLS.iter() {
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
        assert_eq!(dynamic_mut.len(), TEST_SYSCALLS.len() + 1);
    }

    #[test]
    fn test_dynamic_map_add_duplicate() {
        let mut map = DynamicSyscallMap::from_names(&["existing"]).unwrap();

        // Try to add the same syscall again
        let result = map.add("existing".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Hash conflict"));
    }

    #[test]
    fn test_dynamic_map_len_and_is_empty() {
        let empty_map = DynamicSyscallMap::from_names(&[]).unwrap();
        assert!(empty_map.is_empty());
        assert_eq!(empty_map.len(), 0);

        let map = DynamicSyscallMap::from_names(&["syscall1", "syscall2"]).unwrap();
        assert!(!map.is_empty());
        assert_eq!(map.len(), 2);
    }
}
