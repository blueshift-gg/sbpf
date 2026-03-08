use {
    crate::{
        config::{ExecutionCost, RuntimeConfig, SysvarContext},
        cpi,
        errors::{RuntimeError, RuntimeResult},
        serialize,
        syscalls::RuntimeSyscallHandler,
    },
    base64::{Engine, engine::general_purpose::STANDARD as BASE64},
    either::Either,
    sbpf_common::{execute::Vm, inst_param::Number, instruction::Instruction, opcode::Opcode},
    sbpf_disassembler::program::Program,
    sbpf_vm::{
        compute::ComputeMeter,
        memory::Memory,
        vm::{CallFrame, SbpfVm, SbpfVmConfig},
    },
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction as SolanaInstruction},
    std::{cell::RefCell, collections::HashMap, rc::Rc},
};

pub type LogCollector = Rc<RefCell<Vec<String>>>;

pub enum ElfSource {
    Path(String),
    Bytes(Vec<u8>),
}

impl From<&str> for ElfSource {
    fn from(path: &str) -> Self {
        ElfSource::Path(path.to_string())
    }
}

impl From<&[u8]> for ElfSource {
    fn from(bytes: &[u8]) -> Self {
        ElfSource::Bytes(bytes.to_vec())
    }
}

impl From<Vec<u8>> for ElfSource {
    fn from(bytes: Vec<u8>) -> Self {
        ElfSource::Bytes(bytes)
    }
}

pub struct ExecutionResult {
    pub exit_code: Option<u64>,
    pub compute_units_consumed: u64,
    pub logs: Vec<String>,
}

pub struct Runtime {
    program_id: Address,
    instructions: Vec<Instruction>,
    rodata: Vec<u8>,
    entrypoint: usize,
    programs: HashMap<Address, Vec<u8>>,
    config: RuntimeConfig,
    sysvars: SysvarContext,
    vm: Option<SbpfVm<RuntimeSyscallHandler>>,
    accounts: HashMap<Address, Account>,
    account_metas: Vec<AccountMeta>,
    log_collector: LogCollector,
}

impl Runtime {
    pub fn new(
        program_id: Address,
        elf: impl Into<ElfSource>,
        config: RuntimeConfig,
    ) -> RuntimeResult<Self> {
        let elf_bytes = match elf.into() {
            ElfSource::Path(path) => std::fs::read(&path)?,
            ElfSource::Bytes(bytes) => bytes,
        };

        let program = Program::from_bytes(&elf_bytes)
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

        Ok(Self {
            program_id,
            instructions,
            rodata,
            entrypoint: entrypoint as usize,
            programs: HashMap::new(),
            config,
            sysvars: SysvarContext::default(),
            vm: None,
            accounts: HashMap::new(),
            account_metas: Vec::new(),
            log_collector: Rc::new(RefCell::new(Vec::new())),
        })
    }

    pub fn add_program(&mut self, program_id: &Address, elf: impl Into<ElfSource>) {
        let elf_bytes = match elf.into() {
            ElfSource::Path(path) => std::fs::read(&path).expect("Failed to read ELF"),
            ElfSource::Bytes(bytes) => bytes,
        };
        self.programs.insert(*program_id, elf_bytes);
    }

    fn setup_vm(
        &mut self,
        instruction: &SolanaInstruction,
        accounts: &[(Address, Account)],
    ) -> RuntimeResult<()> {
        self.accounts = accounts.iter().cloned().collect();
        self.account_metas = instruction.accounts.clone();

        let input = serialize::serialize_parameters(
            &self.accounts,
            &self.account_metas,
            &instruction.data,
            &self.program_id,
        )?;

        let vm_config = SbpfVmConfig {
            compute_unit_limit: self.config.compute_budget,
            max_call_depth: self.config.max_call_depth,
            heap_size: self.config.heap_size,
        };

        let handler = RuntimeSyscallHandler::new(
            ExecutionCost::default(),
            self.program_id,
            self.sysvars.clone(),
            self.log_collector.clone(),
        );

        let mut vm = SbpfVm::new_with_config(
            self.instructions.clone(),
            input,
            self.rodata.clone(),
            handler,
            vm_config,
        );
        vm.compute_meter = ComputeMeter::new(self.config.compute_budget);
        vm.set_entrypoint(self.entrypoint);

        self.vm = Some(vm);
        Ok(())
    }

