pub mod commands;
pub mod config;

use anyhow::Error;
use clap::{Args, Parser, Subcommand};
use commands::{build, clean, deploy, init, test};

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
    #[command(about = "Initialize or manage configuration")]  
    Config(ConfigArgs),
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
struct ConfigArgs {
    #[command(subcommand)]
    action: ConfigAction,
}

#[derive(Subcommand)]
enum ConfigAction {
    #[command(about = "Show current configuration")]
    Show,
    #[command(about = "Initialize default configuration")]
    Init,
    #[command(about = "Set a configuration value")]
    Set { 
        key: String, 
        value: String 
    },
}

fn main() -> Result<(), Error> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Init(args) => init(args.name.clone(), args.ts_tests),
        Commands::Build => build(),
        Commands::Deploy(args) => deploy(args.name.clone(), args.url.clone()),
        Commands::Test => test(),
        Commands::E2E(args) => {
            build()?;
            deploy(args.name.clone(), args.url.clone())?;
            test()
        }
        Commands::Clean => clean(),
        Commands::Config(args) => handle_config(args),  
    }
}


fn handle_config(args: &ConfigArgs) -> Result<(), Error> {
    use config::SbpfConfig;
    
    match &args.action {
        ConfigAction::Show => {
            match SbpfConfig::load() {
                Ok(config) => {
                    let toml_content = toml::to_string_pretty(&config)?;
                    println!("Current configuration:");
                    println!("{}", toml_content);
                }
                Err(e) => {
                    println!("❌ Configuration not found");
                    println!("   Error: {}", e);
                }
            }
        }
        ConfigAction::Init => {
            let current_dir = std::env::current_dir()?;
            let project_name = current_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("sbpf-project");
            
            let config = SbpfConfig::default_for_project(project_name);
            config.save(".")?;
        }
        ConfigAction::Set { key, value } => {
            let mut config = match SbpfConfig::load() {
                Ok(config) => config,
                Err(_) => {
                    println!("❌ No configuration file found. Run 'sbpf config init' first.");
                    return Ok(());
                }
            };
            
            match set_config_value(&mut config, key, value) {
                Ok(()) => {
                    config.save(".")?;
                    println!("✅ Configuration updated: {} = {}", key, value);
                }
                Err(e) => {
                    println!("❌ Failed to set configuration: {}", e);
                    println!("Valid keys include:");
                    println!("  project.name, project.version, project.description");
                    println!("  build.optimization, build.target");
                    println!("  deploy.cluster, deploy.program_id");
                    println!("  test.framework");
                }
            }
        }
    }
    
    Ok(())
}


fn set_config_value(config: &mut config::SbpfConfig, key: &str, value: &str) -> Result<(), Error> {
    match key {
        "project.name" => config.project.name = value.to_string(),
        "project.version" => config.project.version = value.to_string(),
        "project.description" => config.project.description = Some(value.to_string()),
        
        "build.optimization" => {
            if value == "debug" || value == "release" {
                config.build.optimization = value.to_string();
            } else {
                return Err(Error::msg("build.optimization must be 'debug' or 'release'"));
            }
        }
        "build.target" => config.build.target = value.to_string(),
        
        "deploy.cluster" => {
            if ["localhost", "devnet", "testnet", "mainnet"].contains(&value) {
                config.deploy.cluster = value.to_string();
            } else {
                return Err(Error::msg("deploy.cluster must be one of: localhost, devnet, testnet, mainnet"));
            }
        }
        "deploy.program_id" => config.deploy.program_id = Some(value.to_string()),
        
        "test.framework" => {
            if value == "mollusk" || value == "typescript" {
                config.test.framework = value.to_string();
            } else {
                return Err(Error::msg("test.framework must be 'mollusk' or 'typescript'"));
            }
        }
        
        _ => return Err(Error::msg(format!("Unknown configuration key: {}", key))),
    }
    
    Ok(())
}