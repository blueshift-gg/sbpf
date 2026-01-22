pub mod commands;
use {
    anyhow::Error,
    clap::{Args, Parser, Subcommand, ValueEnum},
    commands::{build, clean, deploy, disassemble, init, test},
    sbpf_assembler::SbpfArch,
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

#[derive(Clone, Copy, ValueEnum)]
pub enum ArchArg {
    V0,
    V3,
}

impl From<ArchArg> for SbpfArch {
    fn from(arg: ArchArg) -> Self {
        match arg {
            ArchArg::V0 => SbpfArch::V0,
            ArchArg::V3 => SbpfArch::V3,
        }
    }
}

#[derive(Args)]
struct BuildArgs {
    #[arg(short = 'g', long, help = "Include debug information")]
    debug: bool,
    #[arg(
        short = 'a',
        long,
        default_value = "v0",
        help = "Target architecture (v0 or v3)"
    )]
    arch: ArchArg,
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

fn main() -> Result<(), Error> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Init(args) => init(args.name.clone(), args.ts_tests),
        Commands::Build(args) => build(args.debug, args.arch.into()),
        Commands::Deploy(args) => deploy(args.name.clone(), args.url.clone()),
        Commands::Test => test(),
        Commands::E2E(args) => {
            build(false, SbpfArch::V0)?; // E2E uses release build
            deploy(args.name.clone(), args.url.clone())?;
            test()
        }
        Commands::Clean => clean(),
        Commands::Disassemble(args) => disassemble(args.filename.clone(), args.debug),
    }
}
