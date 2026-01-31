use anyhow::Result;
use sbpf_debugger::{
    repl::Repl,
    runner::{load_session_from_asm, load_session_from_elf, parse_input},
};
use sbpf_vm::{syscalls::MockSyscallHandler, vm::SbpfVmConfig};

use crate::DebugArgs;

pub fn debug(args: &DebugArgs) -> Result<()> {
    let input_bytes = parse_input(&args.input)?;
    let config = SbpfVmConfig {
        max_steps: args.max_steps,
        stack_size: args.stack_size,
        heap_size: args.heap_size,
        ..SbpfVmConfig::default()
    };

    let session = match (&args.asm, &args.elf) {
        (Some(asm_path), None) => load_session_from_asm(
            asm_path.as_str(),
            input_bytes,
            MockSyscallHandler::default(),
            config,
        )?,
        (None, Some(elf_path)) => load_session_from_elf(
            elf_path.as_str(),
            input_bytes,
            MockSyscallHandler::default(),
            config,
        )?,
        _ => {
            anyhow::bail!("Provide exactly one of --asm or --elf");
        }
    };

    let mut repl = Repl::new(session.debugger);
    repl.start();

    Ok(())
}
