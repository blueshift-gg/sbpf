use {
    crate::{
        cpi::{
            CallerAccountInfo, CpiAccountMeta, CpiContext, CpiInstruction,
            serialization::{deserialize_input, serialize_input},
        },
        execution_cost::ExecutionCost,
        syscalls::DebuggerSyscallHandler,
    },
    either::Either,
    sbpf_common::{inst_param::Number, opcode::Opcode},
    sbpf_disassembler::program::Program,
    sbpf_vm::{
        compute::ComputeMeter,
        errors::{SbpfVmError, SbpfVmResult},
        memory::Memory,
        vm::{SbpfVm, SbpfVmConfig},
    },
    solana_sdk::{
        account::Account, clock::Clock, epoch_schedule::EpochSchedule, pubkey::Pubkey, rent::Rent,
    },
};

/// Sync account state from caller's memory to AccountStore
pub fn sync_accounts_from_caller(
    cpi_ctx: &CpiContext,
    memory: &Memory,
    caller_accounts: &[CallerAccountInfo],
) -> SbpfVmResult<()> {
    let mut ctx = cpi_ctx.borrow_mut();

    for acct in caller_accounts {
        // Read current lamports from caller's memory
        let lamports = memory.read_u64(acct.lamports_addr)?;

        // Read current data from caller's memory
        let data = memory
            .read_bytes(acct.data_addr, acct.data_len as usize)?
            .to_vec();

        let owner = acct.owner;
        let executable = acct.is_executable;
        let rent_epoch = acct.rent_epoch;

        // Update or insert into AccountStore
        if let Some(account) = ctx.accounts.get_mut(&acct.pubkey) {
            account.lamports = lamports;
            account.data = data;
            account.owner = owner;
            account.executable = executable;
            account.rent_epoch = rent_epoch;
        } else {
            ctx.accounts.insert(
                acct.pubkey,
                Account {
                    lamports,
                    data,
                    owner,
                    executable,
                    rent_epoch,
                },
            );
        }
    }

    Ok(())
}

/// Sync account changes back to caller's memory after CPI
pub fn sync_accounts_to_caller(
    cpi_ctx: &CpiContext,
    memory: &mut Memory,
    caller_accounts: &[CallerAccountInfo],
) -> SbpfVmResult<()> {
    let ctx = cpi_ctx.borrow();

    for acct in caller_accounts {
        if !acct.is_writable {
            continue;
        }

        if let Some(state) = ctx.accounts.get(&acct.pubkey) {
            // Write updated lamports
            memory.write_u64(acct.lamports_addr, state.lamports)?;

            // Write updated data (if size matches)
            if state.data.len() == acct.data_len as usize {
                memory.write_bytes(acct.data_addr, &state.data)?;
            } else {
                eprintln!(
                    "CPI warning: data length mismatch for {} (caller {}, updated {})",
                    acct.pubkey,
                    acct.data_len,
                    state.data.len()
                );
            }
        }
    }

    Ok(())
}

