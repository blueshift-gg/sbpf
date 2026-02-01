use {
    crate::execution_cost::ExecutionCost,
    sbpf_vm::{
        compute::ComputeMeter,
        errors::{SbpfVmError, SbpfVmResult},
        memory::Memory,
        syscalls::SyscallHandler,
    },
};

/// Debugger syscall handler
#[derive(Debug)]
pub struct DebuggerSyscallHandler {
    pub costs: ExecutionCost,
}

impl Default for DebuggerSyscallHandler {
    fn default() -> Self {
        Self {
            costs: ExecutionCost::default(),
        }
    }
}

impl DebuggerSyscallHandler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_costs(costs: ExecutionCost) -> Self {
        Self { costs }
    }

    fn sol_log(
        &mut self,
        registers: [u64; 5],
        memory: &Memory,
        compute: &mut ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let msg_ptr = registers[0];
        let msg_len = registers[1];

        let cost = self.costs.syscall_base_cost.max(msg_len);
        compute.consume(cost)?;

        let msg_bytes = memory.read_bytes(msg_ptr, msg_len as usize)?;
        let msg = String::from_utf8_lossy(&msg_bytes);
        println!("Program log: {}", msg);
        Ok(0)
    }

    fn sol_log_64(&mut self, registers: [u64; 5], compute: &mut ComputeMeter) -> SbpfVmResult<u64> {
        let cost = self.costs.log_64_units;
        compute.consume(cost)?;

        println!(
            "Program log: {:#x}, {:#x}, {:#x}, {:#x}, {:#x}",
            registers[0], registers[1], registers[2], registers[3], registers[4]
        );
        Ok(0)
    }

    fn sol_log_pubkey(
        &mut self,
        registers: [u64; 5],
        memory: &Memory,
        compute: &mut ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let cost = self.costs.log_pubkey_units;
        compute.consume(cost)?;

        let pubkey_ptr = registers[0];
        let pubkey_bytes = memory.read_bytes(pubkey_ptr, 32)?;
        let pubkey_base58 = bs58::encode(pubkey_bytes).into_string();
        println!("Program log: {}", pubkey_base58);
        Ok(0)
    }

    fn sol_log_compute_units(&mut self, compute: &mut ComputeMeter) -> SbpfVmResult<u64> {
        let cost = self.costs.syscall_base_cost;
        compute.consume(cost)?;

        let remaining = compute.get_remaining();
        println!("Program consumption: {} units remaining", remaining);
        Ok(0)
    }

    fn sol_remaining_compute_units(&mut self, compute: &mut ComputeMeter) -> SbpfVmResult<u64> {
        let cost = self.costs.syscall_base_cost;
        compute.consume(cost)?;

        Ok(compute.get_remaining())
    }

    // Helper for memory operations CU consumption
    fn mem_op_consume(&self, n: u64, compute: &mut ComputeMeter) -> SbpfVmResult<()> {
        let cost = self.costs.mem_op_base_cost.max(
            n.checked_div(self.costs.cpi_bytes_per_unit)
                .unwrap_or(u64::MAX),
        );
        compute.consume(cost)
    }

    fn is_nonoverlapping(src: u64, src_len: u64, dst: u64, dst_len: u64) -> bool {
        if src > dst {
            src.saturating_sub(dst) >= dst_len
        } else {
            dst.saturating_sub(src) >= src_len
        }
    }

    fn sol_memcpy(
        &mut self,
        registers: [u64; 5],
        memory: &mut Memory,
        compute: &mut ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let dst = registers[0];
        let src = registers[1];
        let n = registers[2];

        self.mem_op_consume(n, compute)?;

        if !Self::is_nonoverlapping(src, n, dst, n) {
            return Err(SbpfVmError::OverlappingMemoryRegions);
        }

        let data = memory.read_bytes(src, n as usize)?.to_vec();
        memory.write_bytes(dst, &data)?;

        Ok(0)
    }

    fn sol_memmove(
        &mut self,
        registers: [u64; 5],
        memory: &mut Memory,
        compute: &mut ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let dst = registers[0];
        let src = registers[1];
        let n = registers[2];

        self.mem_op_consume(n, compute)?;

        let data = memory.read_bytes(src, n as usize)?.to_vec();
        memory.write_bytes(dst, &data)?;

        Ok(0)
    }

    fn sol_memset(
        &mut self,
        registers: [u64; 5],
        memory: &mut Memory,
        compute: &mut ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let dst = registers[0];
        let c = registers[1] as u8;
        let n = registers[2];

        self.mem_op_consume(n, compute)?;

        let data = vec![c; n as usize];
        memory.write_bytes(dst, &data)?;

        Ok(0)
    }

    fn sol_memcmp(
        &mut self,
        registers: [u64; 5],
        memory: &mut Memory,
        compute: &mut ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let s1 = registers[0];
        let s2 = registers[1];
        let n = registers[2];
        let result_ptr = registers[3];

        self.mem_op_consume(n, compute)?;

        let s1_bytes = memory.read_bytes(s1, n as usize)?;
        let s2_bytes = memory.read_bytes(s2, n as usize)?;

        let mut result: i32 = 0;
        for i in 0..n as usize {
            if s1_bytes[i] != s2_bytes[i] {
                result = (s1_bytes[i] as i32).saturating_sub(s2_bytes[i] as i32);
                break;
            }
        }

        memory.write_u32(result_ptr, result as u32)?;

        Ok(0)
    }

    fn abort(&mut self) -> SbpfVmResult<u64> {
        Err(SbpfVmError::Abort)
    }

    fn sol_panic(
        &mut self,
        registers: [u64; 5],
        memory: &Memory,
        compute: &mut ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let file_ptr = registers[0];
        let file_len = registers[1];
        let line = registers[2];
        let column = registers[3];

        compute.consume(file_len)?;

        let file_bytes = memory.read_bytes(file_ptr, file_len as usize)?;
        let file = String::from_utf8_lossy(&file_bytes);

        eprintln!("Program panicked at {}:{}:{}", file, line, column);

        Err(SbpfVmError::Abort)
    }
}

