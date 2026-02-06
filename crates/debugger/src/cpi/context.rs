use {
    crate::execution_cost::ExecutionCost,
    sbpf_vm::compute::ComputeMeter,
    solana_sdk::{
        account::Account, clock::Clock, epoch_schedule::EpochSchedule, pubkey::Pubkey, rent::Rent,
    },
    std::{cell::RefCell, collections::HashMap, fs, path::Path, rc::Rc},
};

pub const MAX_CPI_DEPTH: usize = 4;

#[derive(Debug, Default)]
pub struct AccountStore {
    accounts: HashMap<Pubkey, Account>,
    // Tracks the original data length per account
    original_data_lens: HashMap<Pubkey, usize>,
}

impl AccountStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, pubkey: &Pubkey) -> Option<&Account> {
        self.accounts.get(pubkey)
    }

    pub fn get_mut(&mut self, pubkey: &Pubkey) -> Option<&mut Account> {
        self.accounts.get_mut(pubkey)
    }

    pub fn insert(&mut self, pubkey: Pubkey, account: Account) {
        let data_len = account.data.len();
        self.accounts.insert(pubkey, account);
        self.original_data_lens.entry(pubkey).or_insert(data_len);
    }

    pub fn get_original_data_len(&self, pubkey: &Pubkey) -> usize {
        self.original_data_lens.get(pubkey).copied().unwrap_or(0)
    }

    pub fn update_lamports(&mut self, pubkey: &Pubkey, lamports: u64) -> bool {
        if let Some(state) = self.accounts.get_mut(pubkey) {
            state.lamports = lamports;
            true
        } else {
            false
        }
    }

    pub fn update_data(&mut self, pubkey: &Pubkey, data: Vec<u8>) -> bool {
        if let Some(state) = self.accounts.get_mut(pubkey) {
            state.data = data;
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Default)]
pub struct ProgramRegistry {
    programs: HashMap<Pubkey, Vec<u8>>,
}

impl ProgramRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, program_id: Pubkey, elf_bytes: Vec<u8>) {
        self.programs.insert(program_id, elf_bytes);
    }

    pub fn load_from_file(&mut self, program_id: Pubkey, path: &Path) -> std::io::Result<()> {
        let elf_bytes = fs::read(path)?;
        self.register(program_id, elf_bytes);
        Ok(())
    }

    pub fn get(&self, program_id: &Pubkey) -> Option<&[u8]> {
        self.programs.get(program_id).map(|v| v.as_slice())
    }

    pub fn contains(&self, program_id: &Pubkey) -> bool {
        self.programs.contains_key(program_id)
    }

    pub fn list(&self) -> Vec<Pubkey> {
        self.programs.keys().copied().collect()
    }
}

#[derive(Debug, Clone)]
pub struct CpiContext {
    inner: Rc<RefCell<CpiContextInner>>,
}

#[derive(Debug)]
pub struct CpiContextInner {
    pub stack_height: usize,
    pub program_registry: ProgramRegistry,
    pub accounts: AccountStore,
    pub return_data: Option<(Pubkey, Vec<u8>)>,
    pub costs: ExecutionCost,
    pub compute_meter: ComputeMeter,
    pub clock: Clock,
    pub rent: Rent,
    pub epoch_schedule: EpochSchedule,
}

impl CpiContextInner {
    pub fn new(compute_meter: ComputeMeter) -> Self {
        Self {
            stack_height: 1,
            program_registry: ProgramRegistry::new(),
            accounts: AccountStore::new(),
            return_data: None,
            costs: ExecutionCost::default(),
            compute_meter,
            clock: Clock::default(),
            rent: Rent::default(),
            epoch_schedule: EpochSchedule::default(),
        }
    }
}

impl CpiContext {
    pub fn new(compute_meter: ComputeMeter) -> Self {
        Self {
            inner: Rc::new(RefCell::new(CpiContextInner::new(compute_meter))),
        }
    }

    pub fn borrow(&self) -> std::cell::Ref<'_, CpiContextInner> {
        self.inner.borrow()
    }

    pub fn borrow_mut(&self) -> std::cell::RefMut<'_, CpiContextInner> {
        self.inner.borrow_mut()
    }

    pub fn can_invoke(&self) -> bool {
        self.borrow().stack_height < MAX_CPI_DEPTH
    }

    pub fn push(&self) -> bool {
        let mut ctx = self.borrow_mut();
        if ctx.stack_height < MAX_CPI_DEPTH {
            ctx.stack_height += 1;
            true
        } else {
            false
        }
    }

    pub fn pop(&self) {
        let mut ctx = self.borrow_mut();
        if ctx.stack_height > 1 {
            ctx.stack_height -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_store_insert_and_get() {
        let mut store = AccountStore::new();
        let pubkey = Pubkey::new_unique();
        let account = Account {
            lamports: 1_000_000,
            data: vec![1, 2, 3],
            owner: Pubkey::new_unique(),
            executable: false,
            rent_epoch: 0,
        };

        store.insert(pubkey, account.clone());
        let retrieved = store.get(&pubkey).unwrap();
        assert_eq!(retrieved.lamports, 1_000_000);
        assert_eq!(retrieved.data, vec![1, 2, 3]);
    }

    #[test]
    fn test_account_store_update_lamports() {
        let mut store = AccountStore::new();
        let pubkey = Pubkey::new_unique();
        store.insert(
            pubkey,
            Account {
                lamports: 1000,
                data: vec![],
                owner: Pubkey::default(),
                executable: false,
                rent_epoch: 0,
            },
        );

        assert!(store.update_lamports(&pubkey, 2000));
        assert_eq!(store.get(&pubkey).unwrap().lamports, 2000);

        // Update non-existent account.
        let other = Pubkey::new_unique();
        assert!(!store.update_lamports(&other, 500));
    }

    #[test]
    fn test_account_store_update_data() {
        let mut store = AccountStore::new();
        let pubkey = Pubkey::new_unique();
        store.insert(
            pubkey,
            Account {
                lamports: 0,
                data: vec![1, 2],
                owner: Pubkey::default(),
                executable: false,
                rent_epoch: 0,
            },
        );

        assert!(store.update_data(&pubkey, vec![3, 4, 5]));
        assert_eq!(store.get(&pubkey).unwrap().data, vec![3, 4, 5]);
    }

    #[test]
    fn test_program_registry_register_and_get() {
        let mut registry = ProgramRegistry::new();
        let program_id = Pubkey::new_unique();
        let elf_bytes = vec![0x7f, 0x45, 0x4c, 0x46];

        registry.register(program_id, elf_bytes.clone());

        assert!(registry.contains(&program_id));
        assert_eq!(registry.get(&program_id).unwrap(), elf_bytes.as_slice());
        assert_eq!(registry.list().len(), 1);
    }

    #[test]
    fn test_program_registry_not_found() {
        let registry = ProgramRegistry::new();
        let program_id = Pubkey::new_unique();

        assert!(!registry.contains(&program_id));
        assert!(registry.get(&program_id).is_none());
    }

    #[test]
    fn test_return_data_storage() {
        let mut ctx = CpiContextInner::new(ComputeMeter::new(10_000));
        let program_id = Pubkey::new_unique();
        let data = vec![1, 2, 3, 4, 5];

        ctx.return_data = Some((program_id, data.clone()));

        let (stored_id, stored_data) = ctx.return_data.unwrap();
        assert_eq!(stored_id, program_id);
        assert_eq!(stored_data, data);
    }
}
