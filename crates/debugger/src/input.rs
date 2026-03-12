use {
    crate::error::{DebuggerError, DebuggerResult},
    serde::Deserialize,
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
    std::{fs, path::Path, str::FromStr},
};

#[derive(Deserialize)]
struct DebuggerInput {
    instruction: InstructionJson,
    accounts: Vec<AccountJson>,
    #[serde(default)]
    programs: Vec<ProgramJson>,
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

#[derive(Deserialize)]
struct ProgramJson {
    program_id: String,
    elf: String,
}

pub struct ParsedInput {
    pub instruction: Instruction,
    pub accounts: Vec<(Address, Account)>,
    pub programs: Vec<(Address, Vec<u8>)>,
}

pub fn parse_input(input: &str) -> DebuggerResult<ParsedInput> {
    let input = input.trim();
    if input.is_empty() {
        let program_id = Address::new_unique();
        return Ok(ParsedInput {
            instruction: Instruction::new_with_bytes(program_id, &[], vec![]),
            accounts: Vec::new(),
            programs: Vec::new(),
        });
    }

    // Handle both JSON file path or JSON string.
    let input_path = Path::new(input);
    let (json_str, base_dir) = if input_path.exists() {
        let base = input_path.parent().unwrap_or(Path::new(".")).to_path_buf();
        (fs::read_to_string(input)?, base)
    } else {
        (input.to_string(), Path::new(".").to_path_buf())
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

    let accounts: Vec<(Address, Account)> = debugger_input
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
                Account {
                    lamports: a.lamports,
                    data,
                    owner,
                    executable: a.executable,
                    rent_epoch: 0,
                },
            ))
        })
        .collect::<DebuggerResult<Vec<_>>>()?;

    let programs: Vec<(Address, Vec<u8>)> = debugger_input
        .programs
        .iter()
        .map(|p| {
            let program_id = Address::from_str(&p.program_id)
                .map_err(|e| DebuggerError::InvalidInput(format!("Invalid program_id: {}", e)))?;
            let elf_path = base_dir.join(&p.elf);
            let elf_bytes = fs::read(&elf_path).map_err(|e| {
                DebuggerError::InvalidInput(format!(
                    "Failed to read ELF at {}: {}",
                    elf_path.display(),
                    e
                ))
            })?;
            Ok((program_id, elf_bytes))
        })
        .collect::<DebuggerResult<Vec<_>>>()?;

    Ok(ParsedInput {
        instruction,
        accounts,
        programs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_input() {
        let parsed = parse_input("").unwrap();
        assert!(parsed.accounts.is_empty());
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

        let parsed = parse_input(&json).unwrap();
        assert_eq!(parsed.instruction.program_id, program_id);
        assert_eq!(parsed.accounts.len(), 1);
        assert_eq!(parsed.accounts[0].0, account_pubkey);
    }
}
