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

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            runtime::LogCollector,
            syscalls::tests::test_helpers::{costs, make_memory, meter},
        },
        sbpf_vm::{errors::SbpfVmError, memory::Memory},
        std::{cell::RefCell, rc::Rc},
    };

    fn new_log() -> LogCollector {
        Rc::new(RefCell::new(Vec::new()))
    }

    #[test]
    fn test_sol_log_basic() {
        let mut memory = make_memory();
        let msg = b"hello world";
        memory.write_bytes(Memory::HEAP_START, msg).unwrap();

        let log = new_log();
        let registers = [Memory::HEAP_START, msg.len() as u64, 0, 0, 0];
        sol_log(registers, &memory, &meter(1_000_000), &costs(), &log).unwrap();

        assert_eq!(log.borrow()[0], "Program log: hello world");
    }

    #[test]
    fn test_sol_log_empty_message() {
        let memory = make_memory();
        let log = new_log();
        let registers = [Memory::HEAP_START, 0, 0, 0, 0];
        sol_log(registers, &memory, &meter(1_000_000), &costs(), &log).unwrap();

        assert_eq!(log.borrow()[0], "Program log: ");
    }

    #[test]
    fn test_sol_log_invalid_utf8_is_lossy() {
        let mut memory = make_memory();
        memory.write_bytes(Memory::HEAP_START, &[0xFF]).unwrap();
        let log = new_log();
        let registers = [Memory::HEAP_START, 1, 0, 0, 0];
        sol_log(registers, &memory, &meter(1_000_000), &costs(), &log).unwrap();

        assert!(log.borrow()[0].starts_with("Program log: "));
    }

    #[test]
    fn test_sol_log_compute_exhausted() {
        let memory = make_memory();
        let log = new_log();
        let registers = [Memory::HEAP_START, 0, 0, 0, 0];
        assert!(matches!(
            sol_log(registers, &memory, &meter(99), &costs(), &log),
            Err(SbpfVmError::ComputeBudgetExceeded { .. })
        ));
    }

    #[test]
    fn test_sol_log_64_format() {
        let log = new_log();
        let registers = [1, 2, 3, 4, 5];
        sol_log_64(registers, &meter(1_000_000), &costs(), &log).unwrap();

        assert_eq!(log.borrow()[0], "Program log: 0x1, 0x2, 0x3, 0x4, 0x5");
    }

    #[test]
    fn test_sol_log_64_zeros() {
        let log = new_log();
        sol_log_64([0, 0, 0, 0, 0], &meter(1_000_000), &costs(), &log).unwrap();
        assert_eq!(log.borrow()[0], "Program log: 0x0, 0x0, 0x0, 0x0, 0x0");
    }

    #[test]
    fn test_sol_log_64_compute_exhausted() {
        let log = new_log();
        assert!(matches!(
            sol_log_64([0; 5], &meter(99), &costs(), &log),
            Err(SbpfVmError::ComputeBudgetExceeded { .. })
        ));
    }

    #[test]
    fn test_sol_log_pubkey_formats_base58() {
        let mut memory = make_memory();
        let pubkey = [0u8; 32];
        memory.write_bytes(Memory::HEAP_START, &pubkey).unwrap();

        let log = new_log();
        let registers = [Memory::HEAP_START, 0, 0, 0, 0];
        sol_log_pubkey(registers, &memory, &meter(1_000_000), &costs(), &log).unwrap();

        let expected = bs58::encode(&pubkey).into_string();
        assert_eq!(log.borrow()[0], format!("Program log: {}", expected));
    }

    #[test]
    fn test_sol_log_pubkey_nonzero() {
        let mut memory = make_memory();
        let mut pubkey = [0u8; 32];
        pubkey[0] = 1;
        memory.write_bytes(Memory::HEAP_START, &pubkey).unwrap();

        let log = new_log();
        let registers = [Memory::HEAP_START, 0, 0, 0, 0];
        sol_log_pubkey(registers, &memory, &meter(1_000_000), &costs(), &log).unwrap();

        let expected = bs58::encode(&pubkey).into_string();
        assert_eq!(log.borrow()[0], format!("Program log: {}", expected));
    }

    #[test]
    fn test_sol_log_compute_units_logs_remaining() {
        let compute = meter(1_000_000);
        let log = new_log();
        sol_log_compute_units(&compute, &costs(), &log).unwrap();

        // After consuming syscall_base_cost=100, remaining = 999_900
        let msg = &log.borrow()[0];
        assert!(msg.starts_with("Program consumption:"));
        assert!(msg.contains("999900 units remaining"));
    }

    #[test]
    fn test_sol_remaining_compute_units_returns_remaining() {
        let compute = meter(1_000);
        let result = sol_remaining_compute_units(&compute, &costs()).unwrap();
        assert_eq!(result, 900);
    }

    #[test]
    fn test_sol_remaining_compute_units_exhausted() {
        let compute = meter(50);
        assert!(matches!(
            sol_remaining_compute_units(&compute, &costs()),
            Err(SbpfVmError::ComputeBudgetExceeded { .. })
        ));
    }
}
