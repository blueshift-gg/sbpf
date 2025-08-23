pub mod commands;
pub mod config;

use anyhow::Error;
use clap::{Args, Parser, Subcommand};
use commands::{build, clean, deploy, init, test};
use std::process::Command;

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
    #[command(about = "Run a script defined in configuration")]  
    Script(ScriptArgs),
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

#[derive(Args)]
struct ScriptArgs {
    #[arg(help = "Name of the script to run")]
    name: String,
    #[arg(help = "More arguments to pass to the script")]
    args: Vec<String>,
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
        // use arg to specify if use light build
        Commands::E2E(args) => {
            build()?;
            deploy(args.name.clone(), args.url.clone())?;
            test()
        }
        Commands::Clean => clean(),
        Commands::Config(args) => handle_config(args),  
        Commands::Script(args) => handle_script(args),
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
                    println!("  project.name, project.version");
                    println!("  deploy.cluster, deploy.wallet");
                    println!("  script.<name>");
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
        
        "deploy.cluster" => {
            if ["localhost", "devnet", "testnet", "mainnet"].contains(&value) ||
               value.starts_with("http") {
                config.deploy.cluster = value.to_string();
            } else {
                return Err(Error::msg("deploy.cluster must be 'localhost', 'devnet', 'testnet', 'mainnet', a URL, or an environment variable"));
            }
        }

        "deploy.program_id" => config.deploy.program_id = Some(value.to_string()),

        key if key.starts_with("scripts.") => {
            let script_name = key.strip_prefix("scripts.").unwrap();
            config.set_script(script_name.to_string(), value.to_string());
        }
        
        _ => return Err(Error::msg(format!("Unknown configuration key: {}", key))),
    }
    
    Ok(())
}

fn handle_script(args: &ScriptArgs) -> Result<(), Error> {
    let config = config::SbpfConfig::load().map_err(|e| {
        Error::msg(format!(
            "Cannot run script: No configuration found.\n{}\n\n💡 Run 'sbpf config init' to create a configuration file.",
            e
        ))
    })?;

    let script_command = config.scripts.get_script(&args.name).ok_or_else(|| {
        let available_scripts = config.list_scripts();
        if available_scripts.is_empty() {
            Error::msg(format!(
                "Script '{}' not found. No scripts are defined in your configuration.\n\n💡 Add scripts to your sbpf.toml:\n[scripts]\n{} = \"your command here\"",
                args.name, args.name
            ))
        } else {
            Error::msg(format!(
                "Script '{}' not found.\n\nAvailable scripts:\n{}\n\n💡 Add to your sbpf.toml:\n[scripts]\n{} = \"your command here\"",
                args.name,
                available_scripts.iter().map(|s| format!("  • {}", s)).collect::<Vec<_>>().join("\n"),
                args.name
            ))
        }
    })?;

    println!("🔧 Running script '{}': {}", args.name, script_command);
    run_script_command(script_command, &args.args)
}

fn run_script_command(command: &str, additional_args: &[String]) -> Result<(), Error> {
    let parts = shell_parse(command);
    if parts.is_empty() {
        return Err(Error::msg("Empty script command"));
    }

    let program = &parts[0];
    let mut args = parts[1..].to_vec();
    
    args.extend(additional_args.iter().cloned());

    let status = Command::new(program)
        .args(&args)
        .status()
        .map_err(|e| Error::msg(format!("Failed to execute script: {} ({})", command, e)))?;

    if !status.success() {
        return Err(Error::msg(format!(
            "Script '{}' failed with exit code: {}",
            command,
            status.code().unwrap_or(-1)
        )));
    }

    println!("✅ Script completed successfully");
    Ok(())
}

fn shell_parse(command: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut escape_next = false;

    for ch in command.chars() {
        if escape_next {
            current.push(ch);
            escape_next = false;
        } else if ch == '\\' {
            escape_next = true;
        } else if ch == '"' {
            in_quotes = !in_quotes;
        } else if ch.is_whitespace() && !in_quotes {
            if !current.is_empty() {
                parts.push(current);
                current = String::new();
            }
        } else {
            current.push(ch);
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }

    parts
}