    fn sync_accounts(&mut self) {
        if let Some(ref vm) = self.vm {
            serialize::deserialize_parameters(
                &mut self.accounts,
                &self.account_metas,
                &vm.memory.input,
            );
        }
    }

    pub fn run(
        &mut self,
        instruction: &SolanaInstruction,
        accounts: &[(Address, Account)],
    ) -> RuntimeResult<ExecutionResult> {
        self.log_collector.borrow_mut().clear();
        self.setup_vm(instruction, accounts)?;

        self.log_collector
            .borrow_mut()
            .push(format!("Program {} invoke [1]", self.program_id));

        loop {
            let vm = self.vm.as_mut().unwrap();
            if let Err(e) = vm.step() {
                self.log_collector
                    .borrow_mut()
                    .push(format!("Program failed: {}", e));
                return Err(e.into());
            }

            if let Some(request) = vm.syscall_handler.pending_cpi.take() {
                if let Err(e) = self.handle_cpi(request) {
                    self.log_collector
                        .borrow_mut()
                        .push(format!("Program failed: {}", e));
                    return Err(e);
                }
                continue;
            }

            if vm.halted {
                break;
            }
        }

        self.sync_accounts();

        let vm = self.vm.as_ref().unwrap();
        let consumed = vm.compute_meter.get_consumed();
        let exit_code = vm.exit_code;

        if let Some(ref return_data) = vm.syscall_handler.return_data
            && !return_data.1.is_empty()
        {
            self.log_collector.borrow_mut().push(format!(
                "Program return: {} {}",
                return_data.0,
                BASE64.encode(&return_data.1)
            ));
        }

        self.log_collector.borrow_mut().push(format!(
            "Program {} consumed {} of {} compute units",
            self.program_id, consumed, self.config.compute_budget
        ));

        if exit_code.unwrap_or(0) == 0 {
            self.log_collector
                .borrow_mut()
                .push(format!("Program {} success", self.program_id));
        } else {
            self.log_collector.borrow_mut().push(format!(
                "Program {} failed: exit code {}",
                self.program_id,
                exit_code.unwrap_or(0)
            ));
        }

        let logs = self.log_collector.borrow().clone();

        Ok(ExecutionResult {
            exit_code,
            compute_units_consumed: consumed,
            logs,
        })
    }

    pub fn prepare(
        &mut self,
        instruction: &SolanaInstruction,
        accounts: &[(Address, Account)],
    ) -> RuntimeResult<()> {
        self.log_collector.borrow_mut().clear();
        self.setup_vm(instruction, accounts)?;
        self.log_collector
            .borrow_mut()
            .push(format!("Program {} invoke [1]", self.program_id));
        Ok(())
    }

    pub fn step(&mut self) -> RuntimeResult<()> {
        let vm = self.vm.as_mut().ok_or(RuntimeError::VmNotPrepared)?;
        if vm.halted {
            return Ok(());
        }
        if let Err(e) = vm.step() {
            self.log_collector
                .borrow_mut()
                .push(format!("Program failed: {}", e));
            return Err(e.into());
        }

        if let Some(request) = vm.syscall_handler.pending_cpi.take()
            && let Err(e) = self.handle_cpi(request)
        {
            self.log_collector
                .borrow_mut()
                .push(format!("Program failed: {}", e));
            return Err(e);
        }

        let vm_ref = self.vm.as_ref().unwrap();
        if vm_ref.halted {
            self.sync_accounts();

            let vm = self.vm.as_ref().unwrap();
            let consumed = vm.compute_meter.get_consumed();
            let exit_code = vm.exit_code;

            if let Some(ref return_data) = vm.syscall_handler.return_data
                && !return_data.1.is_empty()
            {
                self.log_collector.borrow_mut().push(format!(
                    "Program return: {} {}",
                    return_data.0,
                    BASE64.encode(&return_data.1)
                ));
            }

            self.log_collector.borrow_mut().push(format!(
                "Program {} consumed {} of {} compute units",
                self.program_id, consumed, self.config.compute_budget
            ));

            if exit_code.unwrap_or(0) == 0 {
                self.log_collector
                    .borrow_mut()
                    .push(format!("Program {} success", self.program_id));
            } else {
                self.log_collector.borrow_mut().push(format!(
                    "Program {} failed: exit code {}",
                    self.program_id,
                    exit_code.unwrap_or(0)
                ));
            }
        }

        Ok(())
    }

