use {
    crate::DebugArgs,
    anyhow::Result,
    sbpf_debugger::{
        adapter::run_adapter_loop,
        repl::Repl,
        runner::{load_session_from_asm, load_session_from_elf, parse_input},
    },
    sbpf_vm::vm::SbpfVmConfig,
};

pub fn debug(args: &DebugArgs) -> Result<()> {
    let input_bytes = parse_input(&args.input)?;
    let config = SbpfVmConfig {
        compute_unit_limit: args.compute_unit_limit,
        stack_size: args.stack_size,
        heap_size: args.heap_size,
        ..SbpfVmConfig::default()
    };

    let program_id = args.program_id.as_deref();

    let session = match (&args.asm, &args.elf) {
        (Some(asm_path), None) => {
            load_session_from_asm(asm_path.as_str(), input_bytes, config, program_id)?
        }
        (None, Some(elf_path)) => {
            load_session_from_elf(elf_path.as_str(), input_bytes, config, program_id)?
        }
        _ => {
            anyhow::bail!("Provide exactly one of --asm or --elf");
        }
    };

    if args.adapter {
        let mut debugger = session.debugger;
        run_adapter_loop(&mut debugger);
    } else {
        let mut repl = Repl::new(session);
        repl.start();
    }

    Ok(())
}
