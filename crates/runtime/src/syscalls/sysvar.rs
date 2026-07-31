use {
    crate::config::{ExecutionCost, SysvarContext},
    sbpf_vm::{
        compute::ComputeMeter,
        errors::{SbpfVmError, SbpfVmResult},
        memory::Memory,
    },
    solana_address::Address,
    solana_clock::Clock,
    solana_epoch_rewards::EpochRewards,
    solana_epoch_schedule::EpochSchedule,
    solana_last_restart_slot::LastRestartSlot,
    solana_rent::Rent,
    std::mem::size_of,
};

fn write_sysvar_bytes<T>(memory: &mut Memory, addr: u64, sysvar: &T) -> SbpfVmResult<()> {
    let bytes =
        unsafe { std::slice::from_raw_parts(sysvar as *const T as *const u8, size_of::<T>()) };
    memory.write_bytes(addr, bytes)
}

pub fn sol_get_clock_sysvar(
    registers: [u64; 5],
    memory: &mut Memory,
    compute: &ComputeMeter,
    costs: &ExecutionCost,
    sysvars: &SysvarContext,
) -> SbpfVmResult<u64> {
    compute.consume(
        costs
            .sysvar_base_cost
            .saturating_add(size_of::<Clock>() as u64),
    )?;
    write_sysvar_bytes(memory, registers[0], &sysvars.clock)?;
    Ok(0)
}

pub fn sol_get_rent_sysvar(
    registers: [u64; 5],
    memory: &mut Memory,
    compute: &ComputeMeter,
    costs: &ExecutionCost,
    sysvars: &SysvarContext,
) -> SbpfVmResult<u64> {
    compute.consume(
        costs
            .sysvar_base_cost
            .saturating_add(size_of::<Rent>() as u64),
    )?;
    write_sysvar_bytes(memory, registers[0], &sysvars.rent)?;
    Ok(0)
}

pub fn sol_get_epoch_schedule_sysvar(
    registers: [u64; 5],
    memory: &mut Memory,
    compute: &ComputeMeter,
    costs: &ExecutionCost,
    sysvars: &SysvarContext,
) -> SbpfVmResult<u64> {
    compute.consume(
        costs
            .sysvar_base_cost
            .saturating_add(size_of::<EpochSchedule>() as u64),
    )?;
    write_sysvar_bytes(memory, registers[0], &sysvars.epoch_schedule)?;
    Ok(0)
}

pub fn sol_get_last_restart_slot(
    registers: [u64; 5],
    memory: &mut Memory,
    compute: &ComputeMeter,
    costs: &ExecutionCost,
    sysvars: &SysvarContext,
) -> SbpfVmResult<u64> {
    compute.consume(
        costs
            .sysvar_base_cost
            .saturating_add(size_of::<LastRestartSlot>() as u64),
    )?;
    write_sysvar_bytes(memory, registers[0], &sysvars.last_restart_slot)?;
    Ok(0)
}

pub fn sol_get_epoch_rewards_sysvar(
    registers: [u64; 5],
    memory: &mut Memory,
    compute: &ComputeMeter,
    costs: &ExecutionCost,
    sysvars: &SysvarContext,
) -> SbpfVmResult<u64> {
    compute.consume(
        costs
            .sysvar_base_cost
            .saturating_add(size_of::<EpochRewards>() as u64),
    )?;
    write_sysvar_bytes(memory, registers[0], &sysvars.epoch_rewards)?;
    Ok(0)
}

