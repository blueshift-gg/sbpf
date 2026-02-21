use {
    crate::execution_cost::ExecutionCost,
    blake3::Hasher as Blake3Hasher,
    sbpf_vm::{
        compute::ComputeMeter,
        errors::{SbpfVmError, SbpfVmResult},
        memory::Memory,
        syscalls::SyscallHandler,
    },
    sha2::{Digest, Sha256},
    sha3::Keccak256,
    solana_address::Address,
    solana_clock::Clock,
    solana_epoch_schedule::EpochSchedule,
    solana_rent::Rent,
    std::mem::size_of,
};

const MAX_SEED_LEN: usize = 32;
const MAX_SEEDS: usize = 16;

/// Debugger syscall handler
#[derive(Debug)]
pub struct DebuggerSyscallHandler {
    pub costs: ExecutionCost,
    pub current_program_id: Address,
    pub clock: Clock,
    pub rent: Rent,
    pub epoch_schedule: EpochSchedule,
}

impl DebuggerSyscallHandler {
    pub fn new(costs: ExecutionCost, current_program_id: Address) -> Self {
        Self {
            costs,
            current_program_id,
            clock: Clock::default(),
            rent: Rent::default(),
            epoch_schedule: EpochSchedule::default(),
        }
    }

    fn sol_log(
        &mut self,
        registers: [u64; 5],
        memory: &Memory,
        compute: &ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let msg_ptr = registers[0];
        let msg_len = registers[1];

        let cost = self.costs.syscall_base_cost.max(msg_len);
        compute.consume(cost)?;

        let msg_bytes = memory.read_bytes(msg_ptr, msg_len as usize)?;
        let msg = String::from_utf8_lossy(msg_bytes);
        println!("Program log: {}", msg);
        Ok(0)
    }

    fn sol_log_64(&mut self, registers: [u64; 5], compute: &ComputeMeter) -> SbpfVmResult<u64> {
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
        compute: &ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let cost = self.costs.log_pubkey_units;
        compute.consume(cost)?;

        let pubkey_ptr = registers[0];
        let pubkey_bytes = memory.read_bytes(pubkey_ptr, 32)?;
        let pubkey_base58 = bs58::encode(pubkey_bytes).into_string();
        println!("Program log: {}", pubkey_base58);
        Ok(0)
    }

    fn sol_log_compute_units(&mut self, compute: &ComputeMeter) -> SbpfVmResult<u64> {
        let cost = self.costs.syscall_base_cost;
        compute.consume(cost)?;

        let remaining = compute.get_remaining();
        println!("Program consumption: {} units remaining", remaining);
        Ok(0)
    }

    fn sol_remaining_compute_units(&mut self, compute: &ComputeMeter) -> SbpfVmResult<u64> {
        let cost = self.costs.syscall_base_cost;
        compute.consume(cost)?;
        Ok(compute.get_remaining())
    }

    fn mem_op_consume(&self, n: u64, compute: &ComputeMeter) -> SbpfVmResult<()> {
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
        compute: &ComputeMeter,
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
        compute: &ComputeMeter,
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
        compute: &ComputeMeter,
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
        compute: &ComputeMeter,
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
        compute: &ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let file_ptr = registers[0];
        let file_len = registers[1];
        let line = registers[2];
        let column = registers[3];

        compute.consume(file_len)?;

        let file_bytes = memory.read_bytes(file_ptr, file_len as usize)?;
        let file = String::from_utf8_lossy(file_bytes);

        eprintln!("Program panicked at {}:{}:{}", file, line, column);

        Err(SbpfVmError::Abort)
    }

    fn read_slices(
        &self,
        memory: &Memory,
        vals_addr: u64,
        vals_len: u64,
    ) -> SbpfVmResult<Vec<(u64, u64)>> {
        let mut slices = Vec::with_capacity(vals_len as usize);
        for i in 0..vals_len {
            let slice_addr = vals_addr.saturating_add(i.saturating_mul(16));
            let ptr = memory.read_u64(slice_addr)?;
            let len = memory.read_u64(slice_addr.saturating_add(8))?;
            slices.push((ptr, len));
        }
        Ok(slices)
    }

