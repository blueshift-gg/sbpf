pub mod request;
pub mod sync;
pub mod validate;

use {
    crate::{
        config::{ExecutionCost, RuntimeConfig, SysvarContext},
        errors::{RuntimeError, RuntimeResult},
        serialize,
        syscalls::RuntimeSyscallHandler,
    },
    either::Either,
    request::CpiRequest,
    sbpf_common::{inst_param::Number, opcode::Opcode},
    sbpf_disassembler::program::Program,
    sbpf_vm::{
        compute::ComputeMeter,
        memory::Memory,
        vm::{SbpfVm, SbpfVmConfig},
    },
    solana_account::Account,
    solana_address::Address,
    solana_instruction::AccountMeta,
    std::collections::HashMap,
};

pub fn execute_cpi(
    request: CpiRequest,
    programs: &HashMap<Address, Vec<u8>>,
    accounts: &mut HashMap<Address, Account>,
    config: &RuntimeConfig,
    sysvars: &SysvarContext,
    compute_remaining: u64,
    cpi_depth: usize,
    caller_account_metas: &[AccountMeta],
) -> RuntimeResult<(u64, Option<(Address, Vec<u8>)>, u64)> {
    if cpi_depth >= config.max_cpi_depth {
        return Err(RuntimeError::CpiDepthExceeded(config.max_cpi_depth));
    }

    validate::check_privileges(&request, caller_account_metas)?;

    let elf_bytes = programs
        .get(&request.program_id)
        .ok_or_else(|| RuntimeError::ProgramNotFound(request.program_id.to_string()))?;

    let program = Program::from_bytes(elf_bytes)
        .map_err(|e| RuntimeError::ElfParseError(format!("{:?}", e)))?;
    let entrypoint = program.get_entrypoint_offset().unwrap_or(0);
    let (mut instructions, rodata_section) = program
        .to_ixs()
        .map_err(|e| RuntimeError::ElfParseError(format!("{:?}", e)))?;

    let rodata = rodata_section
        .as_ref()
        .map(|s| s.data.clone())
        .unwrap_or_default();

    if let Some(ref section) = rodata_section {
        let elf_base = section.base_address;
        let elf_end = elf_base + section.data.len() as u64;
        for ix in &mut instructions {
            if ix.opcode == Opcode::Lddw
                && let Some(Either::Right(Number::Int(imm))) = &ix.imm
            {
                let addr = *imm as u64;
                if addr >= elf_base && addr < elf_end {
                    ix.imm = Some(Either::Right(Number::Int(
                        (Memory::RODATA_START + addr - elf_base) as i64,
                    )));
                }
            }
        }
    }

    let account_metas: Vec<AccountMeta> = request
        .accounts
        .iter()
        .map(|a| AccountMeta {
            pubkey: a.pubkey,
            is_signer: a.is_signer,
            is_writable: a.is_writable,
        })
        .collect();

    let callee_accounts: Vec<(Address, Account)> = account_metas
        .iter()
        .filter_map(|meta| accounts.get(&meta.pubkey).map(|a| (meta.pubkey, a.clone())))
        .collect();

    let input = serialize::serialize_parameters(
        &callee_accounts.iter().cloned().collect(),
        &account_metas,
        &request.data,
        &request.program_id,
    )?;

    let vm_config = SbpfVmConfig {
        compute_unit_limit: compute_remaining,
        max_call_depth: config.max_call_depth,
        heap_size: config.heap_size,
    };

    let handler = RuntimeSyscallHandler::new(
        ExecutionCost::default(),
        request.program_id,
        sysvars.clone(),
    );

    let mut callee_vm = SbpfVm::new_with_config(instructions, input, rodata, handler, vm_config);
    callee_vm.compute_meter = ComputeMeter::new(compute_remaining);
    callee_vm.set_entrypoint(entrypoint as usize);

    // Run callee VM to completion.
    loop {
        callee_vm.step()?;

        // Handle nested CPI.
        if let Some(nested_request) = callee_vm.syscall_handler.pending_cpi.take() {
            sync::sync_from_caller(&callee_vm.memory, &nested_request.caller_accounts, accounts)?;

            let caller_accounts_for_sync = nested_request.caller_accounts;
            let nested_consumed = callee_vm.compute_meter.get_consumed();
            let nested_remaining = compute_remaining.saturating_sub(nested_consumed);

            let cpi_request = CpiRequest {
                program_id: nested_request.program_id,
                accounts: nested_request.accounts,
                data: nested_request.data,
                caller_accounts: Vec::new(),
                signers: nested_request.signers,
            };

            let (nested_exit, nested_return_data, nested_cu) = execute_cpi(
                cpi_request,
                programs,
                accounts,
                config,
                sysvars,
                nested_remaining,
                cpi_depth + 1,
                &account_metas,
            )?;

            callee_vm.compute_meter.consume(nested_cu)?;
            callee_vm.syscall_handler.return_data = nested_return_data;

            if nested_exit != 0 {
                let consumed = callee_vm.compute_meter.get_consumed();
                return Ok((
                    nested_exit,
                    callee_vm.syscall_handler.return_data.take(),
                    consumed,
                ));
            }

            sync::sync_to_caller(&mut callee_vm.memory, &caller_accounts_for_sync, accounts)?;
        }

        if callee_vm.halted {
            break;
        }
    }

    let exit_code = callee_vm.exit_code.unwrap_or(0);
    let callee_return_data = callee_vm.syscall_handler.return_data.take();
    let consumed = callee_vm.compute_meter.get_consumed();

    if exit_code != 0 {
        return Ok((exit_code, callee_return_data, consumed));
    }

    // Sync callee's account changes back to the runtime's account store.
    serialize::deserialize_parameters(accounts, &account_metas, &callee_vm.memory.input);

    // Validate account changes.
    validate::check_account_changes(
        &request.program_id,
        &account_metas,
        &callee_accounts,
        accounts,
    )?;

    Ok((0, callee_return_data, consumed))
}
