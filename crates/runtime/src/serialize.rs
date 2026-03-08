use {
    crate::errors::{RuntimeError, RuntimeResult},
    solana_account::Account,
    solana_address::Address,
    solana_instruction::AccountMeta,
    std::{collections::HashMap, mem::size_of},
};

const BPF_ALIGN_OF_U128: usize = 16;
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
        let alignment_needed =
            (BPF_ALIGN_OF_U128 - (self.buffer.len() % BPF_ALIGN_OF_U128)) % BPF_ALIGN_OF_U128;
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
) -> RuntimeResult<Vec<u8>> {
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
    s.write_all(instruction_data);
    s.write_all(program_id.as_ref());

    Ok(s.finish())
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

    fn read_account_data(&mut self, data_len: usize) -> &'a [u8] {
        let data = &self.buffer[self.offset..self.offset + data_len];
        self.offset += data_len;
        self.offset += MAX_PERMITTED_DATA_INCREASE;
        let align = (BPF_ALIGN_OF_U128 - (self.offset % BPF_ALIGN_OF_U128)) % BPF_ALIGN_OF_U128;
        self.offset += align;
        data
    }
}

pub fn deserialize_parameters(
    accounts: &mut HashMap<Address, Account>,
    account_metas: &[AccountMeta],
    input: &[u8],
) {
    let mut d = Deserializer::new(input);
    let _num_accounts = d.read_u64();
    let mut seen: HashMap<Address, usize> = HashMap::new();

    for (i, meta) in account_metas.iter().enumerate() {
        if seen.contains_key(&meta.pubkey) {
            d.skip(8); // duplicate: index(1) + padding(7)
            continue;
        }
        seen.insert(meta.pubkey, i);

        d.skip(8); // dup_marker + is_signer + is_writable + executable + padding
        d.skip(64); // pubkey + owner
        let lamports = d.read_u64();
        let data_len = d.read_u64() as usize;
        let data = d.read_account_data(data_len);
        let _rent_epoch = d.read_u64();

        if meta.is_writable
            && let Some(account) = accounts.get_mut(&meta.pubkey)
        {
            account.lamports = lamports;
            account.data = data.to_vec();
        }
    }
}
