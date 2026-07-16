pub mod builtins;
pub mod request;
pub mod sync;
pub mod validate;

use {
    crate::{
        config::{ExecutionCost, RuntimeConfig, SysvarContext},
        elf::load_elf,
        errors::{RuntimeError, RuntimeResult},
        runtime::LogCollector,
        serialize,
        syscalls::RuntimeSyscallHandler,
    },
    base64::{Engine, engine::general_purpose::STANDARD as BASE64},
    request::CpiRequest,
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

pub type ReturnData = Option<(Address, Vec<u8>)>;

pub struct CpiOutput {
    pub exit_code: u64,
    pub return_data: ReturnData,
    pub compute_consumed: u64,
}

pub type CpiExecResult = RuntimeResult<CpiOutput>;

pub struct CpiContext<'a> {
    pub request: CpiRequest,
    pub programs: &'a HashMap<Address, Vec<u8>>,
    pub accounts: &'a mut HashMap<Address, Account>,
    pub config: &'a RuntimeConfig,
    pub sysvars: &'a SysvarContext,
    pub compute_remaining: u64,
    pub cpi_depth: usize,
    pub caller_account_metas: &'a [AccountMeta],
    pub log_collector: &'a LogCollector,
}

pub fn execute_cpi(ctx: &mut CpiContext) -> CpiExecResult {
    if ctx.cpi_depth >= ctx.config.max_cpi_depth {
        return Err(RuntimeError::CpiDepthExceeded(ctx.config.max_cpi_depth));
    }

    validate::check_privileges(&ctx.request, ctx.caller_account_metas)?;

    ctx.log_collector.borrow_mut().push(format!(
        "Program {} invoke [{}]",
        ctx.request.program_id,
        ctx.cpi_depth + 1
    ));

    if builtins::is_builtin(&ctx.request.program_id) {
        let mut all_signers = ctx.request.signers.clone();
        for meta in &ctx.request.accounts {
            if meta.is_signer && !all_signers.contains(&meta.pubkey) {
                all_signers.push(meta.pubkey);
            }
        }
        let consumed = builtins::execute_builtin(
            &ctx.request.program_id,
            ctx.accounts,
            &ctx.request,
            &all_signers,
            ctx.compute_remaining,
        )?;
        ctx.log_collector.borrow_mut().push(format!(
            "Program {} consumed {} of {} compute units",
            ctx.request.program_id, consumed, ctx.compute_remaining
        ));
        ctx.log_collector
            .borrow_mut()
            .push(format!("Program {} success", ctx.request.program_id));
        return Ok(CpiOutput {
            exit_code: 0,
            return_data: None,
            compute_consumed: consumed,
        });
    }

    execute_elf_cpi(ctx)
}