pub fn sol_get_sysvar(
    registers: [u64; 5],
    memory: &mut Memory,
    compute: &ComputeMeter,
    costs: &ExecutionCost,
    sysvars: &SysvarContext,
) -> SbpfVmResult<u64> {
    let sysvar_id_addr = registers[0];
    let var_addr = registers[1];
    let offset = registers[2];
    let length = registers[3];

    let sysvar_id_cost = 32_u64.checked_div(costs.cpi_bytes_per_unit).unwrap_or(0);
    let sysvar_buf_cost = length.checked_div(costs.cpi_bytes_per_unit).unwrap_or(0);
    let cost = costs
        .sysvar_base_cost
        .saturating_add(sysvar_id_cost)
        .saturating_add(sysvar_buf_cost.max(costs.mem_op_base_cost));
    compute.consume(cost)?;

    // Check if memory region is writable
    let length_usize = usize::try_from(length)
        .map_err(|_| SbpfVmError::SyscallError("sol_get_sysvar: length exceeds usize".into()))?;
    memory.check_writable(var_addr, length_usize)?;

    // Read sysvar ID
    let id_bytes = memory.read_bytes(sysvar_id_addr, 32)?;
    let mut id_arr = [0u8; 32];
    id_arr.copy_from_slice(id_bytes);
    let sysvar_id = Address::new_from_array(id_arr);

    // Check for overflows
    let offset_length = offset.checked_add(length).ok_or_else(|| {
        SbpfVmError::SyscallError("sol_get_sysvar: offset + length overflow".into())
    })?;
    let _ = var_addr.checked_add(length).ok_or_else(|| {
        SbpfVmError::SyscallError("sol_get_sysvar: var_addr + length overflow".into())
    })?;

    let serialized = if sysvar_id == solana_clock::sysvar::ID {
        wincode::serialize(&sysvars.clock)
    } else if sysvar_id == solana_rent::sysvar::ID {
        wincode::serialize(&sysvars.rent)
    } else if sysvar_id == solana_epoch_schedule::sysvar::ID {
        wincode::serialize(&sysvars.epoch_schedule)
    } else if sysvar_id == solana_last_restart_slot::sysvar::ID {
        wincode::serialize(&sysvars.last_restart_slot)
    } else if sysvar_id == solana_epoch_rewards::sysvar::ID {
        wincode::serialize(&sysvars.epoch_rewards)
    } else if sysvar_id == solana_slot_hashes::sysvar::ID {
        wincode::serialize(&sysvars.slot_hashes)
    } else if sysvar_id == solana_stake_history::sysvar::ID {
        wincode::serialize(&sysvars.stake_history)
    } else {
        // sysvar not found
        return Ok(2);
    };
    let buf = serialized.map_err(|error| {
        SbpfVmError::SyscallError(format!("failed to serialize sysvar: {error}"))
    })?;

    if let Some(slice) = buf.get(offset as usize..offset_length as usize) {
        memory.write_bytes(var_addr, slice)?;
        Ok(0)
    } else {
        // offset length exceeds sysvar
        Ok(1)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            config::SysvarContext,
            syscalls::tests::test_helpers::{costs, make_memory, meter},
        },
        sbpf_vm::{errors::SbpfVmError, memory::Memory},
    };

    fn raw_bytes<T>(val: &T) -> Vec<u8> {
        unsafe { std::slice::from_raw_parts(val as *const T as *const u8, size_of::<T>()).to_vec() }
    }

    #[test]
    fn test_sol_get_clock_sysvar() {
        let mut memory = make_memory();
        let mut sysvars = SysvarContext::default();
        sysvars.clock.slot = 99_999;
        sysvars.clock.unix_timestamp = 1_700_000_000;

        let addr = Memory::HEAP_START;
        let registers = [addr, 0, 0, 0, 0];
        sol_get_clock_sysvar(
            registers,
            &mut memory,
            &meter(1_000_000),
            &costs(),
            &sysvars,
        )
        .unwrap();

        let written = memory.read_bytes(addr, size_of::<Clock>()).unwrap();
        assert_eq!(written, raw_bytes(&sysvars.clock).as_slice());
    }

    #[test]
    fn test_sol_get_clock_sysvar_compute_exhausted() {
        let mut memory = make_memory();
        let sysvars = SysvarContext::default();
        let cost = costs().sysvar_base_cost + size_of::<Clock>() as u64;
        let registers = [Memory::HEAP_START, 0, 0, 0, 0];
        assert!(matches!(
            sol_get_clock_sysvar(registers, &mut memory, &meter(cost - 1), &costs(), &sysvars),
            Err(SbpfVmError::ComputeBudgetExceeded { .. })
        ));
    }

    #[test]
    fn test_sol_get_rent_sysvar() {
        let mut memory = make_memory();
        let sysvars = SysvarContext {
            rent: Rent::with_lamports_per_byte(3_480),
            ..Default::default()
        };

        let addr = Memory::HEAP_START;
        let registers = [addr, 0, 0, 0, 0];
        sol_get_rent_sysvar(
            registers,
            &mut memory,
            &meter(1_000_000),
            &costs(),
            &sysvars,
        )
        .unwrap();

        let written = memory.read_bytes(addr, size_of::<Rent>()).unwrap();
        assert_eq!(written, raw_bytes(&sysvars.rent).as_slice());
    }

    #[test]
    fn test_sol_get_epoch_schedule_sysvar() {
        let mut memory = make_memory();
        let mut sysvars = SysvarContext::default();
        sysvars.epoch_schedule.slots_per_epoch = 432_000;

        let addr = Memory::HEAP_START;
        let registers = [addr, 0, 0, 0, 0];
        sol_get_epoch_schedule_sysvar(
            registers,
            &mut memory,
            &meter(1_000_000),
            &costs(),
            &sysvars,
        )
        .unwrap();

        let written = memory.read_bytes(addr, size_of::<EpochSchedule>()).unwrap();
        assert_eq!(written, raw_bytes(&sysvars.epoch_schedule).as_slice());
    }

    #[test]
    fn test_sol_get_last_restart_slot() {
        let mut memory = make_memory();
        let mut sysvars = SysvarContext::default();
        sysvars.last_restart_slot.last_restart_slot = 12_345_678;

        let addr = Memory::HEAP_START;
        let registers = [addr, 0, 0, 0, 0];
        sol_get_last_restart_slot(
            registers,
            &mut memory,
            &meter(1_000_000),
            &costs(),
            &sysvars,
        )
        .unwrap();

        let written = memory
            .read_bytes(addr, size_of::<LastRestartSlot>())
            .unwrap();
        assert_eq!(written, raw_bytes(&sysvars.last_restart_slot).as_slice());
    }

    #[test]
    fn test_sol_get_epoch_rewards_sysvar() {
        let mut memory = make_memory();
        let mut sysvars = SysvarContext::default();
        sysvars.epoch_rewards.total_rewards = 111_000;
        sysvars.epoch_rewards.active = true;

        let addr = Memory::HEAP_START;
        let registers = [addr, 0, 0, 0, 0];
        sol_get_epoch_rewards_sysvar(
            registers,
            &mut memory,
            &meter(1_000_000),
            &costs(),
            &sysvars,
        )
        .unwrap();

        let written = memory.read_bytes(addr, size_of::<EpochRewards>()).unwrap();
        assert_eq!(written, raw_bytes(&sysvars.epoch_rewards).as_slice());
    }

    #[test]
    fn test_sol_get_sysvar() {
        let mut memory = make_memory();
        let sysvars = SysvarContext::default();
        let expected = wincode::serialize(&sysvars.rent).unwrap();
        assert_eq!(expected.len(), 17);

        let id_addr = Memory::HEAP_START;
        let var_addr = Memory::HEAP_START + 64;

        // Get the rent sysvar
        memory
            .write_bytes(id_addr, solana_rent::sysvar::ID.as_ref())
            .unwrap();

        let registers = [id_addr, var_addr, 0, expected.len() as u64, 0];
        let compute = meter(1_000_000);
        let ret = sol_get_sysvar(registers, &mut memory, &compute, &costs(), &sysvars).unwrap();
        assert_eq!(ret, 0);
        assert_eq!(
            memory.read_bytes(var_addr, expected.len()).unwrap(),
            expected.as_slice()
        );
        // cost: sysvar_base_cost(100) + 32/250(0) + max(17/250(0), mem_op_base_cost(10)) = 110
        assert_eq!(compute.get_consumed(), 110);
    }

    #[test]
    fn test_sol_get_sysvar_not_found() {
        let mut memory = make_memory();
        let sysvars = SysvarContext::default();
        let id_addr = Memory::HEAP_START;
        let var_addr = Memory::HEAP_START + 64;
        memory.write_bytes(id_addr, &[0u8; 32]).unwrap();

        let registers = [id_addr, var_addr, 0, 8, 0];
        let ret = sol_get_sysvar(
            registers,
            &mut memory,
            &meter(1_000_000),
            &costs(),
            &sysvars,
        )
        .unwrap();
        assert_eq!(ret, 2);
    }

    #[test]
    fn test_sol_get_sysvar_offset_length_exceeds() {
        let mut memory = make_memory();
        let sysvars = SysvarContext::default();
        let id_addr = Memory::HEAP_START;
        let var_addr = Memory::HEAP_START + 64;
        memory
            .write_bytes(id_addr, solana_rent::sysvar::ID.as_ref())
            .unwrap();

        // rent is 17 bytes, so offset 16 + length 8 should exceed
        let registers = [id_addr, var_addr, 16, 8, 0];
        let ret = sol_get_sysvar(
            registers,
            &mut memory,
            &meter(1_000_000),
            &costs(),
            &sysvars,
        )
        .unwrap();
        assert_eq!(ret, 1);
    }
}