/// Execute a CPI call
pub fn execute_cpi(
    cpi_ctx: &CpiContext,
    costs: &ExecutionCost,
    clock: &Clock,
    rent: &Rent,
    epoch_schedule: &EpochSchedule,
    instruction: &CpiInstruction,
    caller_accounts: &[CallerAccountInfo],
    derived_signers: &[Pubkey],
    caller_memory: &mut Memory,
    compute: &ComputeMeter,
    remaining_compute: u64,
) -> SbpfVmResult<u64> {
    // Get callee program ELF
    let callee_elf = {
        let ctx = cpi_ctx.borrow();
        ctx.program_registry
            .get(&instruction.program_id)
            .map(|b| b.to_vec())
    };

    let callee_elf = callee_elf
        .ok_or_else(|| SbpfVmError::ProgramNotFound(instruction.program_id.to_string()))?;

    // Disassemble and run callee program in VM
    let program = Program::from_bytes(&callee_elf)
        .map_err(|e| SbpfVmError::SyscallError(format!("Failed to parse callee ELF: {:?}", e)))?;

    let entrypoint = program.get_entrypoint_offset().unwrap_or(0);
    let (mut callee_instructions, rodata_section) = program
        .to_ixs(false)
        .map_err(|e| SbpfVmError::SyscallError(format!("Failed to disassemble callee: {:?}", e)))?;

    let rodata_bytes = rodata_section
        .as_ref()
        .map(|s| s.data.clone())
        .unwrap_or_default();

    if let Some(ref section) = rodata_section {
        let elf_rodata_base = section.base_address;
        let elf_rodata_end = elf_rodata_base + section.data.len() as u64;

        for ix in &mut callee_instructions {
            if ix.opcode == Opcode::Lddw {
                if let Some(Either::Right(Number::Int(imm))) = &ix.imm {
                    let addr = *imm as u64;
                    if addr >= elf_rodata_base && addr < elf_rodata_end {
                        let offset = addr - elf_rodata_base;
                        let vm_addr = Memory::RODATA_START + offset;
                        ix.imm = Some(Either::Right(Number::Int(vm_addr as i64)));
                    }
                }
            }
        }
    }

    // Prepare accounts for serialization
    let ctx = cpi_ctx.borrow();
    let mut serialization_accounts = Vec::new();

    for meta in &instruction.accounts {
        let account = ctx.accounts.get(&meta.pubkey).cloned().unwrap_or(Account {
            lamports: 0,
            data: Vec::new(),
            owner: Pubkey::default(),
            executable: false,
            rent_epoch: 0,
        });

        // Check if this is a PDA signer
        let is_signer = meta.is_signer || derived_signers.contains(&meta.pubkey);

        serialization_accounts.push((meta.pubkey, account, is_signer, meta.is_writable));
    }
    drop(ctx);

    // Serialize input for callee
    let callee_input = serialize_input(
        &serialization_accounts,
        &instruction.data,
        &instruction.program_id,
    );

    // Increment stack height
    cpi_ctx.push();

    // Create callee syscall handler (shares CpiContext)
    let callee_handler = DebuggerSyscallHandler {
        cpi_ctx: cpi_ctx.clone(),
        current_program_id: instruction.program_id,
        costs: costs.clone(),
        compute_meter: compute.clone(),
        clock: clock.clone(),
        rent: rent.clone(),
        epoch_schedule: epoch_schedule.clone(),
    };

    // Create callee VM
    let config = SbpfVmConfig {
        max_call_depth: 64,
        compute_unit_limit: remaining_compute,
        stack_size: Memory::DEFAULT_STACK_SIZE,
        heap_size: Memory::DEFAULT_HEAP_SIZE,
    };

    let mut callee_vm = SbpfVm::new_with_config(
        callee_instructions,
        callee_input,
        rodata_bytes,
        callee_handler,
        config,
    );
    callee_vm.compute_meter = compute.clone();
    callee_vm.set_entrypoint(entrypoint as usize);

    println!("  Executing callee at entrypoint {}", entrypoint);

    // Run callee to completion
    let result = callee_vm.run();

    // Decrement stack height
    cpi_ctx.pop();

    // Handle result
    match result {
        Ok(()) => {
            let exit_code = callee_vm.exit_code.unwrap_or(0);

            if exit_code != 0 {
                println!("  Callee returned error: {}", exit_code);
                return Ok(exit_code);
            }

            // Update AccountStore from callee's output
            update_accounts_after_cpi(cpi_ctx, &callee_vm.memory, &instruction.accounts)?;

            // Sync changes back to caller's memory
            sync_accounts_to_caller(cpi_ctx, caller_memory, caller_accounts)?;

            println!("  CPI completed successfully");
            Ok(0)
        }
        Err(e) => {
            println!("  CPI failed: {:?}", e);
            Err(e)
        }
    }
}

/// Update AccountStore from callee's output memory
pub fn update_accounts_after_cpi(
    cpi_ctx: &CpiContext,
    callee_memory: &Memory,
    account_metas: &[CpiAccountMeta],
) -> SbpfVmResult<()> {
    let input_bytes = callee_memory.read_bytes(Memory::INPUT_START, callee_memory.input.len())?;

    let (deserialized, _instruction_data, _program_id) =
        deserialize_input(input_bytes, account_metas.len())
            .map_err(|e| SbpfVmError::SyscallError(e.to_string()))?;

    let mut ctx = cpi_ctx.borrow_mut();

    for (pubkey, lamports, data) in deserialized {
        // Find the corresponding meta to check if writable
        let is_writable = account_metas
            .iter()
            .find(|m| m.pubkey == pubkey)
            .map(|m| m.is_writable)
            .unwrap_or(false);

        if !is_writable {
            continue;
        }

        if let Some(state) = ctx.accounts.get_mut(&pubkey) {
            state.lamports = lamports;
            state.data = data;
        }
    }

    Ok(())
}