fn execute_elf_cpi(ctx: &mut CpiContext) -> CpiExecResult {
    let elf_bytes = ctx
        .programs
        .get(&ctx.request.program_id)
        .ok_or_else(|| RuntimeError::ProgramNotFound(ctx.request.program_id.to_string()))?;

    let (instructions, rodata, entrypoint) = load_elf(elf_bytes)?;

    let account_metas: Vec<AccountMeta> = ctx
        .request
        .accounts
        .iter()
        .map(|a| AccountMeta {
            pubkey: a.pubkey,
            is_signer: a.is_signer,
            is_writable: a.is_writable,
        })
        .collect();

    let (input, pre_lens, instruction_data_offset) = serialize::serialize_parameters(
        ctx.accounts,
        &account_metas,
        &ctx.request.data,
        &ctx.request.program_id,
    )?;

    let vm_config = SbpfVmConfig {
        compute_unit_limit: ctx.compute_remaining,
        max_call_depth: ctx.config.max_call_depth,
        heap_size: ctx.config.heap_size,
    };

    let handler = RuntimeSyscallHandler::new(
        ExecutionCost::default(),
        ctx.request.program_id,
        ctx.sysvars.clone(),
        ctx.log_collector.clone(),
    );

    let mut callee_vm = SbpfVm::new_with_config(instructions, input, rodata, handler, vm_config);
    callee_vm.compute_meter = ComputeMeter::new(ctx.compute_remaining);
    callee_vm.set_entrypoint(entrypoint);
    callee_vm.registers[2] = Memory::INPUT_START + instruction_data_offset as u64;

    loop {
        if let Err(e) = callee_vm.step() {
            return Err(e.into());
        }

        if let Some(nested_request) = callee_vm.syscall_handler.pending_cpi.take() {
            sync::sync_from_caller(
                &callee_vm.memory,
                &nested_request.caller_accounts,
                ctx.accounts,
            )?;

            let caller_accounts_for_sync = nested_request.caller_accounts;
            let nested_consumed = callee_vm.compute_meter.get_consumed();
            let nested_remaining = ctx.compute_remaining.saturating_sub(nested_consumed);

            let nested_cpi_request = CpiRequest {
                program_id: nested_request.program_id,
                accounts: nested_request.accounts,
                data: nested_request.data,
                caller_accounts: Vec::new(),
                signers: nested_request.signers,
            };

            let mut nested_ctx = CpiContext {
                request: nested_cpi_request,
                programs: ctx.programs,
                accounts: ctx.accounts,
                config: ctx.config,
                sysvars: ctx.sysvars,
                compute_remaining: nested_remaining,
                cpi_depth: ctx.cpi_depth + 1,
                caller_account_metas: &account_metas,
                log_collector: ctx.log_collector,
            };

            let nested_output = execute_cpi(&mut nested_ctx)?;

            callee_vm
                .compute_meter
                .consume(nested_output.compute_consumed)?;
            callee_vm.syscall_handler.return_data = nested_output.return_data;

            if nested_output.exit_code != 0 {
                let consumed = callee_vm.compute_meter.get_consumed();
                return Ok(CpiOutput {
                    exit_code: nested_output.exit_code,
                    return_data: callee_vm.syscall_handler.return_data.take(),
                    compute_consumed: consumed,
                });
            }

            sync::sync_to_caller(
                &mut callee_vm.memory,
                &caller_accounts_for_sync,
                ctx.accounts,
            )?;
        }

        if callee_vm.halted {
            break;
        }
    }

    let exit_code = callee_vm.exit_code.unwrap_or(0);
    let callee_return_data = callee_vm.syscall_handler.return_data.take();
    let consumed = callee_vm.compute_meter.get_consumed();

    if exit_code != 0 {
        ctx.log_collector.borrow_mut().push(format!(
            "Program {} consumed {} of {} compute units",
            ctx.request.program_id, consumed, ctx.compute_remaining
        ));
        ctx.log_collector.borrow_mut().push(format!(
            "Program {} failed: exit code {}",
            ctx.request.program_id, exit_code
        ));
        return Ok(CpiOutput {
            exit_code,
            return_data: callee_return_data,
            compute_consumed: consumed,
        });
    }

    serialize::deserialize_parameters(
        ctx.accounts,
        &account_metas,
        &callee_vm.memory.input,
        &pre_lens,
        &ctx.request.program_id,
    )?;

    if let Some((ref pid, ref data)) = callee_return_data
        && !data.is_empty()
    {
        ctx.log_collector.borrow_mut().push(format!(
            "Program return: {} {}",
            pid,
            BASE64.encode(data)
        ));
    }

    ctx.log_collector.borrow_mut().push(format!(
        "Program {} consumed {} of {} compute units",
        ctx.request.program_id, consumed, ctx.compute_remaining
    ));
    ctx.log_collector
        .borrow_mut()
        .push(format!("Program {} success", ctx.request.program_id));

    Ok(CpiOutput {
        exit_code: 0,
        return_data: callee_return_data,
        compute_consumed: consumed,
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::cpi::request::CpiAccountMeta,
        solana_system_interface::{instruction::SystemInstruction, program as system_program},
        std::{cell::RefCell, rc::Rc},
    };

    fn addr(seed: u8) -> Address {
        Address::new_from_array([seed; 32])
    }

    fn log_collector() -> LogCollector {
        Rc::new(RefCell::new(Vec::new()))
    }

    fn system_account(lamports: u64) -> Account {
        Account {
            lamports,
            data: vec![],
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        }
    }

    #[test]
    fn execute_cpi_program_not_found() {
        let config = RuntimeConfig::default();
        let sysvars = SysvarContext::default();
        let programs: HashMap<Address, Vec<u8>> = HashMap::new();
        let mut accounts: HashMap<Address, Account> = HashMap::new();
        let logs = log_collector();

        // Non-builtin program that was never registered via add_program.
        let request = CpiRequest {
            program_id: addr(9),
            accounts: Vec::new(),
            data: Vec::new(),
            caller_accounts: Vec::new(),
            signers: Vec::new(),
        };
        let mut ctx = CpiContext {
            request,
            programs: &programs,
            accounts: &mut accounts,
            config: &config,
            sysvars: &sysvars,
            compute_remaining: 200_000,
            cpi_depth: 0,
            caller_account_metas: &[],
            log_collector: &logs,
        };

        match execute_cpi(&mut ctx) {
            Err(RuntimeError::ProgramNotFound(_)) => {}
            Err(other) => panic!("expected ProgramNotFound, got {other:?}"),
            Ok(_) => panic!("expected ProgramNotFound error"),
        }
    }

    #[test]
    fn execute_cpi_depth_exceeded() {
        let config = RuntimeConfig::default();
        let sysvars = SysvarContext::default();
        let programs: HashMap<Address, Vec<u8>> = HashMap::new();
        let mut accounts: HashMap<Address, Account> = HashMap::new();
        let logs = log_collector();

        let request = CpiRequest {
            program_id: addr(9),
            accounts: Vec::new(),
            data: Vec::new(),
            caller_accounts: Vec::new(),
            signers: Vec::new(),
        };
        let mut ctx = CpiContext {
            request,
            programs: &programs,
            accounts: &mut accounts,
            config: &config,
            sysvars: &sysvars,
            compute_remaining: 200_000,
            cpi_depth: config.max_cpi_depth, // already at the limit
            caller_account_metas: &[],
            log_collector: &logs,
        };

        match execute_cpi(&mut ctx) {
            Err(RuntimeError::CpiDepthExceeded(_)) => {}
            Err(other) => panic!("expected CpiDepthExceeded, got {other:?}"),
            Ok(_) => panic!("expected CpiDepthExceeded error"),
        }
    }

    #[test]
    fn execute_cpi_dispatches_system_transfer_builtin() {
        let config = RuntimeConfig::default();
        let sysvars = SysvarContext::default();
        let programs: HashMap<Address, Vec<u8>> = HashMap::new();
        let logs = log_collector();

        let from = addr(1);
        let to = addr(2);
        let mut accounts =
            HashMap::from([(from, system_account(1_000_000)), (to, system_account(0))]);

        let request = CpiRequest {
            program_id: system_program::ID,
            accounts: vec![
                CpiAccountMeta {
                    pubkey: from,
                    is_signer: true,
                    is_writable: true,
                },
                CpiAccountMeta {
                    pubkey: to,
                    is_signer: false,
                    is_writable: true,
                },
            ],
            data: wincode::serialize(&SystemInstruction::Transfer { lamports: 400_000 }).unwrap(),
            caller_accounts: Vec::new(),
            signers: Vec::new(),
        };

        let caller_metas = vec![
            AccountMeta {
                pubkey: from,
                is_signer: true,
                is_writable: true,
            },
            AccountMeta {
                pubkey: to,
                is_signer: false,
                is_writable: true,
            },
        ];

        let mut ctx = CpiContext {
            request,
            programs: &programs,
            accounts: &mut accounts,
            config: &config,
            sysvars: &sysvars,
            compute_remaining: 200_000,
            cpi_depth: 0,
            caller_account_metas: &caller_metas,
            log_collector: &logs,
        };

        let output = execute_cpi(&mut ctx).unwrap();
        assert_eq!(output.exit_code, 0);
        assert_eq!(accounts[&from].lamports, 600_000);
        assert_eq!(accounts[&to].lamports, 400_000);
        assert!(logs.borrow().iter().any(|l| l.contains("invoke [1]")));
        assert!(logs.borrow().iter().any(|l| l.contains("success")));
    }
}
