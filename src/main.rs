pub mod commands;
pub mod config;

use anyhow::Error;
use clap::{Args, Parser, Subcommand};
use std::process::Command;
use commands::{build, light_build, clean, deploy, init, test};

use crate::config::SbpfConfig;

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
    #[command(about = "Compile without any platform tools")]
    LightBuild,
    #[command(about = "Build and deploy the program")]
    Deploy(DeployArgs),
    #[command(about = "Test deployed program")]
    Test(TestArgs),
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
    #[arg(help = "Name of the project to create")]
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
    #[arg(long, help = "Override build mode (full/light)")]
    mode: Option<String>,
    #[arg(long, help = "Override optimization (debug/release)")]
    optimization: Option<String>,
}

#[derive(Args)]
struct DeployArgs {
    #[arg(short, long, help = "Name of specific program to deploy")]
    program: Option<String>,
    #[arg(long, help = "Override cluster from config")]
    cluster: Option<String>,
}

#[derive(Args)]
struct ConfigArgs {
    #[command(subcommand)]
    action: ConfigAction,
}

#[derive(Args)]
struct TestArgs {
    #[arg(long, help = "Override test command")]
    command: Option<String>,
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
    #[command(about = "Show configuration manual and examples")]
    Manual,
}

fn main() -> Result<(), Error> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Init(args) => init(args.name.clone(), args.ts_tests),
        Commands::Build(args) => {
            if let Ok(config) = config::SbpfConfig::load() {
                if let Some(script_command) = config.get_builtin_override("build") {
                    println!("🔧 Running custom build script: {}", script_command);
                    return run_script_command(script_command, &[]);
                }
            }
            build_with_mode(args)
        },
        Commands::LightBuild => light_build(),
        Commands::Deploy(args) => {
            if let Ok(config) = config::SbpfConfig::load() {
                if let Some(script_command) = config.get_builtin_override("deploy") {
                    println!("🚀 Running deploy script: {}", script_command);
                    return run_script_command(script_command, &[]);
                }
            }
            deploy_with_args(args)
        },
        Commands::Test(args) => {
            if let Ok(config) = config::SbpfConfig::load() {
                if let Some(script_command) = config.get_builtin_override("test") {
                    println!("🧪 Running custom test script: {}", script_command);
                    return run_script_command(script_command, &[]);
                }
            }
            test_with_args(args)
        },
        Commands::E2E(args) => {
            // use arg to specify if use light build
            build_with_mode(&BuildArgs { mode: None, optimization: None })?;
            deploy_with_args(args)?;
            test_with_args(&TestArgs { command: None })
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
                    let warnings = config.security_warnings();
                    if !warnings.is_empty() {
                        println!("\n 📄 Security Notes:");
                        for warning in warnings {
                            println!("   {}", warning.message());
                        }
                    }
                }
                Err(e) => {
                    let current_dir = std::env::current_dir()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|_| "<unknown>".to_string());
                    
                    println!("❌ Cannot modify configuration: No sbpf.toml found");
                    println!("   Current directory: {}", current_dir);
                    println!("   Error: {}", e);
                    println!();
                    println!("💡 First create a configuration file:");
                    println!("   sbpf config init");
                    return Ok(());
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
                    println!("✔️ Configuration updated: {} = {}", key, value);
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
        ConfigAction::Manual => {
            show_config_manual();
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
        
        "build.mode" => {
            if value == "full" || value == "light" {
                config.build.mode = value.to_string();
            } else {
                return Err(Error::msg("build.mode must be 'full' or 'light'"));
            }
        }
        
        "deploy.cluster" => {
            if ["localhost", "devnet", "testnet", "mainnet"].contains(&value) ||
               value.starts_with("http") || config::SbpfConfig::has_env_vars(value) {
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

    println!("✔️ Script completed successfully");
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

fn build_with_mode(args: &BuildArgs) -> Result<(), Error> {
    let config = config::SbpfConfig::load_or_default("sbpf-project");

    let build_mode = args.mode.as_deref().unwrap_or(&config.build.mode);
    let optimization = args.optimization.as_deref().unwrap_or(&config.build.optimization);
    
    if let Some(ref cli_mode) = args.mode {
        println!("🔧 CLI override: build mode = {}", cli_mode);
    }
    if let Some(ref cli_opt) = args.optimization {
        println!("🔧 CLI override: optimization = {}", cli_opt);
    }

    match build_mode {
        "light" => {
            println!("⚡ Using light build mode (sbpf-assembler)");
            light_build_with_config(&config)
        }
        "full" => {
            println!("⭕ Using full build mode (Solana toolchain)");
            let opt_flag = match optimization {
                "debug" => "--debug",
                "release" => "--release",
                _ => {
                    println!("⚠️ Invalid optimization '{}', defaulting to 'debug'", optimization);
                    "--debug"
                }
            };
            println!("🏳️ Applying optimization flag: {}", opt_flag);
            build()
        }
        invalid_mode => {
            Err(Error::msg(format!(
                "Invalid build mode: '{}'\n\n💡 Valid modes:\n  • 'full' - Uses Solana toolchain (clang/ld)\n  • 'light' - Uses built-in sbpf-assembler\n\nUpdate your configuration:\n  sbpf config set build.mode light",
                invalid_mode
            )))
        }
    }
}


fn deploy_with_args(args: &DeployArgs) -> Result<(), Error> {
    if let Some(ref cli_cluster) = args.cluster {
        let resolved_url = SbpfConfig::resolve_cluster_to_url(cli_cluster);
        let warnings = SbpfConfig::check_security_warnings_for_cluster(&resolved_url);
        if !warnings.is_empty() {
            println!("📄 Security Notes:");
            for warning in warnings {
                println!("   {}", warning.message());
            }
            println!();
        }
        deploy(args.program.clone(), Some(resolved_url))
    } else {
        deploy(args.program.clone(), None)
    }
}

fn test_with_args(args: &TestArgs) -> Result<(), Error> {
    if let Some(ref cli_command) = args.command {
        println!("🧪 CLI override: test command = {}", cli_command);
        return run_script_command(cli_command, &[]);
    }
    
    test()
}

fn light_build_with_config(config: &config::SbpfConfig) -> Result<(), Error> {
    println!("📋 Light build using project configuration");
    println!("   Project: {}", config.project.name);
    println!("   Optimization: {}", config.build.optimization);

    light_build()
}

fn show_config_manual() {
    println!("📚 sbpf Configuration Manual");
    println!("--------------------------------"); 
    println!();
    
    println!("📁 Configuration File: sbpf.toml");
    println!("Place this file in your project root directory.");
    println!();
    
    println!("Example Configuration:");
    println!("```toml");
    println!("[project]");
    println!("name = \"my-solana-program\"");
    println!("version = \"0.1.0\"");
    println!("description = \"My awesome Solana program\"");
    println!();
    println!("[build]");
    println!("mode = \"full\"           # 'full' (Solana toolchain) or 'light' (sbpf-assembler)");
    println!("optimization = \"debug\"   # 'debug' or 'release'");
    println!("target = \"sbf\"");
    println!("flags = [\"--strip\"]       # Additional compiler flags");
    println!();
    println!("[deploy]");
    println!("cluster = \"devnet\"        # 'localhost', 'devnet', 'testnet', 'mainnet', or URL");
    println!("# cluster = \"${{SOLANA_RPC_URL:-devnet}}\"  # Environment variable with fallback");
    println!("program_id = \"7xKXt...\"   # Optional custom program ID");
    println!();
    println!("[test]");
    println!("validator_args = [\"--reset\", \"--quiet\"]");
    println!();
    println!("[scripts]");
    println!("# Override built-in commands");
    println!("test = \"cargo test --verbose\"");
    println!("build = \"echo 'Custom build' && cargo build\"");
    println!("# Custom scripts");
    println!("lint = \"cargo clippy -- -D warnings\"");
    println!("deploy-prod = \"sbpf deploy --cluster mainnet\"");
    println!("```");
    println!();
    
    println!("🖥️ Environment Variables:");
    println!("You can use environment variables in any string value:");
    println!("  ${{VAR}}           - Use environment variable VAR");
    println!("  ${{VAR:-default}}  - Use VAR, or 'default' if not set");
    println!();
    
    println!("📋 Commands:");
    println!("  sbpf config init                    - Create default config");
    println!("  sbpf config show                    - View current config");
    println!("  sbpf config set key value           - Modify current config");
    println!("  eg. sbpf config set build.mode light - Set build mode");
    println!();
    
    println!("📄 Script Usage:");
    println!("  sbpf test            - Runs built-in test (or scripts.test override)");
    println!("  sbpf script lint     - Runs custom script");
    println!("  sbpf script deploy-prod arg1 arg2  - Runs script with arguments");
    println!();
    
    println!("⏭️ Tips:");
    println!("  • Use environment variables for sensitive info");
    println!("  • Keep wallet paths in standard locations (eg. ~/.config/solana/id.json)");
}