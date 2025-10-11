pub mod commands;
use anyhow::Error;
use clap::{Args, Parser, Subcommand};
use commands::{build, clean, deploy, disassemble, init, test};

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
    Build,
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
    asm: bool,
}

fn main() -> Result<(), Error> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Init(args) => init(args.name.clone(), args.ts_tests),
        Commands::Build => build(),
        Commands::Deploy(args) => deploy(args.name.clone(), args.url.clone()),
        Commands::Test => test(),
        // use arg to specify if use light build
        Commands::E2E(args) => {
            build()?;
            deploy(args.name.clone(), args.url.clone())?;
            test()
        }
        Commands::Clean => clean(),
        Commands::Disassemble(args) => disassemble(args.filename.clone(), args.asm),
    }
}
