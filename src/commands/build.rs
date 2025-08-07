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

use crate::commands::common::{SolanaConfig, DEFAULT_LINKER};
use crate::config::SbpfConfig;  

pub fn build() -> Result<()> {
    // Load configuration or use defaults
    let config = match SbpfConfig::load() {
        Ok(config) => {
            if let Err(e) = config.validate() {
                return Err(Error::msg(format!("Configuration validation failed: {}", e)));
            }
            config
        }
        Err(_) => {
            // If there is no config file, then use regular behavior with current directory name
            let current_dir = std::env::current_dir()?;
            let project_name = current_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("sbpf-project");
            
            println!("📋 No sbpf.toml found, using default configuration for '{}'", project_name);
            println!("   Build mode: 'full', Optimization: 'debug'");
            println!("   💡 Run 'sbpf config init' to create a configuration file for this project");
            SbpfConfig::default_for_project(project_name)
        }
    };

    // Now construct the path to the config file
    let home_dir = home_dir().expect("❌ Could not find $HOME directory");
    // Solana Config path
    let config_path = home_dir.join(".config/solana/install/config.yml");

    if !Path::new(&config_path).exists() {
        return Err(Error::msg("❌ Solana config not found. Please install the Solana CLI:\n\nhttps://docs.anza.xyz/cli/install"));
    }

    // Read the file contents
    let config_content = fs::read_to_string(config_path)?;

    // Parse the YAML file
    let solana_config: SolanaConfig = serde_yaml::from_str(&config_content)?;

    // Solana SDK and toolchain paths
    let platform_tools = [solana_config.active_release_dir.clone(), "/bin/platform-tools-sdk/sbf/dependencies/platform-tools".to_owned()].concat();
    let llvm_dir = [platform_tools.clone(), "/llvm".to_owned()].concat();
    let clang = [llvm_dir.clone(), "/bin/clang".to_owned()].concat();
    let ld = [llvm_dir.clone(), "/bin/ld.lld".to_owned()].concat();

    // Now we check whether platform tools exist, if not we return this error message
    if !Path::new(&llvm_dir).exists() {
        return Err(Error::msg(format!("❌ Solana platform-tools not found. To manually install, please download the latest release here: \n\nhttps://github.com/anza-xyz/platform-tools/releases\n\nThen unzip to this directory and try again:\n\n{}", &platform_tools)));
    }

    // Set src/out directory and compiler flags
    let src = "src";
    let out = ".sbpf";
    let deploy = "deploy";
    
    // Use configuration for target architecture
    let arch = "-target";
    let arch_target = &config.build.target; 

    // Then we create necessary directories
    create_dir_all(out)?;
    create_dir_all(deploy)?;

    // Function to compile assembly
    fn compile_assembly(
        clang: &str,
        arch: &str,
        arch_target: &str,
        out: &str,
        src: &str,
        filename: &str,
        config: &SbpfConfig,  
    ) -> Result<()> {
        let output_file = format!("{}/{}.o", out, filename);
        let input_file = format!("{}/{}/{}.s", src, filename, filename);
        
        // Get compiler arguments from configuration
        let mut args = vec![arch, arch_target, "-c", "-o", &output_file, &input_file];
        
        // Including optimization flags based on config
        let additional_args: Vec<String>;
        if config.is_release_build() {
            additional_args = vec!["-O3".to_string(), "--strip".to_string()];
            for arg in &additional_args {
                args.push(arg);
            }
        }
        
        // Including custom flags from config
        let custom_args: Vec<String> = config.build.flags.clone();
        for arg in &custom_args {
            args.push(arg);
        }
        
        println!("🔧 Compiling with: {} {}", clang, args.join(" "));
        
        let status = Command::new(clang)
            .args(&args)
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

    // Function to build shared object
    fn build_shared_object(ld: &str, filename: &str, config: &SbpfConfig) -> Result<()> {
        let default_linker = ".sbpf/linker.ld".to_string();
        let output_file = format!("deploy/{}.so", filename);
        let input_file = format!(".sbpf/{}.o", filename);
        
        // Here we check for a custom linker script in config first
        let mut linker_file = if let Some(custom_linker) = &config.build.linker_script {
            custom_linker.to_string_lossy().to_string()
        } else {
            format!("src/{}/{}.ld", filename, filename)
        };
        
        // Then we check if the specified linker file exists
        if !Path::new(&linker_file).exists() {
            if !Path::new(&default_linker).exists() {
                fs::create_dir(".sbpf").unwrap_or(());
                fs::write(&default_linker, DEFAULT_LINKER)?;
            }
            linker_file = default_linker;
        };

        let status = Command::new(ld)
            .arg("-shared")
            .arg("-z")
            .arg("notext")
            .arg("--image-base")
            .arg("0x100000000")
            .arg("-T")
            .arg(linker_file)
            .arg("-o")
            .arg(&output_file)
            .arg(&input_file)
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

    // Function to check if keypair file exists.
    fn has_keypair_file(dir: &Path) -> bool {
        if dir.exists() && dir.is_dir() {
            match fs::read_dir(dir) {
                Ok(entries) => entries.filter_map(Result::ok).any(|entry| {
                    entry
                        .path()
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(|name| name.ends_with("-keypair.json"))
                        .unwrap_or(false)
                }),
                Err(_) => false,
            }
        } else {
            false
        }
    }

    // Check if keypair file exists. If not, create one.
    let deploy_path = Path::new(deploy);
    if !has_keypair_file(deploy_path) {
        // We're using the config project name instead of directory name
        let project_name = &config.project.name;
        let mut rng = OsRng;
        fs::write(
            deploy_path.join(format!("{}-keypair.json", project_name)),
            serde_json::json!(SigningKey::generate(&mut rng).to_keypair_bytes()[..]).to_string(),
        )?;
        println!("🔑 Generated keypair for project '{}'", project_name);
    }

    // Then process directories 
    let src_path = Path::new(src);
    for entry in src_path.read_dir()? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(subdir) = path.file_name().and_then(|name| name.to_str()) {
                let asm_file = format!("{}/{}/{}.s", src, subdir, subdir);
                if Path::new(&asm_file).exists() {
                    println!("🔄 Building \"{}\"", subdir);
                    let start = Instant::now();
                    compile_assembly(&clang, arch, arch_target, out, src, subdir, &config)?;
                    build_shared_object(&ld, subdir, &config)?;
                    let duration = start.elapsed();
                    println!(
                        "✔️ \"{}\" built successfully in {}ms! ({} mode)",
                        subdir,
                        duration.as_micros() as f64 / 1000.0,
                        if config.is_release_build() { "release" } else { "debug" }
                    );
                }
            }
        }
    }

    Ok(())
}

