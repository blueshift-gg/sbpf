use {
    crate::config::{ExecutionCost, SysvarContext},
    sbpf_vm::{compute::ComputeMeter, errors::SbpfVmResult, memory::Memory},
    solana_clock::Clock,
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

pub fn sol_get_last_restart_slot_sysvar(
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
    fn test_sol_get_last_restart_slot_sysvar() {
        let mut memory = make_memory();
        let mut sysvars = SysvarContext::default();
        sysvars.last_restart_slot.last_restart_slot = 12_345_678;

        let addr = Memory::HEAP_START;
        let registers = [addr, 0, 0, 0, 0];
        sol_get_last_restart_slot_sysvar(
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
}