impl SyscallHandler for DebuggerSyscallHandler {
    fn handle(
        &mut self,
        name: &str,
        registers: [u64; 5],
        memory: &mut Memory,
        compute: &mut ComputeMeter,
    ) -> SbpfVmResult<u64> {
        match name {
            // Logging
            "sol_log_" => self.sol_log(registers, memory, compute),
            "sol_log_64_" => self.sol_log_64(registers, compute),
            "sol_log_pubkey" => self.sol_log_pubkey(registers, memory, compute),
            "sol_log_compute_units_" => self.sol_log_compute_units(compute),
            "sol_remaining_compute_units" => self.sol_remaining_compute_units(compute),

            // Memory
            "sol_memcpy_" => self.sol_memcpy(registers, memory, compute),
            "sol_memmove_" => self.sol_memmove(registers, memory, compute),
            "sol_memset_" => self.sol_memset(registers, memory, compute),
            "sol_memcmp_" => self.sol_memcmp(registers, memory, compute),

            // Abort
            "abort" => self.abort(),
            "sol_panic_" => self.sol_panic(registers, memory, compute),

            // Unknown syscall
            _ => {
                compute.consume(self.costs.syscall_base_cost)?;
                eprintln!("Unknown syscall: {}", name);
                Ok(0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sol_log_64_cu_consumption() {
        let mut handler = DebuggerSyscallHandler::default();
        let mut compute = ComputeMeter::new(1000);

        let registers = [1, 2, 3, 4, 5];
        handler.sol_log_64(registers, &mut compute).unwrap();

        // should consume log_64_units (100)
        assert_eq!(compute.consumed, 100);
    }

    #[test]
    fn test_sol_log_64_budget_exceeded() {
        let mut handler = DebuggerSyscallHandler::default();
        let mut compute = ComputeMeter::new(50); // less than 100

        let registers = [1, 2, 3, 4, 5];
        let result = handler.sol_log_64(registers, &mut compute);

        assert!(matches!(
            result,
            Err(SbpfVmError::ComputeBudgetExceeded { .. })
        ));
    }
}
