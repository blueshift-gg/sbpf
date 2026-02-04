use {
    crate::DebugArgs,
    anyhow::Result,
    sbpf_debugger::{
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

    let mut session = match (&args.asm, &args.elf) {
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

    // Load additional programs for CPI (if provided).
    for program_spec in &args.program {
        let (program_id, path) = parse_program_spec(program_spec)?;
        session.load_program(&program_id, &path)?;
        println!("Loaded program {} from {}", program_id, path);
    }

    let mut repl = Repl::new(session);
    repl.start();

    Ok(())
}

// Parse a program spec in the format "PROGRAM_ID:PATH"
fn parse_program_spec(spec: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = spec.splitn(2, ':').collect();
    if parts.len() != 2 {
        anyhow::bail!(
            "Invalid program format: '{}'. Expected PROGRAM_ID:PATH",
            spec
        );
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}
