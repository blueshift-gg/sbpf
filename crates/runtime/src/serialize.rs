use {
    crate::{
        cpi::validate::check_account_change,
        errors::{RuntimeError, RuntimeResult},
    },
    solana_account::Account,
    solana_address::Address,
    solana_instruction::AccountMeta,
    std::{collections::HashMap, mem::size_of},
};

const ALIGN_OF_U64: usize = 8;
const MAX_PERMITTED_DATA_INCREASE: usize = 10240;
const NON_DUP_MARKER: u8 = 0xff;

struct Serializer {
    buffer: Vec<u8>,
}

impl Serializer {
    fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    fn write<T>(&mut self, value: T) {
        let bytes =
            unsafe { std::slice::from_raw_parts(&value as *const T as *const u8, size_of::<T>()) };
        self.buffer.extend_from_slice(bytes);
    }

    fn write_all(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    fn write_account_data(&mut self, data: &[u8]) {
        self.write_all(data);
        self.buffer
            .extend(std::iter::repeat_n(0u8, MAX_PERMITTED_DATA_INCREASE));
        let alignment_needed = (ALIGN_OF_U64 - (self.buffer.len() % ALIGN_OF_U64)) % ALIGN_OF_U64;
        self.buffer
            .extend(std::iter::repeat_n(0u8, alignment_needed));
    }

    fn finish(self) -> Vec<u8> {
        self.buffer
    }
}

enum SerializeAccount {
    Account(Address, Account, bool, bool),
    Duplicate(u8),
}

pub fn serialize_parameters(
    accounts: &HashMap<Address, Account>,
    account_metas: &[AccountMeta],
    instruction_data: &[u8],
    program_id: &Address,
) -> RuntimeResult<(Vec<u8>, Vec<usize>, usize)> {
    let mut seen: HashMap<Address, usize> = HashMap::new();
    let mut serialize_accounts = Vec::with_capacity(account_metas.len());

    for (i, meta) in account_metas.iter().enumerate() {
        if let Some(&first_idx) = seen.get(&meta.pubkey) {
            serialize_accounts.push(SerializeAccount::Duplicate(first_idx as u8));
        } else {
            seen.insert(meta.pubkey, i);
            let account = accounts.get(&meta.pubkey).ok_or_else(|| {
                RuntimeError::MissingAccount(format!("Missing account data for {}", meta.pubkey))
            })?;
            serialize_accounts.push(SerializeAccount::Account(
                meta.pubkey,
                account.clone(),
                meta.is_signer,
                meta.is_writable,
            ));
        }
    }

    let mut s = Serializer::new();
    let mut pre_lens: Vec<usize> = Vec::new();
    s.write::<u64>((serialize_accounts.len() as u64).to_le());

    for account in serialize_accounts {
        match account {
            SerializeAccount::Account(pubkey, acct, is_signer, is_writable) => {
                s.write::<u8>(NON_DUP_MARKER);
                s.write::<u8>(is_signer as u8);
                s.write::<u8>(is_writable as u8);
                s.write::<u8>(acct.executable as u8);
                s.write_all(&[0u8; 4]);
                s.write_all(pubkey.as_ref());
                s.write_all(acct.owner.as_ref());
                s.write::<u64>(acct.lamports.to_le());
                pre_lens.push(acct.data.len());
                s.write::<u64>((acct.data.len() as u64).to_le());
                s.write_account_data(&acct.data);
                s.write::<u64>(acct.rent_epoch.to_le());
            }
            SerializeAccount::Duplicate(position) => {
                s.write::<u8>(position);
                s.write_all(&[0u8; 7]);
            }
        }
    }

    s.write::<u64>((instruction_data.len() as u64).to_le());
    let instruction_data_offset = s.buffer.len();
    s.write_all(instruction_data);
    s.write_all(program_id.as_ref());

    Ok((s.finish(), pre_lens, instruction_data_offset))
}

struct Deserializer<'a> {
    buffer: &'a [u8],
    offset: usize,
}

impl<'a> Deserializer<'a> {
    fn new(buffer: &'a [u8]) -> Self {
        Self { buffer, offset: 0 }
    }

