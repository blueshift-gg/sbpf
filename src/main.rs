pub mod commands;
use {
    anyhow::Error,
    clap::{Parser, Subcommand},
    commands::{
        build::{BuildArgs, build},
        clean::clean,
        deploy::{DeployArgs, deploy},
        disassemble::{DisassembleArgs, disassemble},
        init::{InitArgs, init},
        test::test,
    },
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

fn main() -> Result<(), Error> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init(args) => init(args),
        Commands::Build(args) => build(args),
        Commands::Deploy(args) => deploy(args),
        Commands::Test => test(),
        Commands::E2E(args) => {
            build(BuildArgs::default())?;
            deploy(args)?;
            test()
        }
        Commands::Clean => clean(),
        Commands::Disassemble(args) => disassemble(args),
    }
}