    fn hash_slices<H: Digest>(
        &mut self,
        memory: &mut Memory,
        compute: &ComputeMeter,
        vals_addr: u64,
        vals_len: u64,
        result_addr: u64,
    ) -> SbpfVmResult<u64> {
        if vals_len > self.costs.sha256_max_slices {
            return Err(SbpfVmError::TooManySlices);
        }

        compute.consume(self.costs.sha256_base_cost)?;

        let mut hasher = H::new();
        if vals_len > 0 {
            for (ptr, len) in self.read_slices(memory, vals_addr, vals_len)? {
                let cost = self
                    .costs
                    .mem_op_base_cost
                    .max(self.costs.sha256_byte_cost.saturating_mul(len / 2));
                compute.consume(cost)?;
                hasher.update(memory.read_bytes(ptr, len as usize)?);
            }
        }

        memory.write_bytes(result_addr, &hasher.finalize())?;
        Ok(0)
    }

    fn sol_sha256(
        &mut self,
        registers: [u64; 5],
        memory: &mut Memory,
        compute: &ComputeMeter,
    ) -> SbpfVmResult<u64> {
        self.hash_slices::<Sha256>(memory, compute, registers[0], registers[1], registers[2])
    }

    fn sol_keccak256(
        &mut self,
        registers: [u64; 5],
        memory: &mut Memory,
        compute: &ComputeMeter,
    ) -> SbpfVmResult<u64> {
        self.hash_slices::<Keccak256>(memory, compute, registers[0], registers[1], registers[2])
    }

    fn sol_blake3(
        &mut self,
        registers: [u64; 5],
        memory: &mut Memory,
        compute: &ComputeMeter,
    ) -> SbpfVmResult<u64> {
        self.hash_slices::<Blake3Hasher>(memory, compute, registers[0], registers[1], registers[2])
    }

    fn read_seeds(
        &self,
        memory: &Memory,
        seeds_addr: u64,
        seeds_len: u64,
    ) -> SbpfVmResult<Vec<Vec<u8>>> {
        if seeds_len as usize > MAX_SEEDS {
            return Err(SbpfVmError::MaxSeedLengthExceeded);
        }

        let mut seeds = Vec::with_capacity(seeds_len as usize);
        for i in 0..seeds_len {
            let slice_addr = seeds_addr.saturating_add(i.saturating_mul(16));
            let ptr = memory.read_u64(slice_addr)?;
            let len = memory.read_u64(slice_addr.saturating_add(8))?;

            if len as usize > MAX_SEED_LEN {
                return Err(SbpfVmError::MaxSeedLengthExceeded);
            }

            let seed = memory.read_bytes(ptr, len as usize)?.to_vec();
            seeds.push(seed);
        }
        Ok(seeds)
    }

    fn sol_create_program_address(
        &mut self,
        registers: [u64; 5],
        memory: &mut Memory,
        compute: &ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let seeds_addr = registers[0];
        let seeds_len = registers[1];
        let program_id_addr = registers[2];
        let address_addr = registers[3];

        let cost = self.costs.create_program_address_units;
        compute.consume(cost)?;

        let seeds = self.read_seeds(memory, seeds_addr, seeds_len)?;
        let program_id = Address::from(
            <[u8; 32]>::try_from(memory.read_bytes(program_id_addr, 32)?)
                .map_err(|_| SbpfVmError::InvalidSliceConversion)?,
        );

        let seed_slices: Vec<&[u8]> = seeds.iter().map(|s| s.as_slice()).collect();
        match Address::create_program_address(&seed_slices, &program_id) {
            Ok(addr) => {
                memory.write_bytes(address_addr, addr.as_ref())?;
                Ok(0)
            }
            Err(_) => Ok(1),
        }
    }

    fn sol_try_find_program_address(
        &mut self,
        registers: [u64; 5],
        memory: &mut Memory,
        compute: &ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let seeds_addr = registers[0];
        let seeds_len = registers[1];
        let program_id_addr = registers[2];
        let address_addr = registers[3];
        let bump_seed_addr = registers[4];

        let cost = self.costs.create_program_address_units;
        compute.consume(cost)?;

        let seeds = self.read_seeds(memory, seeds_addr, seeds_len)?;
        let program_id = Address::from(
            <[u8; 32]>::try_from(memory.read_bytes(program_id_addr, 32)?)
                .map_err(|_| SbpfVmError::InvalidSliceConversion)?,
        );

        let seed_slices: Vec<&[u8]> = seeds.iter().map(|s| s.as_slice()).collect();
        match Address::try_find_program_address(&seed_slices, &program_id) {
            Some((addr, bump)) => {
                memory.write_u8(bump_seed_addr, bump)?;
                memory.write_bytes(address_addr, addr.as_ref())?;
                Ok(0)
            }
            None => Ok(1),
        }
    }

