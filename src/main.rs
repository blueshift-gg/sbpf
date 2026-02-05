pub mod commands;
use {
    anyhow::Error,
    clap::{Args, Parser, Subcommand},
    commands::{build, clean, debug, deploy, disassemble, init, test},
};

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Create a new project scaffold")]
    Init(InitArgs),
    #[command(about = "Compile into a Solana program executable")]
    Build(BuildArgs),
    #[command(about = "Build and deploy the program")]
    Deploy(DeployArgs),
    #[command(about = "Test deployed program")]
    Test,
    #[command(about = "Build, deploy and test a program")]
    E2E(DeployArgs),
    #[command(about = "Clean up build and deploy artifacts")]
    Clean,
    #[command(about = "Disassemble a Solana program executable")]
    Disassemble(DisassembleArgs),
    #[command(about = "Debug a program")]
    Debug(DebugArgs),
}

#[derive(Args)]
pub struct InitArgs {
    name: Option<String>,
    #[arg(
        short,
        long = "ts-tests",
        help = "Initialize with TypeScript tests instead of Mollusk Rust tests"
    )]
    ts_tests: bool,
}

#[derive(Args)]
struct BuildArgs {
    #[arg(short = 'g', long, help = "Include debug information")]
    debug: bool,
    #[arg(short = 's', long = "static-syscalls", help = "Use static syscalls")]
    static_syscalls: bool,
}

#[derive(Args)]
struct DeployArgs {
    name: Option<String>,
    url: Option<String>,
}

#[derive(Args)]
struct LinkArgs {
    source: Option<String>,
}

#[derive(Args)]
struct DisassembleArgs {
    filename: String,
    #[arg(short, long)]
    debug: bool,
}

#[derive(Args)]
pub struct DebugArgs {
    #[arg(long, conflicts_with = "elf", help = "Path to assembly file")]
    asm: Option<String>,
    #[arg(long, conflicts_with = "asm", help = "Path to elf file")]
    elf: Option<String>,
    #[arg(long, default_value = "", help = "Input hex")]
    input: String,
    #[arg(long, help = "Program ID")]
    program_id: Option<String>,
    #[arg(long, default_value = "1400000", help = "Compute unit limit")]
    compute_unit_limit: u64,
    #[arg(long, default_value = "4096", help = "Stack size")]
    stack_size: usize,
    #[arg(long, default_value = "32768", help = "Heap size")]
    heap_size: usize,
    #[arg(long, value_name = "PROGRAM_ID:PATH", help = "Additional program elfs")]
    program: Vec<String>,
    #[arg(long, help = "Run in adapter mode")]
    adapter: bool,
}

fn main() -> Result<(), Error> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Init(args) => init(args.name.clone(), args.ts_tests),
        Commands::Build(args) => build(args.debug, args.static_syscalls),
        Commands::Deploy(args) => deploy(args.name.clone(), args.url.clone()),
        Commands::Test => test(),
        Commands::E2E(args) => {
            build(false, false)?; // E2E uses release build
            deploy(args.name.clone(), args.url.clone())?;
            test()
        }
        Commands::Clean => clean(),
        Commands::Disassemble(args) => disassemble(args.filename.clone(), args.debug),
        Commands::Debug(args) => debug(args),
    }
}
