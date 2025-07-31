pub mod commands;
use anyhow::Error;
use clap::{Args, Parser, Subcommand};
use commands::{build, light_build, clean, deploy, init, test};

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
    #[command(about = "Compile without any platform tools")]
    LightBuild,
    #[command(about = "Build and deploy the program")]
    Deploy(DeployArgs),
    #[command(about = "Test deployed program")]
    Test,
    #[command(about = "Build, deploy and test a program")]
    E2E(DeployArgs),
    #[command(about = "Clean up build and deploy artifacts")]
    Clean,
    #[command(about = "Analyze assembly program")]
    Analyze(AnalyzeArgs),
    #[command(about = "Validate assembly syntax")]
    Validate(ValidateArgs),
    #[command(about = "Show compilation metrics")]
    Metrics(MetricsArgs),
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
struct AnalyzeArgs {
    #[arg(help = "Assembly file to analyze")]
    file: Option<String>,
}

#[derive(Args)]
struct ValidateArgs {
    #[arg(help = "Assembly file to validate")]
    file: Option<String>,
}

#[derive(Args)]
struct MetricsArgs {
    #[arg(help = "Assembly file to analyze metrics")]
    file: Option<String>,
}

fn main() -> Result<(), Error> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Init(args) => init(args.name.clone(), args.ts_tests),
        Commands::Build => build(),
        Commands::LightBuild => light_build(),
        Commands::Deploy(args) => deploy(args.name.clone(), args.url.clone()),
        Commands::Test => test(),
        // use arg to specify if use light build
        Commands::E2E(args) => {
            build()?;
            deploy(args.name.clone(), args.url.clone())?;
            test()
        }
        Commands::Clean => clean(),
        Commands::Analyze(args) => analyze_assembly(args.file.clone()),
        Commands::Validate(args) => validate_assembly(args.file.clone()),
        Commands::Metrics(args) => show_metrics(args.file.clone()),
    }
}

fn analyze_assembly(file: Option<String>) -> Result<(), Error> {
    let file_path = file.unwrap_or_else(|| "src".to_string());
    
    if std::path::Path::new(&file_path).is_file() {
        // Analyze single file
        let source = std::fs::read_to_string(&file_path)?;
        let profiler = sbpf_assembler::analyze_program(&source)?;
        
        println!("📊 Assembly Analysis for {}", file_path);
        println!("═══════════════════════════════════════");
        println!("🔍 Instruction counts:");
        for (opcode, count) in &profiler.instruction_counts {
            println!("  • {}: {}", opcode, count);
        }
        
        println!("\n📊 Register usage:");
        for (reg, count) in &profiler.register_usage {
            println!("  • r{}: {}", reg, count);
        }
        
        println!("\n📈 Statistics:");
        println!("  • Total instructions: {}", profiler.instruction_counts.values().sum::<usize>());
        println!("  • Unique instructions: {}", profiler.instruction_counts.len());
        println!("  • Registers used: {}/10", profiler.register_usage.len());
        
    } else {
        // Analyze all files in directory
        let src_path = std::path::Path::new(&file_path);
        if src_path.exists() && src_path.is_dir() {
            for entry in std::fs::read_dir(src_path)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |ext| ext == "s") {
                    println!("\n📊 Analyzing {}", path.display());
                    let source = std::fs::read_to_string(&path)?;
                    let _profiler = sbpf_assembler::analyze_program(&source)?;
                }
            }
        }
    }
    
    Ok(())
}

fn validate_assembly(file: Option<String>) -> Result<(), Error> {
    let file_path = file.unwrap_or_else(|| "src".to_string());
    
    if std::path::Path::new(&file_path).is_file() {
        // Validate single file
        let source = std::fs::read_to_string(&file_path)?;
        let validator = sbpf_assembler::AssemblyValidator::default();
        
        match validator.validate(&source) {
            Ok(()) => {
                println!("✅ Assembly validation passed for {}", file_path);
            }
            Err(errors) => {
                println!("❌ Assembly validation failed for {}", file_path);
                for error in errors {
                    println!("  • {}", error);
                }
                return Err(Error::msg("Validation failed"));
            }
        }
        
    } else {
        // Validate all files in directory
        let src_path = std::path::Path::new(&file_path);
        if src_path.exists() && src_path.is_dir() {
            let mut all_valid = true;
            for entry in std::fs::read_dir(src_path)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |ext| ext == "s") {
                    let source = std::fs::read_to_string(&path)?;
                    let validator = sbpf_assembler::AssemblyValidator::default();
                    
                    match validator.validate(&source) {
                        Ok(()) => {
                            println!("✅ {}", path.display());
                        }
                        Err(errors) => {
                            println!("❌ {}", path.display());
                            for error in errors {
                                println!("  • {}", error);
                            }
                            all_valid = false;
                        }
                    }
                }
            }
            
            if !all_valid {
                return Err(Error::msg("Some files failed validation"));
            }
        }
    }
    
    Ok(())
}

fn show_metrics(file: Option<String>) -> Result<(), Error> {
    let file_path = file.unwrap_or_else(|| "src".to_string());
    
    if std::path::Path::new(&file_path).is_file() {
        // Show metrics for single file
        let source = std::fs::read_to_string(&file_path)?;
        let (_, metrics) = sbpf_assembler::assemble_with_validation(&source, "deploy")?;
        metrics.print_report();
        
    } else {
        // Show metrics for all files in directory
        let src_path = std::path::Path::new(&file_path);
        if src_path.exists() && src_path.is_dir() {
            let mut total_metrics = sbpf_assembler::CompilationMetrics::new();
            
            for entry in std::fs::read_dir(src_path)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |ext| ext == "s") {
                    println!("\n📊 Metrics for {}", path.display());
                    let source = std::fs::read_to_string(&path)?;
                    let (_, metrics) = sbpf_assembler::assemble_with_validation(&source, "deploy")?;
                    
                    // Accumulate metrics
                    total_metrics.instruction_count += metrics.instruction_count;
                    total_metrics.bytecode_size += metrics.bytecode_size;
                    total_metrics.memory_usage += metrics.memory_usage;
                    total_metrics.lex_time += metrics.lex_time;
                    total_metrics.parse_time += metrics.parse_time;
                    total_metrics.codegen_time += metrics.codegen_time;
                }
            }
            
            println!("\n📊 Total Metrics:");
            total_metrics.print_report();
        }
    }
    
    Ok(())
}
