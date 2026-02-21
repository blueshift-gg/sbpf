use {
    anyhow::Result,
    clap::Args,
    sbpf_debugger::{
        adapter::run_adapter_loop,
        input::parse_input,
        repl::Repl,
        runner::{load_session_from_asm, load_session_from_elf},
    },
    sbpf_vm::vm::SbpfVmConfig,
};

#[derive(Args)]
pub struct DebugArgs {
    #[arg(long, conflicts_with = "elf", help = "Path to assembly file")]
    asm: Option<String>,
    #[arg(long, conflicts_with = "asm", help = "Path to elf file")]
    elf: Option<String>,
    #[arg(long, default_value = "", help = "Input JSON file or JSON string")]
    input: String,
    #[arg(long, default_value = "1400000", help = "Compute unit limit")]
    compute_unit_limit: u64,
    #[arg(long, default_value = "64", help = "Maximum call depth")]
    max_call_depth: usize,
    #[arg(long, default_value = "32768", help = "Heap size")]
    heap_size: usize,
    #[arg(long, help = "Run in adapter mode")]
    adapter: bool,
}

pub fn debug(args: DebugArgs) -> Result<()> {
    let (input_bytes, program_id) = parse_input(&args.input)?;
    let config = SbpfVmConfig {
        max_call_depth: args.max_call_depth,
        compute_unit_limit: args.compute_unit_limit,
        heap_size: args.heap_size,
    };

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
