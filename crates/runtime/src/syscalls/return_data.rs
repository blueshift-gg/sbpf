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

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::syscalls::tests::test_helpers::{costs, make_memory, meter},
        sbpf_vm::{errors::SbpfVmError, memory::Memory},
        solana_address::Address,
    };

    fn test_program_id() -> Address {
        Address::from([7u8; 32])
    }

    #[test]
    fn test_set_return_data_basic() {
        let mut memory = make_memory();
        let data = b"result";
        memory.write_bytes(Memory::HEAP_START, data).unwrap();

        let program_id = test_program_id();
        let registers = [Memory::HEAP_START, data.len() as u64, 0, 0, 0];
        let (ret, rd) =
            sol_set_return_data(registers, &memory, &meter(1_000_000), &costs(), &program_id)
                .unwrap();

        assert_eq!(ret, 0);
        let (stored_id, stored_data) = rd.unwrap();
        assert_eq!(stored_id, program_id);
        assert_eq!(stored_data, data);
    }

    #[test]
    fn test_set_return_data_empty() {
        let memory = make_memory();
        let program_id = test_program_id();
        let registers = [Memory::HEAP_START, 0, 0, 0, 0];
        let (ret, rd) =
            sol_set_return_data(registers, &memory, &meter(1_000_000), &costs(), &program_id)
                .unwrap();

        assert_eq!(ret, 0);
        let (_, stored_data) = rd.unwrap();
        assert!(stored_data.is_empty());
    }

    #[test]
    fn test_set_return_data_too_large() {
        let memory = make_memory();
        let program_id = test_program_id();
        // MAX_RETURN_DATA = 1024; pass 1025
        let registers = [Memory::HEAP_START, 1025, 0, 0, 0];
        let result =
            sol_set_return_data(registers, &memory, &meter(1_000_000), &costs(), &program_id);
        assert!(matches!(result, Err(SbpfVmError::SyscallError(_))));
    }

    #[test]
    fn test_set_return_data_compute_exhausted() {
        let memory = make_memory();
        let program_id = test_program_id();
        // cost = 0/250 + syscall_base_cost=100; budget=99 fails
        let registers = [Memory::HEAP_START, 0, 0, 0, 0];
        assert!(matches!(
            sol_set_return_data(registers, &memory, &meter(99), &costs(), &program_id),
            Err(SbpfVmError::ComputeBudgetExceeded { .. })
        ));
    }

    #[test]
    fn test_get_return_data_none() {
        let mut memory = make_memory();
        let registers = [Memory::HEAP_START, 32, Memory::HEAP_START + 64, 0, 0];
        let ret = sol_get_return_data(registers, &mut memory, &meter(1_000_000), &costs(), &None)
            .unwrap();
        assert_eq!(ret, 0);
    }

    #[test]
    fn test_get_return_data_full() {
        let mut memory = make_memory();
        let program_id = test_program_id();
        let data = vec![1u8, 2, 3, 4, 5];
        let return_data: ReturnData = Some((program_id, data.clone()));

        let buf_addr = Memory::HEAP_START;
        let pid_addr = Memory::HEAP_START + 64;
        let registers = [buf_addr, data.len() as u64, pid_addr, 0, 0];
        let ret = sol_get_return_data(
            registers,
            &mut memory,
            &meter(1_000_000),
            &costs(),
            &return_data,
        )
        .unwrap();

        assert_eq!(ret, data.len() as u64);
        assert_eq!(
            memory.read_bytes(buf_addr, data.len()).unwrap(),
            data.as_slice()
        );
        assert_eq!(
            memory.read_bytes(pid_addr, 32).unwrap(),
            program_id.as_ref()
        );
    }

    #[test]
    fn test_get_return_data_truncated_buf() {
        let mut memory = make_memory();
        let program_id = test_program_id();
        let data = vec![10u8, 20, 30, 40, 50];
        let return_data: ReturnData = Some((program_id, data.clone()));

        let buf_addr = Memory::HEAP_START;
        let pid_addr = Memory::HEAP_START + 64;
        // buf_len=3; should write only first 3 bytes but return full 5
        let registers = [buf_addr, 3, pid_addr, 0, 0];
        let ret = sol_get_return_data(
            registers,
            &mut memory,
            &meter(1_000_000),
            &costs(),
            &return_data,
        )
        .unwrap();

        assert_eq!(ret, 5, "returns full data length even when buf is smaller");
        assert_eq!(memory.read_bytes(buf_addr, 3).unwrap(), &[10, 20, 30]);
    }

    #[test]
    fn test_get_return_data_zero_buf_len() {
        let mut memory = make_memory();
        let program_id = test_program_id();
        let data = vec![1u8, 2, 3];
        let return_data: ReturnData = Some((program_id, data.clone()));

        // buf_len=0 → length=0 → no write, but returns data.len()
        let registers = [Memory::HEAP_START, 0, Memory::HEAP_START + 64, 0, 0];
        let ret = sol_get_return_data(
            registers,
            &mut memory,
            &meter(1_000_000),
            &costs(),
            &return_data,
        )
        .unwrap();
        assert_eq!(ret, 3);
    }

    #[test]
    fn test_get_return_data_compute_exhausted() {
        let mut memory = make_memory();
        let registers = [Memory::HEAP_START, 32, Memory::HEAP_START + 64, 0, 0];
        // syscall_base_cost=100; budget=99 fails
        assert!(matches!(
            sol_get_return_data(registers, &mut memory, &meter(99), &costs(), &None),
            Err(SbpfVmError::ComputeBudgetExceeded { .. })
        ));
    }
}
