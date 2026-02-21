use {
    crate::error::{DebuggerError, DebuggerResult},
    serde::Deserialize,
    solana_account::Account as SolAccount,
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
    std::{collections::HashMap, fs, mem::size_of, path::Path, str::FromStr},
};

const BPF_ALIGN_OF_U128: usize = 16;
const MAX_PERMITTED_DATA_INCREASE: usize = 10240;
const NON_DUP_MARKER: u8 = 0xff;

#[derive(Deserialize)]
struct DebuggerInput {
    instruction: InstructionJson,
    accounts: Vec<AccountJson>,
}

#[derive(Deserialize)]
struct InstructionJson {
    program_id: String,
    accounts: Vec<AccountMetaJson>,
    #[serde(default)]
    data: String,
}

#[derive(Deserialize)]
struct AccountMetaJson {
    pubkey: String,
    is_signer: bool,
    is_writable: bool,
}

#[derive(Deserialize)]
struct AccountJson {
    pubkey: String,
    owner: String,
    lamports: u64,
    #[serde(default)]
    data: String,
    #[serde(default)]
    executable: bool,
}

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
        let current_len = self.buffer.len();
        let alignment_needed =
            (BPF_ALIGN_OF_U128 - (current_len % BPF_ALIGN_OF_U128)) % BPF_ALIGN_OF_U128;
        self.buffer
            .extend(std::iter::repeat_n(0u8, alignment_needed));
    }

    fn finish(self) -> Vec<u8> {
        self.buffer
    }
}

enum SerializeAccount {
    Account(Address, SolAccount, bool, bool),
    Duplicate(u8),
}

fn serialize_parameters(
    accounts: Vec<SerializeAccount>,
    instruction_data: &[u8],
    program_id: &Address,
) -> Vec<u8> {
    let mut s = Serializer::new();

    s.write::<u64>((accounts.len() as u64).to_le());

    for account in accounts {
        match account {
            SerializeAccount::Account(pubkey, acct, is_signer, is_writable) => {
                s.write::<u8>(NON_DUP_MARKER);
                s.write::<u8>(is_signer as u8);
                s.write::<u8>(is_writable as u8);
                s.write::<u8>(acct.executable as u8);
                s.write_all(&[0u8; 4]); // padding
                s.write_all(pubkey.as_ref());
                s.write_all(acct.owner.as_ref());
                s.write::<u64>(acct.lamports.to_le());
                s.write::<u64>((acct.data.len() as u64).to_le());
                s.write_account_data(&acct.data);
                s.write::<u64>(acct.rent_epoch.to_le());
            }
            SerializeAccount::Duplicate(position) => {
                s.write::<u8>(position);
                s.write_all(&[0u8; 7]); // padding
            }
        }
    }

    s.write::<u64>((instruction_data.len() as u64).to_le());
    s.write_all(instruction_data);
    s.write_all(program_id.as_ref());

    s.finish()
}

/// Parse input JSON into serialized VM input bytes and program_id.
/// Returns empty input bytes and random program_id if input is empty.
pub fn parse_input(input: &str) -> DebuggerResult<(Vec<u8>, Address)> {
    let input = input.trim();
    if input.is_empty() {
        return Ok((Vec::new(), Address::new_unique()));
    }

    // Handle both JSON file path or JSON string.
    let json_str = if Path::new(input).exists() {
        fs::read_to_string(input)?
    } else {
        input.to_string()
    };

    let debugger_input: DebuggerInput =
        serde_json::from_str(&json_str).map_err(|e| DebuggerError::InvalidInput(e.to_string()))?;

    let program_id = Address::from_str(&debugger_input.instruction.program_id)
        .map_err(|e| DebuggerError::InvalidInput(format!("Invalid program_id: {}", e)))?;

    let account_metas: Vec<AccountMeta> = debugger_input
        .instruction
        .accounts
        .iter()
        .map(|a| {
            let pubkey = Address::from_str(&a.pubkey)
                .map_err(|e| DebuggerError::InvalidInput(format!("Invalid pubkey: {}", e)))?;
            Ok(AccountMeta {
                pubkey,
                is_signer: a.is_signer,
                is_writable: a.is_writable,
            })
        })
        .collect::<DebuggerResult<Vec<_>>>()?;

    let instruction_data = if debugger_input.instruction.data.is_empty() {
        Vec::new()
    } else {
        bs58::decode(&debugger_input.instruction.data)
            .into_vec()
            .map_err(|e| {
                DebuggerError::InvalidInput(format!("Invalid base58 instruction data: {}", e))
            })?
    };

    let instruction = Instruction::new_with_bytes(program_id, &instruction_data, account_metas);

    let account_map: HashMap<Address, SolAccount> = debugger_input
        .accounts
        .iter()
        .map(|a| {
            let pubkey = Address::from_str(&a.pubkey)
                .map_err(|e| DebuggerError::InvalidInput(format!("Invalid pubkey: {}", e)))?;
            let owner = Address::from_str(&a.owner)
                .map_err(|e| DebuggerError::InvalidInput(format!("Invalid owner: {}", e)))?;
            let data = if a.data.is_empty() {
                Vec::new()
            } else {
                bs58::decode(&a.data).into_vec().map_err(|e| {
                    DebuggerError::InvalidInput(format!("Invalid base58 account data: {}", e))
                })?
            };
            Ok((
                pubkey,
                SolAccount {
                    lamports: a.lamports,
                    data,
                    owner,
                    executable: a.executable,
                    rent_epoch: 0,
                },
            ))
        })
        .collect::<DebuggerResult<HashMap<_, _>>>()?;

    let mut serialized_accounts = Vec::new();
    let mut seen: HashMap<Address, usize> = HashMap::new();

    for (i, meta) in instruction.accounts.iter().enumerate() {
        if let Some(&first_idx) = seen.get(&meta.pubkey) {
            serialized_accounts.push(SerializeAccount::Duplicate(first_idx as u8));
        } else {
            seen.insert(meta.pubkey, i);
            let acct = account_map.get(&meta.pubkey).ok_or_else(|| {
                DebuggerError::InvalidInput(format!("Missing account data for {}", meta.pubkey))
            })?;
            serialized_accounts.push(SerializeAccount::Account(
                meta.pubkey,
                acct.clone(),
                meta.is_signer,
                meta.is_writable,
            ));
        }
    }

    let bytes = serialize_parameters(
        serialized_accounts,
        &instruction.data,
        &instruction.program_id,
    );

    Ok((bytes, program_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_input() {
        let (bytes, _) = parse_input("").unwrap();
        assert!(bytes.is_empty());
    }

    #[test]
    fn test_parse_json_string() {
        let program_id = Address::new_unique();
        let account_pubkey = Address::new_unique();
        let owner = Address::new_unique();

        let json = format!(
            r#"{{
                "instruction": {{
                    "program_id": "{}",
                    "accounts": [
                        {{ "pubkey": "{}", "is_signer": true, "is_writable": true }}
                    ],
                    "data": "q"
                }},
                "accounts": [
                    {{
                        "pubkey": "{}",
                        "owner": "{}",
                        "lamports": 1000000,
                        "data": "",
                        "executable": false
                    }}
                ]
            }}"#,
            program_id, account_pubkey, account_pubkey, owner
        );

        let (bytes, pid) = parse_input(&json).unwrap();
        assert!(!bytes.is_empty());
        assert_eq!(pid, program_id);
    }
}
