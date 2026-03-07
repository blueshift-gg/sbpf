use {
    crate::config::{ExecutionCost, SysvarContext},
    sbpf_vm::{compute::ComputeMeter, errors::SbpfVmResult, memory::Memory},
    solana_clock::Clock,
    solana_epoch_schedule::EpochSchedule,
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
