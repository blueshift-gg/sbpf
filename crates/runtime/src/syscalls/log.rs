use {
    crate::{config::ExecutionCost, runtime::LogCollector},
    sbpf_vm::{compute::ComputeMeter, errors::SbpfVmResult, memory::Memory},
};

pub fn sol_log(
    registers: [u64; 5],
    memory: &Memory,
    compute: &ComputeMeter,
    costs: &ExecutionCost,
    log_collector: &LogCollector,
) -> SbpfVmResult<u64> {
    let msg_ptr = registers[0];
    let msg_len = registers[1];

    compute.consume(costs.syscall_base_cost.max(msg_len))?;

    let msg_bytes = memory.read_bytes(msg_ptr, msg_len as usize)?;
    let msg = String::from_utf8_lossy(msg_bytes);
    log_collector
        .borrow_mut()
        .push(format!("Program log: {}", msg));
    Ok(0)
}

pub fn sol_log_64(
    registers: [u64; 5],
    compute: &ComputeMeter,
    costs: &ExecutionCost,
    log_collector: &LogCollector,
) -> SbpfVmResult<u64> {
    compute.consume(costs.log_64_units)?;
    log_collector.borrow_mut().push(format!(
        "Program log: {:#x}, {:#x}, {:#x}, {:#x}, {:#x}",
        registers[0], registers[1], registers[2], registers[3], registers[4]
    ));
    Ok(0)
}

pub fn sol_log_pubkey(
    registers: [u64; 5],
    memory: &Memory,
    compute: &ComputeMeter,
    costs: &ExecutionCost,
    log_collector: &LogCollector,
) -> SbpfVmResult<u64> {
    compute.consume(costs.log_pubkey_units)?;

    let pubkey_bytes = memory.read_bytes(registers[0], 32)?;
    let pubkey_base58 = bs58::encode(pubkey_bytes).into_string();
    log_collector
        .borrow_mut()
        .push(format!("Program log: {}", pubkey_base58));
    Ok(0)
}

pub fn sol_log_compute_units(
    compute: &ComputeMeter,
    costs: &ExecutionCost,
    log_collector: &LogCollector,
) -> SbpfVmResult<u64> {
    compute.consume(costs.syscall_base_cost)?;
    log_collector.borrow_mut().push(format!(
        "Program consumption: {} units remaining",
        compute.get_remaining()
    ));
    Ok(0)
}

pub fn sol_remaining_compute_units(
    compute: &ComputeMeter,
    costs: &ExecutionCost,
) -> SbpfVmResult<u64> {
    compute.consume(costs.syscall_base_cost)?;
    Ok(compute.get_remaining())
}