    fn write_sysvar_bytes<T>(
        &self,
        memory: &mut Memory,
        addr: u64,
        sysvar: &T,
    ) -> SbpfVmResult<()> {
        let bytes =
            unsafe { std::slice::from_raw_parts(sysvar as *const T as *const u8, size_of::<T>()) };
        memory.write_bytes(addr, bytes)
    }

    fn sol_get_clock_sysvar(
        &mut self,
        registers: [u64; 5],
        memory: &mut Memory,
        compute: &ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let cost = self
            .costs
            .sysvar_base_cost
            .saturating_add(size_of::<Clock>() as u64);
        compute.consume(cost)?;
        let clock = self.clock.clone();
        self.write_sysvar_bytes(memory, registers[0], &clock)?;
        Ok(0)
    }

    fn sol_get_rent_sysvar(
        &mut self,
        registers: [u64; 5],
        memory: &mut Memory,
        compute: &ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let cost = self
            .costs
            .sysvar_base_cost
            .saturating_add(size_of::<Rent>() as u64);
        compute.consume(cost)?;
        let rent = self.rent.clone();
        self.write_sysvar_bytes(memory, registers[0], &rent)?;
        Ok(0)
    }

    fn sol_get_epoch_schedule_sysvar(
        &mut self,
        registers: [u64; 5],
        memory: &mut Memory,
        compute: &ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let cost = self
            .costs
            .sysvar_base_cost
            .saturating_add(size_of::<EpochSchedule>() as u64);
        compute.consume(cost)?;
        let epoch_schedule = self.epoch_schedule.clone();
        self.write_sysvar_bytes(memory, registers[0], &epoch_schedule)?;
        Ok(0)
    }
}

impl SyscallHandler for DebuggerSyscallHandler {
    fn handle(
        &mut self,
        name: &str,
        registers: [u64; 5],
        memory: &mut Memory,
        compute: ComputeMeter,
    ) -> SbpfVmResult<u64> {
        match name {
            // Logging
            "sol_log_" => self.sol_log(registers, memory, &compute),
            "sol_log_64_" => self.sol_log_64(registers, &compute),
            "sol_log_pubkey" => self.sol_log_pubkey(registers, memory, &compute),
            "sol_log_compute_units_" => self.sol_log_compute_units(&compute),
            "sol_remaining_compute_units" => self.sol_remaining_compute_units(&compute),

            // Memory
            "sol_memcpy_" => self.sol_memcpy(registers, memory, &compute),
            "sol_memmove_" => self.sol_memmove(registers, memory, &compute),
            "sol_memset_" => self.sol_memset(registers, memory, &compute),
            "sol_memcmp_" => self.sol_memcmp(registers, memory, &compute),

            // Abort
            "abort" => self.abort(),
            "sol_panic_" => self.sol_panic(registers, memory, &compute),

            // Hashing
            "sol_sha256" => self.sol_sha256(registers, memory, &compute),
            "sol_keccak256" => self.sol_keccak256(registers, memory, &compute),
            "sol_blake3" => self.sol_blake3(registers, memory, &compute),

            // PDA
            "sol_create_program_address" => {
                self.sol_create_program_address(registers, memory, &compute)
            }
            "sol_try_find_program_address" => {
                self.sol_try_find_program_address(registers, memory, &compute)
            }

            // Sysvars
            "sol_get_clock_sysvar" => self.sol_get_clock_sysvar(registers, memory, &compute),
            "sol_get_rent_sysvar" => self.sol_get_rent_sysvar(registers, memory, &compute),
            "sol_get_epoch_schedule_sysvar" => {
                self.sol_get_epoch_schedule_sysvar(registers, memory, &compute)
            }

            // Unknown syscall
            _ => {
                let cost = self.costs.syscall_base_cost;
                compute.consume(cost)?;
                eprintln!("Unknown syscall: {}", name);
                Ok(0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_handler() -> DebuggerSyscallHandler {
        DebuggerSyscallHandler::new(ExecutionCost::default(), Address::new_unique())
    }

    #[test]
    fn test_sol_log_64_cu_consumption() {
        let mut handler = test_handler();
        let compute = ComputeMeter::new(1000);
        handler.sol_log_64([1, 2, 3, 4, 5], &compute).unwrap();
        assert_eq!(compute.borrow().consumed, 100);
    }

    #[test]
    fn test_sol_log_64_budget_exceeded() {
        let mut handler = test_handler();
        let compute = ComputeMeter::new(50);
        let result = handler.sol_log_64([1, 2, 3, 4, 5], &compute);
        assert!(matches!(
            result,
            Err(SbpfVmError::ComputeBudgetExceeded { .. })
        ));
    }
}
