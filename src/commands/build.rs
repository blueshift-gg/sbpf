use anyhow::{Error, Result};
use dirs::home_dir;
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use std::fs;
use std::fs::create_dir_all;
use std::io;
use std::path::Path;
use std::process::Command;
use std::time::Instant;
use indicatif::{ProgressBar, ProgressStyle};

use crate::commands::common::{SolanaConfig, DEFAULT_LINKER};

pub fn build() -> Result<()> {
    let progress = ProgressBar::new(6);
    progress.set_style(ProgressStyle::default_spinner()
        .template("{spinner:.green} [{elapsed_precise}] {msg}")
        .unwrap());

    // Step 1: Check Solana installation
    progress.set_message("Checking Solana installation...");
    let home_dir = home_dir().expect("❌ Could not find $HOME directory");
    let config_path = home_dir.join(".config/solana/install/config.yml");

    if !Path::new(&config_path).exists() {
        progress.finish_with_message("❌ Solana config not found");
        return Err(Error::msg("❌ Solana config not found. Please install the Solana CLI:\n\nhttps://docs.anza.xyz/cli/install"));
    }
    progress.inc(1);

    // Step 2: Read Solana config
    progress.set_message("Reading Solana configuration...");
    let config_content = fs::read_to_string(config_path)?;
    let solana_config: SolanaConfig = serde_yaml::from_str(&config_content)?;
    progress.inc(1);

    // Step 3: Check platform tools
    progress.set_message("Checking platform tools...");
    let platform_tools = [solana_config.active_release_dir.clone(), "/bin/platform-tools-sdk/sbf/dependencies/platform-tools".to_owned()].concat();
    let llvm_dir = [platform_tools.clone(), "/llvm".to_owned()].concat();
    let clang = [llvm_dir.clone(), "/bin/clang".to_owned()].concat();
    let ld = [llvm_dir.clone(), "/bin/ld.lld".to_owned()].concat();

    if !Path::new(&llvm_dir).exists() {
        progress.finish_with_message("❌ Platform tools not found");
        return Err(Error::msg(format!("❌ Solana platform-tools not found. To manually install, please download the latest release here: \n\nhttps://github.com/anza-xyz/platform-tools/releases\n\nThen unzip to this directory and try again:\n\n{}", &platform_tools)));
    }
    progress.inc(1);

    // Step 4: Setup directories
    progress.set_message("Setting up build directories...");
    let src = "src";
    let out = ".sbpf";
    let deploy = "deploy";
    let arch = "-target";
    let arch_target = "sbf";

    create_dir_all(out)?;
    create_dir_all(deploy)?;
    progress.inc(1);

    // Step 5: Compile assembly with new assembler
    progress.set_message("Compiling assembly...");
    let start_time = Instant::now();
    
    // Use the new assembler with metrics
    let source_files = find_assembly_files(src)?;
    let mut total_metrics = sbpf_assembler::CompilationMetrics::new();
    
    for file in &source_files {
        let file_path = format!("{}/{}", src, file);
        let source_code = fs::read_to_string(&file_path)?;
        
        // Use new assembler with validation and metrics
        let (bytecode, metrics) = sbpf_assembler::assemble_with_validation(&source_code, deploy)?;
        
        // Accumulate metrics
        total_metrics.instruction_count += metrics.instruction_count;
        total_metrics.bytecode_size += metrics.bytecode_size;
        total_metrics.memory_usage += metrics.memory_usage;
        total_metrics.lex_time += metrics.lex_time;
        total_metrics.parse_time += metrics.parse_time;
        total_metrics.codegen_time += metrics.codegen_time;
        
        // Write bytecode to file
        let output_file = format!("{}/{}.so", deploy, file.replace(".s", ""));
        fs::write(output_file, bytecode)?;
    }
    
    let compile_time = start_time.elapsed();
    progress.inc(1);

    // Step 6: Generate keypair if needed
    progress.set_message("Generating keypair...");
    let project_name = get_project_name()?;
    let keypair_path = format!("{}/{}-keypair.json", deploy, project_name);
    
    if !Path::new(&keypair_path).exists() {
        let mut rng = OsRng;
        let keypair = SigningKey::generate(&mut rng);
        fs::write(&keypair_path, serde_json::json!(keypair.to_keypair_bytes()[..]).to_string())?;
    }
    progress.inc(1);

    // Finish with success message and metrics
    progress.finish_with_message("✅ Build completed successfully!");
    
    // Print compilation metrics
    total_metrics.total_time = compile_time;
    total_metrics.print_report();
    
    println!("🎉 Build completed in {:?}", compile_time);
    println!("📦 Generated {} files", source_files.len());
    
    Ok(())
}

fn find_assembly_files(src_dir: &str) -> Result<Vec<String>> {
    let mut files = Vec::new();
    let src_path = Path::new(src_dir);
    
    if src_path.exists() {
        for entry in fs::read_dir(src_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "s") {
                if let Some(file_name) = path.file_name() {
                    files.push(file_name.to_string_lossy().to_string());
                }
            }
        }
    }
    
    Ok(files)
}

fn get_project_name() -> Result<String> {
    let current_dir = std::env::current_dir()?;
    let dir_name = current_dir.file_name()
        .ok_or_else(|| Error::msg("Could not get directory name"))?
        .to_string_lossy();
    
    Ok(dir_name.to_string())
}

// Legacy function for backward compatibility
fn compile_assembly(
    clang: &str,
    arch: &str,
    arch_target: &str,
    out: &str,
    src: &str,
    filename: &str,
) -> Result<()> {
    let output_file = format!("{}/{}.o", out, filename);
    let input_file = format!("{}/{}/{}.s", src, filename, filename);
    let status = Command::new(clang)
        .args([
            arch,
            arch_target,
            "-c",
            "-o",
            &output_file,
            &input_file,
        ])
        .status()?;

    if !status.success() {
        eprintln!("Failed to compile assembly for {}", filename);
        return Err(Error::new(io::Error::new(
            io::ErrorKind::Other,
            "Compilation failed",
        )));
    }
    Ok(())
}

fn build_shared_object(ld: &str, filename: &str) -> Result<()> {
    let default_linker = ".sbpf/linker.ld".to_string();
    let output_file = format!("deploy/{}.so", filename);
    let input_file = format!(".sbpf/{}.o", filename);
    let mut linker_file = format!("src/{}.ld", filename);
    
    // Check if a custom linker file exists
    if !Path::new(&linker_file).exists() {
        if !Path::new(&default_linker).exists() {
            fs::create_dir(".sbpf").unwrap_or(());
            fs::write(&default_linker, DEFAULT_LINKER)?;
        }
        linker_file = default_linker;
    }

    let status = Command::new(ld)
        .arg("-shared")
        .arg("-o")
        .arg(&output_file)
        .arg(&input_file)
        .arg(&linker_file)
        .status()?;

    if !status.success() {
        eprintln!("Failed to build shared object for {}", filename);
        return Err(Error::new(io::Error::new(
            io::ErrorKind::Other,
            "Linking failed",
        )));
    }
    Ok(())
}

fn has_keypair_file(dir: &Path) -> bool {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                if let Some(file_name) = entry.file_name().to_str() {
                    if file_name.ends_with("-keypair.json") {
                        return true;
                    }
                }
            }
        }
    }
    false
}
