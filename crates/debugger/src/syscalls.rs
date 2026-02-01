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
    solana_sdk::{clock::Clock, epoch_schedule::EpochSchedule, pubkey::Pubkey, rent::Rent},
    std::mem::size_of,
};

const MAX_SEED_LEN: usize = 32;
const MAX_SEEDS: usize = 16;
const MAX_RETURN_DATA: usize = 1024;

/// Debugger syscall handler
#[derive(Debug)]
pub struct DebuggerSyscallHandler {
    pub costs: ExecutionCost,
    pub clock: Clock,
    pub rent: Rent,
    pub epoch_schedule: EpochSchedule,
    pub return_data: Option<(Pubkey, Vec<u8>)>,
}

impl Default for DebuggerSyscallHandler {
    fn default() -> Self {
        Self {
            costs: ExecutionCost::default(),
            clock: Clock::default(),
            rent: Rent::default(),
            epoch_schedule: EpochSchedule::default(),
            return_data: None,
        }
    }
}

impl DebuggerSyscallHandler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_costs(costs: ExecutionCost) -> Self {
        Self {
            costs,
            ..Default::default()
        }
    }

    pub fn get_return_data(&self) -> Option<&(Pubkey, Vec<u8>)> {
        self.return_data.as_ref()
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

    fn sol_sha256(
        &mut self,
        registers: [u64; 5],
        memory: &mut Memory,
        compute: &mut ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let vals_addr = registers[0];
        let vals_len = registers[1];
        let result_addr = registers[2];

        if vals_len > self.costs.sha256_max_slices {
            return Err(SbpfVmError::TooManySlices);
        }

        compute.consume(self.costs.sha256_base_cost)?;

        let mut hasher = Sha256::new();

        if vals_len > 0 {
            let slices = self.read_slices(memory, vals_addr, vals_len)?;

            for (ptr, len) in slices {
                let bytes = memory.read_bytes(ptr, len as usize)?;

                let cost = self.costs.mem_op_base_cost.max(
                    self.costs
                        .sha256_byte_cost
                        .saturating_mul(len.checked_div(2).unwrap_or(0)),
                );
                compute.consume(cost)?;

                hasher.update(bytes);
            }
        }

        let hash = hasher.finalize();
        memory.write_bytes(result_addr, &hash)?;

        Ok(0)
    }

    fn sol_keccak256(
        &mut self,
        registers: [u64; 5],
        memory: &mut Memory,
        compute: &mut ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let vals_addr = registers[0];
        let vals_len = registers[1];
        let result_addr = registers[2];

        if vals_len > self.costs.sha256_max_slices {
            return Err(SbpfVmError::TooManySlices);
        }

        compute.consume(self.costs.sha256_base_cost)?;

        let mut hasher = Keccak256::new();

        if vals_len > 0 {
            let slices = self.read_slices(memory, vals_addr, vals_len)?;

            for (ptr, len) in slices {
                let bytes = memory.read_bytes(ptr, len as usize)?;

                let cost = self.costs.mem_op_base_cost.max(
                    self.costs
                        .sha256_byte_cost
                        .saturating_mul(len.checked_div(2).unwrap_or(0)),
                );
                compute.consume(cost)?;

                hasher.update(bytes);
            }
        }

        let hash = hasher.finalize();
        memory.write_bytes(result_addr, &hash)?;

        Ok(0)
    }

    fn sol_blake3(
        &mut self,
        registers: [u64; 5],
        memory: &mut Memory,
        compute: &mut ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let vals_addr = registers[0];
        let vals_len = registers[1];
        let result_addr = registers[2];

        if vals_len > self.costs.sha256_max_slices {
            return Err(SbpfVmError::TooManySlices);
        }

        compute.consume(self.costs.sha256_base_cost)?;

        let mut hasher = Blake3Hasher::new();

        if vals_len > 0 {
            let slices = self.read_slices(memory, vals_addr, vals_len)?;

            for (ptr, len) in slices {
                let bytes = memory.read_bytes(ptr, len as usize)?;

                let cost = self.costs.mem_op_base_cost.max(
                    self.costs
                        .sha256_byte_cost
                        .saturating_mul(len.checked_div(2).unwrap_or(0)),
                );
                compute.consume(cost)?;

                hasher.update(bytes);
            }
        }

        let hash = hasher.finalize();
        let hash_bytes: [u8; 32] = hash.into();
        memory.write_bytes(result_addr, &hash_bytes)?;

        Ok(0)
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
        compute: &mut ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let seeds_addr = registers[0];
        let seeds_len = registers[1];
        let program_id_addr = registers[2];
        let address_addr = registers[3];

        compute.consume(self.costs.create_program_address_units)?;

        let seeds = self.read_seeds(memory, seeds_addr, seeds_len)?;
        let program_id_bytes = memory.read_bytes(program_id_addr, 32)?;
        let program_id = Pubkey::from(
            <[u8; 32]>::try_from(program_id_bytes)
                .map_err(|_| SbpfVmError::InvalidSliceConversion)?,
        );

        let seed_slices: Vec<&[u8]> = seeds.iter().map(|s| s.as_slice()).collect();

        let Ok(new_address) = Pubkey::create_program_address(&seed_slices, &program_id) else {
            return Ok(1); // address is on curve
        };

        memory.write_bytes(address_addr, new_address.as_ref())?;
        Ok(0)
    }

    fn sol_try_find_program_address(
        &mut self,
        registers: [u64; 5],
        memory: &mut Memory,
        compute: &mut ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let seeds_addr = registers[0];
        let seeds_len = registers[1];
        let program_id_addr = registers[2];
        let address_addr = registers[3];
        let bump_seed_addr = registers[4];

        compute.consume(self.costs.create_program_address_units)?;

        let seeds = self.read_seeds(memory, seeds_addr, seeds_len)?;
        let program_id_bytes = memory.read_bytes(program_id_addr, 32)?;
        let program_id = Pubkey::from(
            <[u8; 32]>::try_from(program_id_bytes)
                .map_err(|_| SbpfVmError::InvalidSliceConversion)?,
        );

        let seed_slices: Vec<&[u8]> = seeds.iter().map(|s| s.as_slice()).collect();

        let Some((new_address, bump)) = Pubkey::try_find_program_address(&seed_slices, &program_id)
        else {
            return Ok(1); // no valid PDA found
        };

        memory.write_u8(bump_seed_addr, bump)?;
        memory.write_bytes(address_addr, new_address.as_ref())?;
        Ok(0)
    }

    fn write_sysvar_bytes<T>(
        &self,
        memory: &mut Memory,
        var_addr: u64,
        sysvar: &T,
    ) -> SbpfVmResult<()> {
        let sysvar_bytes =
            unsafe { std::slice::from_raw_parts(sysvar as *const T as *const u8, size_of::<T>()) };
        memory.write_bytes(var_addr, sysvar_bytes)
    }

    fn sol_get_clock_sysvar(
        &mut self,
        registers: [u64; 5],
        memory: &mut Memory,
        compute: &mut ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let var_addr = registers[0];

        let cost = self
            .costs
            .sysvar_base_cost
            .saturating_add(size_of::<Clock>() as u64);
        compute.consume(cost)?;

        self.write_sysvar_bytes(memory, var_addr, &self.clock)?;

        Ok(0)
    }

    fn sol_get_rent_sysvar(
        &mut self,
        registers: [u64; 5],
        memory: &mut Memory,
        compute: &mut ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let var_addr = registers[0];

        let cost = self
            .costs
            .sysvar_base_cost
            .saturating_add(size_of::<Rent>() as u64);
        compute.consume(cost)?;

        self.write_sysvar_bytes(memory, var_addr, &self.rent)?;

        Ok(0)
    }

    fn sol_get_epoch_schedule_sysvar(
        &mut self,
        registers: [u64; 5],
        memory: &mut Memory,
        compute: &mut ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let var_addr = registers[0];

        let cost = self
            .costs
            .sysvar_base_cost
            .saturating_add(size_of::<EpochSchedule>() as u64);
        compute.consume(cost)?;

        self.write_sysvar_bytes(memory, var_addr, &self.epoch_schedule)?;

        Ok(0)
    }

    fn sol_set_return_data(
        &mut self,
        registers: [u64; 5],
        memory: &Memory,
        compute: &mut ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let data_addr = registers[0];
        let data_len = registers[1];

        let cost = self.costs.syscall_base_cost.saturating_add(
            data_len
                .checked_div(self.costs.cpi_bytes_per_unit)
                .unwrap_or(0),
        );
        compute.consume(cost)?;

        if data_len as usize > MAX_RETURN_DATA {
            return Err(SbpfVmError::ReturnDataTooLarge);
        }

        let data = if data_len > 0 {
            memory.read_bytes(data_addr, data_len as usize)?.to_vec()
        } else {
            Vec::new()
        };

        self.return_data = Some((Pubkey::default(), data));

        Ok(0)
    }

    fn sol_get_return_data(
        &mut self,
        registers: [u64; 5],
        memory: &mut Memory,
        compute: &mut ComputeMeter,
    ) -> SbpfVmResult<u64> {
        let data_addr = registers[0];
        let data_len = registers[1];
        let program_id_addr = registers[2];

        let cost = self.costs.syscall_base_cost.saturating_add(
            data_len
                .checked_div(self.costs.cpi_bytes_per_unit)
                .unwrap_or(0),
        );
        compute.consume(cost)?;

        let Some((program_id, return_data)) = &self.return_data else {
            return Ok(0); // no return data
        };

        if program_id_addr != 0 {
            memory.write_bytes(program_id_addr, program_id.as_ref())?;
        }

        let copy_len = (data_len as usize).min(return_data.len());
        if copy_len > 0 && data_addr != 0 {
            memory.write_bytes(data_addr, &return_data[..copy_len])?;
        }

        Ok(return_data.len() as u64)
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

            // Hashing
            "sol_sha256" => self.sol_sha256(registers, memory, compute),
            "sol_keccak256" => self.sol_keccak256(registers, memory, compute),
            "sol_blake3" => self.sol_blake3(registers, memory, compute),

            // PDA
            "sol_create_program_address" => {
                self.sol_create_program_address(registers, memory, compute)
            }
            "sol_try_find_program_address" => {
                self.sol_try_find_program_address(registers, memory, compute)
            }

            // Sysvars
            "sol_get_clock_sysvar" => self.sol_get_clock_sysvar(registers, memory, compute),
            "sol_get_rent_sysvar" => self.sol_get_rent_sysvar(registers, memory, compute),
            "sol_get_epoch_schedule_sysvar" => {
                self.sol_get_epoch_schedule_sysvar(registers, memory, compute)
            }

            // Return Data
            "sol_set_return_data" => self.sol_set_return_data(registers, memory, compute),
            "sol_get_return_data" => self.sol_get_return_data(registers, memory, compute),

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
