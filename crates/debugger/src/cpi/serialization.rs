use {
    crate::error::{DebuggerError, DebuggerResult},
    solana_sdk::{account::Account, pubkey::Pubkey},
    std::collections::HashMap,
};

const NON_DUP_MARKER: u8 = 0xFF;
const MAX_PERMITTED_DATA_INCREASE: usize = 10 * 1024;

/// Serialize accounts and instruction for VM input
pub fn serialize_input(
    accounts: &[(Pubkey, Account, bool, bool)], // (pubkey, account, is_signer, is_writable)
    instruction_data: &[u8],
    program_id: &Pubkey,
) -> Vec<u8> {
    let mut buffer = Vec::new();
    let mut seen_accounts: HashMap<Pubkey, u8> = HashMap::new();

    // Number of accounts
    buffer.extend_from_slice(&(accounts.len() as u64).to_le_bytes());

    // Serialize each account
    for (index, (pubkey, state, is_signer, is_writable)) in accounts.iter().enumerate() {
        if let Some(first_index) = seen_accounts.get(pubkey) {
            buffer.push(*first_index);
            buffer.extend_from_slice(&[0u8; 7]);
            continue;
        }

        let index_byte = u8::try_from(index).expect("account index must fit in u8");
        seen_accounts.insert(*pubkey, index_byte);

        // Non-duplicate marker
        buffer.push(NON_DUP_MARKER);

        // Flags
        buffer.push(*is_signer as u8);
        buffer.push(*is_writable as u8);
        buffer.push(state.executable as u8);
        buffer.extend_from_slice(&[0u8; 4]);

        // Pubkey
        buffer.extend_from_slice(pubkey.as_ref());

        // Owner
        buffer.extend_from_slice(state.owner.as_ref());

        // Lamports
        buffer.extend_from_slice(&state.lamports.to_le_bytes());

        // Data length
        buffer.extend_from_slice(&(state.data.len() as u64).to_le_bytes());

        // Data
        buffer.extend_from_slice(&state.data);

        // Realloc padding
        buffer.extend_from_slice(&vec![0u8; MAX_PERMITTED_DATA_INCREASE]);

        // Alignment padding to 8 bytes
        let padding = (8 - ((state.data.len() + MAX_PERMITTED_DATA_INCREASE) % 8)) % 8;
        buffer.extend_from_slice(&vec![0u8; padding]);

        // Rent epoch
        buffer.extend_from_slice(&state.rent_epoch.to_le_bytes());
    }

    // Instruction data length
    buffer.extend_from_slice(&(instruction_data.len() as u64).to_le_bytes());

    // Instruction data
    buffer.extend_from_slice(instruction_data);

    // Program ID
    buffer.extend_from_slice(program_id.as_ref());

    buffer
}

/// Deserialize program input from VM memory
pub fn deserialize_input(
    data: &[u8],
    account_count: usize,
) -> DebuggerResult<
    (Vec<(Pubkey, u64, Vec<u8>)>, Vec<u8>, Pubkey), // (accounts, instruction_data, program_id)
> {
    let mut results = Vec::with_capacity(account_count);
    let mut offset = 8; // Skip num_accounts u64

    for _ in 0..account_count {
        if offset >= data.len() {
            return Err(DebuggerError::DeserializeError(
                "Buffer too short".to_string(),
            ));
        }

        // Check duplicate marker
        let dup_marker = data[offset];
        offset += 1;

        if dup_marker != NON_DUP_MARKER {
            // Skip duplicate
            offset += 7;
            continue;
        }

        // Skip flags
        offset += 7;

        // Pubkey
        if offset + 32 > data.len() {
            return Err(DebuggerError::DeserializeError(
                "Buffer too short for pubkey".to_string(),
            ));
        }
        let pubkey = Pubkey::from(<[u8; 32]>::try_from(&data[offset..offset + 32]).unwrap());
        offset += 32;

        // Skip owner
        offset += 32;

        // Lamports
        if offset + 8 > data.len() {
            return Err(DebuggerError::DeserializeError(
                "Buffer too short for lamports".to_string(),
            ));
        }
        let lamports = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
        offset += 8;

        // Data length
        if offset + 8 > data.len() {
            return Err(DebuggerError::DeserializeError(
                "Buffer too short for data_len".to_string(),
            ));
        }
        let data_len = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap()) as usize;
        offset += 8;

        // Data
        if offset + data_len > data.len() {
            return Err(DebuggerError::DeserializeError(
                "Buffer too short for data".to_string(),
            ));
        }
        let account_data = data[offset..offset + data_len].to_vec();
        offset += data_len;

        // Skip realloc padding + alignment padding
        offset += MAX_PERMITTED_DATA_INCREASE;
        let padding = (8 - ((data_len + MAX_PERMITTED_DATA_INCREASE) % 8)) % 8;
        offset += padding;

        // Skip rent_epoch
        offset += 8;

        results.push((pubkey, lamports, account_data));
    }

    // Instruction data length
    if offset + 8 > data.len() {
        return Err(DebuggerError::DeserializeError(
            "Buffer too short for instruction data len".to_string(),
        ));
    }
    let instruction_len = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap()) as usize;
    offset += 8;

    // Instruction data
    if offset + instruction_len > data.len() {
        return Err(DebuggerError::DeserializeError(
            "Buffer too short for instruction data".to_string(),
        ));
    }
    let instruction_data = data[offset..offset + instruction_len].to_vec();
    offset += instruction_len;

    // Program ID
    if offset + 32 > data.len() {
        return Err(DebuggerError::DeserializeError(
            "Buffer too short for program id".to_string(),
        ));
    }
    let program_id = Pubkey::from(<[u8; 32]>::try_from(&data[offset..offset + 32]).unwrap());

    Ok((results, instruction_data, program_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize_e2e() {
        let pubkey = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let program_id = Pubkey::new_unique();

        let account = Account {
            lamports: 1_000_000,
            data: vec![1, 2, 3, 4],
            owner,
            executable: false,
            rent_epoch: 0,
        };

        let accounts = vec![(pubkey, account.clone(), true, true)];
        let instruction_data = vec![5, 6, 7];

        let serialized = serialize_input(&accounts, &instruction_data, &program_id);
        let (deserialized, decoded_instruction, decoded_program_id) =
            deserialize_input(&serialized, 1).unwrap();

        assert_eq!(deserialized.len(), 1);
        assert_eq!(deserialized[0].0, pubkey);
        assert_eq!(deserialized[0].1, account.lamports);
        assert_eq!(deserialized[0].2, account.data);
        assert_eq!(decoded_instruction, instruction_data);
        assert_eq!(decoded_program_id, program_id);
    }
}