    fn read_u64(&mut self) -> u64 {
        let bytes: [u8; 8] = self.buffer[self.offset..self.offset + 8]
            .try_into()
            .unwrap_or([0u8; 8]);
        self.offset += 8;
        u64::from_le_bytes(bytes)
    }

    fn skip(&mut self, n: usize) {
        self.offset += n;
    }

    fn read_address(&mut self) -> Address {
        let bytes: [u8; 32] = self.buffer[self.offset..self.offset + 32]
            .try_into()
            .unwrap_or([0u8; 32]);
        self.offset += 32;
        Address::from(bytes)
    }

    fn read_account_data(&mut self, post_len: usize, pre_len: usize) -> &'a [u8] {
        let data = &self.buffer[self.offset..self.offset + post_len];
        // Skip using original length to stay aligned with the serialized layout.
        self.offset += pre_len;
        self.offset += MAX_PERMITTED_DATA_INCREASE;
        let align = (ALIGN_OF_U64 - (self.offset % ALIGN_OF_U64)) % ALIGN_OF_U64;
        self.offset += align;
        data
    }
}

pub fn deserialize_parameters(
    accounts: &mut HashMap<Address, Account>,
    account_metas: &[AccountMeta],
    input: &[u8],
    pre_lens: &[usize],
    callee_program_id: &Address,
) -> RuntimeResult<()> {
    let mut d = Deserializer::new(input);
    let _num_accounts = d.read_u64();
    let mut seen: HashMap<Address, usize> = HashMap::new();
    let mut pre_len_idx = 0;

    for (i, meta) in account_metas.iter().enumerate() {
        if seen.contains_key(&meta.pubkey) {
            d.skip(8); // duplicate: index(1) + padding(7)
            continue;
        }
        seen.insert(meta.pubkey, i);

        let pre_len = pre_lens.get(pre_len_idx).copied().unwrap_or(0);
        pre_len_idx += 1;

        d.skip(8); // dup_marker + is_signer + is_writable + executable + padding
        d.skip(32); // pubkey
        let owner = d.read_address();
        let lamports = d.read_u64();
        let data_len = d.read_u64() as usize;
        let data = d.read_account_data(data_len, pre_len);
        let _rent_epoch = d.read_u64();

        if meta.is_writable
            && let Some(account) = accounts.get_mut(&meta.pubkey)
        {
            check_account_change(
                callee_program_id,
                &meta.pubkey,
                account,
                &owner,
                lamports,
                data,
            )?;
            account.lamports = lamports;
            account.data = data.to_vec();
            account.owner = owner;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROGRAM: Address = Address::new_from_array([1u8; 32]);
    const OTHER: Address = Address::new_from_array([2u8; 32]);
    const ACCT_1: Address = Address::new_from_array([10u8; 32]);
    const ACCT_2: Address = Address::new_from_array([11u8; 32]);

    fn make_account(owner: Address, lamports: u64, data: &[u8]) -> Account {
        Account {
            lamports,
            data: data.to_vec(),
            owner,
            executable: false,
            rent_epoch: 0,
        }
    }

    fn accounts_map(list: &[(Address, Account)]) -> HashMap<Address, Account> {
        list.iter().cloned().collect()
    }

    #[test]
    fn e2e_accounts_preserved() {
        let mut accounts = accounts_map(&[
            (ACCT_1, make_account(PROGRAM, 100, b"hello")),
            (ACCT_2, make_account(PROGRAM, 200, b"world")),
        ]);
        let metas = vec![
            AccountMeta {
                pubkey: ACCT_1,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: ACCT_2,
                is_signer: false,
                is_writable: true,
            },
        ];
        let (input, pre_lens, _) =
            serialize_parameters(&accounts, &metas, b"ix", &PROGRAM).unwrap();
        deserialize_parameters(&mut accounts, &metas, &input, &pre_lens, &PROGRAM).unwrap();

        assert_eq!(accounts[&ACCT_1].lamports, 100);
        assert_eq!(accounts[&ACCT_1].data, b"hello");
        assert_eq!(accounts[&ACCT_2].lamports, 200);
        assert_eq!(accounts[&ACCT_2].data, b"world");
    }

    #[test]
    fn read_only_account_not_updated() {
        let mut accounts = accounts_map(&[(ACCT_1, make_account(PROGRAM, 100, b"data"))]);
        let metas = vec![AccountMeta {
            pubkey: ACCT_1,
            is_signer: false,
            is_writable: false,
        }];
        let (mut input, pre_lens, _) =
            serialize_parameters(&accounts, &metas, b"", &PROGRAM).unwrap();

        // Try to update lamports.
        let lamport_offset = 8 + 8 + 32 + 32;
        input[lamport_offset..lamport_offset + 8].copy_from_slice(&999u64.to_le_bytes());

        deserialize_parameters(&mut accounts, &metas, &input, &pre_lens, &PROGRAM).unwrap();
        assert_eq!(accounts[&ACCT_1].lamports, 100); // lamports unchanged
    }

    #[test]
    fn non_owner_data_change_rejected() {
        let mut accounts = accounts_map(&[(ACCT_1, make_account(OTHER, 100, b"original"))]);
        let metas = vec![AccountMeta {
            pubkey: ACCT_1,
            is_signer: false,
            is_writable: true,
        }];
        let (mut input, pre_lens, _) =
            serialize_parameters(&accounts, &metas, b"", &PROGRAM).unwrap();

        // Try modifying data.
        let data_offset = 8 + 8 + 32 + 32 + 8 + 8;
        input[data_offset..data_offset + 4].copy_from_slice(b"fake");

        let result = deserialize_parameters(&mut accounts, &metas, &input, &pre_lens, &PROGRAM);
        assert!(result.is_err());
        assert_eq!(accounts[&ACCT_1].data, b"original");
    }

    #[test]
    fn non_owner_lamport_debit_rejected() {
        let mut accounts = accounts_map(&[(ACCT_1, make_account(OTHER, 100, b""))]);
        let metas = vec![AccountMeta {
            pubkey: ACCT_1,
            is_signer: false,
            is_writable: true,
        }];
        let (mut input, pre_lens, _) =
            serialize_parameters(&accounts, &metas, b"", &PROGRAM).unwrap();

        let lamport_offset = 8 + 8 + 32 + 32;
        input[lamport_offset..lamport_offset + 8].copy_from_slice(&50u64.to_le_bytes());

        let result = deserialize_parameters(&mut accounts, &metas, &input, &pre_lens, &PROGRAM);
        assert!(result.is_err());
        assert_eq!(accounts[&ACCT_1].lamports, 100);
    }

    #[test]
    fn missing_account_errors() {
        let accounts = accounts_map(&[]);
        let metas = vec![AccountMeta {
            pubkey: ACCT_1,
            is_signer: false,
            is_writable: false,
        }];
        let result = serialize_parameters(&accounts, &metas, b"", &PROGRAM);
        assert!(result.is_err());
    }

    #[test]
    fn check_instruction_data_offset() {
        let accounts = accounts_map(&[(ACCT_1, make_account(PROGRAM, 100, b"data"))]);
        let metas = vec![AccountMeta {
            pubkey: ACCT_1,
            is_signer: false,
            is_writable: true,
        }];
        let ix_data = b"instruction";
        let (input, _, offset) =
            serialize_parameters(&accounts, &metas, ix_data, &PROGRAM).unwrap();

        // Check offset points to instruction data
        assert_eq!(&input[offset..(offset + ix_data.len())], ix_data);
    }

    #[test]
    fn check_instruction_data_offset_no_data() {
        let accounts = accounts_map(&[(ACCT_1, make_account(PROGRAM, 100, b"data"))]);
        let metas = vec![AccountMeta {
            pubkey: ACCT_1,
            is_signer: false,
            is_writable: true,
        }];
        let ix_data = b""; // no data
        let (input, _, offset) =
            serialize_parameters(&accounts, &metas, ix_data, &PROGRAM).unwrap();

        // Check offset points to program ID (since data is empty)
        assert_eq!(&input[offset..offset + 32], PROGRAM.as_ref());
    }
}