    fn handle_cpi(&mut self, request: cpi::request::CpiRequest) -> RuntimeResult<()> {
        let vm = self.vm.as_ref().unwrap();
        let compute_remaining = self.config.compute_budget - vm.compute_meter.get_consumed();

        // Sync latest account state from caller VM memory into account store.
        cpi::sync::sync_from_caller(&vm.memory, &request.caller_accounts, &mut self.accounts)?;

        let caller_accounts = request.caller_accounts;
        let cpi_request = cpi::request::CpiRequest {
            program_id: request.program_id,
            accounts: request.accounts,
            data: request.data,
            caller_accounts: Vec::new(),
            signers: request.signers,
        };

        let mut ctx = cpi::CpiContext {
            request: cpi_request,
            programs: &self.programs,
            accounts: &mut self.accounts,
            config: &self.config,
            sysvars: &self.sysvars,
            compute_remaining,
            cpi_depth: 1,
            caller_account_metas: &self.account_metas,
            log_collector: &self.log_collector,
        };

        let output = cpi::execute_cpi(&mut ctx)?;

        let vm = self.vm.as_mut().unwrap();
        vm.compute_meter.consume(output.compute_consumed)?;
        vm.syscall_handler.return_data = output.return_data;

        if output.exit_code != 0 {
            return Err(RuntimeError::VmError(
                sbpf_vm::errors::SbpfVmError::SyscallError(format!(
                    "CPI callee returned error: {}",
                    output.exit_code
                )),
            ));
        }

        // Sync updated accounts back to caller VM memory.
        let vm = self.vm.as_mut().unwrap();
        cpi::sync::sync_to_caller(&mut vm.memory, &caller_accounts, &self.accounts)?;

        Ok(())
    }

    pub fn get_pc(&self) -> usize {
        self.vm.as_ref().map(|vm| vm.pc).unwrap_or(0)
    }

    pub fn get_registers(&self) -> Option<&[u64; 11]> {
        self.vm.as_ref().map(|vm| &vm.registers)
    }

    pub fn current_program_id(&self) -> &Address {
        &self.program_id
    }

    pub fn is_halted(&self) -> bool {
        self.vm.as_ref().map(|vm| vm.halted).unwrap_or(false)
    }

    pub fn exit_code(&self) -> Option<u64> {
        self.vm.as_ref().and_then(|vm| vm.exit_code)
    }

    pub fn compute_units_consumed(&self) -> u64 {
        self.vm
            .as_ref()
            .map(|vm| vm.compute_meter.get_consumed())
            .unwrap_or(0)
    }

    pub fn get_account(&self, pubkey: &Address) -> Option<Account> {
        self.accounts.get(pubkey).cloned()
    }

    pub fn get_register(&self, idx: usize) -> Option<u64> {
        self.vm
            .as_ref()
            .and_then(|vm| vm.registers.get(idx).copied())
    }

    pub fn set_register(&mut self, idx: usize, value: u64) -> RuntimeResult<()> {
        let vm = self.vm.as_mut().ok_or(RuntimeError::VmNotPrepared)?;
        if idx >= vm.registers.len() {
            return Err(RuntimeError::RegisterOutOfRange(idx));
        }
        vm.set_register(idx, value);
        Ok(())
    }

    pub fn read_memory(&self, addr: u64, size: usize) -> Option<Vec<u8>> {
        self.vm
            .as_ref()
            .and_then(|vm| vm.memory.read_bytes(addr, size).ok().map(|s| s.to_vec()))
    }

    pub fn get_instruction(&self) -> Option<&Instruction> {
        let vm = self.vm.as_ref()?;
        vm.program.get(vm.pc)
    }

    pub fn get_program(&self) -> &[Instruction] {
        &self.instructions
    }

    pub fn get_call_stack(&self) -> Option<&[CallFrame]> {
        self.vm.as_ref().map(|vm| vm.call_stack.as_slice())
    }

    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    pub fn sysvars(&self) -> &SysvarContext {
        &self.sysvars
    }

    pub fn sysvars_mut(&mut self) -> &mut SysvarContext {
        &mut self.sysvars
    }

    pub fn log_collector(&self) -> &LogCollector {
        &self.log_collector
    }

    pub fn drain_logs(&self) -> Vec<String> {
        self.log_collector.borrow_mut().drain(..).collect()
    }
}
