pub mod abort;
pub mod crypto;
pub mod log;
pub mod memory;
pub mod pda;
pub mod return_data;
pub mod sysvar;

use {
    crate::{
        config::{ExecutionCost, SysvarContext},
        cpi::request::{self, CpiRequest},
        runtime::LogCollector,
    },
    sbpf_vm::{
        compute::ComputeMeter, errors::SbpfVmResult, memory::Memory, syscalls::SyscallHandler,
    },
    solana_address::Address,
};

const ACCOUNT_META_SIZE: u64 = 34;
const ACCOUNT_INFO_BYTE_SIZE: u64 = 80;

pub struct RuntimeSyscallHandler {
    pub costs: ExecutionCost,
    pub program_id: Address,
    pub sysvars: SysvarContext,
    pub pending_cpi: Option<CpiRequest>,
    pub return_data: crate::cpi::ReturnData,
    pub log_collector: LogCollector,
}

impl RuntimeSyscallHandler {
    pub fn new(
        costs: ExecutionCost,
        program_id: Address,
        sysvars: SysvarContext,
        log_collector: LogCollector,
    ) -> Self {
        Self {
            costs,
            program_id,
            sysvars,
            pending_cpi: None,
            return_data: None,
            log_collector,
        }
    }
}

// Consume CUs for CPI.
fn consume_cpi_compute_units(
    request: &CpiRequest,
    compute: &ComputeMeter,
    costs: &ExecutionCost,
) -> SbpfVmResult<()> {
    // Base invoke cost.
    compute.consume(costs.invoke_units)?;

    // Instruction data and account meta cost.
    let data_cost = request.data.len() as u64 / costs.cpi_bytes_per_unit;
    let meta_cost = (request.accounts.len() as u64 * ACCOUNT_META_SIZE) / costs.cpi_bytes_per_unit;
    compute.consume(data_cost + meta_cost)?;

    // Account info translation cost.
    let account_info_cost =
        (request.caller_accounts.len() as u64 * ACCOUNT_INFO_BYTE_SIZE) / costs.cpi_bytes_per_unit;
    compute.consume(account_info_cost)?;

    // Per-account data cost (skip duplicates).
    let mut seen = Vec::with_capacity(request.accounts.len());
    for meta in request.accounts.iter() {
        if seen.contains(&meta.pubkey) {
            continue;
        }
        seen.push(meta.pubkey);
        if let Some(caller) = request
            .caller_accounts
            .iter()
            .find(|c| c.pubkey == meta.pubkey)
        {
            let acct_cost = caller.data_len / costs.cpi_bytes_per_unit;
            compute.consume(acct_cost)?;
        }
    }
    Ok(())
}

impl SyscallHandler for RuntimeSyscallHandler {
    fn handle(
        &mut self,
        name: &str,
        registers: [u64; 5],
        memory: &mut Memory,
        compute: ComputeMeter,
    ) -> SbpfVmResult<u64> {
        match name {
            "sol_log_" => log::sol_log(
                registers,
                memory,
                &compute,
                &self.costs,
                &self.log_collector,
            ),
            "sol_log_64_" => log::sol_log_64(registers, &compute, &self.costs, &self.log_collector),
            "sol_log_pubkey" => log::sol_log_pubkey(
                registers,
                memory,
                &compute,
                &self.costs,
                &self.log_collector,
            ),
            "sol_log_compute_units_" => {
                log::sol_log_compute_units(&compute, &self.costs, &self.log_collector)
            }
            "sol_remaining_compute_units" => {
                log::sol_remaining_compute_units(&compute, &self.costs)
            }

            "sol_memcpy_" => memory::sol_memcpy(registers, memory, &compute, &self.costs),
            "sol_memmove_" => memory::sol_memmove(registers, memory, &compute, &self.costs),
            "sol_memset_" => memory::sol_memset(registers, memory, &compute, &self.costs),
            "sol_memcmp_" => memory::sol_memcmp(registers, memory, &compute, &self.costs),

            "abort" => abort::abort(),
            "sol_panic_" => abort::sol_panic(registers, memory),

            "sol_sha256" => crypto::sol_sha256(registers, memory, &compute, &self.costs),
            "sol_keccak256" => crypto::sol_keccak256(registers, memory, &compute, &self.costs),
            "sol_blake3" => crypto::sol_blake3(registers, memory, &compute, &self.costs),

            "sol_create_program_address" => {
                pda::sol_create_program_address(registers, memory, &compute, &self.costs)
            }
            "sol_try_find_program_address" => {
                pda::sol_try_find_program_address(registers, memory, &compute, &self.costs)
            }

            "sol_get_clock_sysvar" => sysvar::sol_get_clock_sysvar(
                registers,
                memory,
                &compute,
                &self.costs,
                &self.sysvars,
            ),
            "sol_get_rent_sysvar" => {
                sysvar::sol_get_rent_sysvar(registers, memory, &compute, &self.costs, &self.sysvars)
            }
            "sol_get_epoch_schedule_sysvar" => sysvar::sol_get_epoch_schedule_sysvar(
                registers,
                memory,
                &compute,
                &self.costs,
                &self.sysvars,
            ),
            "sol_get_last_restart_slot_sysvar" => sysvar::sol_get_last_restart_slot_sysvar(
                registers,
                memory,
                &compute,
                &self.costs,
                &self.sysvars,
            ),

            "sol_set_return_data" => {
                let (result, data) = return_data::sol_set_return_data(
                    registers,
                    memory,
                    &compute,
                    &self.costs,
                    &self.program_id,
                )?;
                self.return_data = data;
                Ok(result)
            }
            "sol_get_return_data" => return_data::sol_get_return_data(
                registers,
                memory,
                &compute,
                &self.costs,
                &self.return_data,
            ),

            "sol_invoke_signed_c" => {
                let request = request::parse_cpi_c(registers, memory, &self.program_id)?;
                consume_cpi_compute_units(&request, &compute, &self.costs)?;
                self.pending_cpi = Some(request);
                Ok(0)
            }
            "sol_invoke_signed_rust" => {
                let request = request::parse_cpi_rust(registers, memory, &self.program_id)?;
                consume_cpi_compute_units(&request, &compute, &self.costs)?;
                self.pending_cpi = Some(request);
                Ok(0)
            }

            _ => {
                compute.consume(self.costs.syscall_base_cost)?;
                eprintln!("Unknown syscall: {}", name);
                Ok(0)
            }
        }
    }
}
