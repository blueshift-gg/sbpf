use {
    crate::{config::ExecutionCost, cpi::ReturnData},
    sbpf_vm::{compute::ComputeMeter, errors::SbpfVmResult, memory::Memory},
    solana_address::Address,
};

const MAX_RETURN_DATA: usize = 1024;

pub fn sol_set_return_data(
    registers: [u64; 5],
    memory: &Memory,
    compute: &ComputeMeter,
    costs: &ExecutionCost,
    program_id: &Address,
) -> SbpfVmResult<(u64, ReturnData)> {
    let addr = registers[0];
    let len = registers[1];

    let cost = len
        .checked_div(costs.cpi_bytes_per_unit)
        .unwrap_or(u64::MAX)
        .saturating_add(costs.syscall_base_cost);
    compute.consume(cost)?;

    if len > MAX_RETURN_DATA as u64 {
        return Err(sbpf_vm::errors::SbpfVmError::SyscallError(format!(
            "Return data too large: {} > {}",
            len, MAX_RETURN_DATA
        )));
    }

    let data = if len == 0 {
        Vec::new()
    } else {
        memory.read_bytes(addr, len as usize)?.to_vec()
    };

    Ok((0, Some((*program_id, data))))
}

pub fn sol_get_return_data(
    registers: [u64; 5],
    memory: &mut Memory,
    compute: &ComputeMeter,
    costs: &ExecutionCost,
    return_data: &ReturnData,
) -> SbpfVmResult<u64> {
    let buf_addr = registers[0];
    let buf_len = registers[1];
    let program_id_addr = registers[2];

    compute.consume(costs.syscall_base_cost)?;

    let Some((program_id, data)) = return_data else {
        return Ok(0);
    };

    let length = buf_len.min(data.len() as u64);

    if length != 0 {
        let cost = length
            .saturating_add(32)
            .checked_div(costs.cpi_bytes_per_unit)
            .unwrap_or(u64::MAX);
        compute.consume(cost)?;

        let from_slice = &data[..length as usize];
        memory.write_bytes(buf_addr, from_slice)?;
        memory.write_bytes(program_id_addr, program_id.as_ref())?;
    }

    Ok(data.len() as u64)
}
