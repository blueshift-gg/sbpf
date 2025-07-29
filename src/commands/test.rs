use anyhow::{Error, Result};
use std::{fs, io, path::Path, process::Command};
use crate::config::SbpfConfig;  

pub fn test() -> Result<(), Error> {
    println!("🧪 Running tests");

    let deploy_dir = Path::new("deploy");

    fn has_so_files(dir: &Path) -> bool {
        if dir.exists() && dir.is_dir() {
            match fs::read_dir(dir) {
                Ok(entries) => entries.filter_map(Result::ok).any(|entry| {
                    entry
                        .path()
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext == "so")
                        .unwrap_or(false)
                }),
                Err(_) => false,
            }
        } else {
            false
        }
    }

    if !has_so_files(deploy_dir) {
        println!("🔄 No .so files found in 'deploy' directory. Running build...");
        crate::commands::build::build()?;
    }

    let config = match SbpfConfig::load() {
        Ok(config) => {
            println!("📋 Using test configuration from sbpf.toml");
            config
        }
        Err(_) => {
            
            let current_dir = std::env::current_dir()?;
            let project_name = current_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("sbpf-project");
            
            println!("📋 No sbpf.toml found, using auto-detection for test framework");
            SbpfConfig::default_for_project(project_name)
        }
    };

    
    let has_cargo = Path::new("Cargo.toml").exists();
    let has_package_json = Path::new("package.json").exists();
    
    
    let preferred_framework = &config.test.framework;
    
    match (has_cargo, has_package_json, preferred_framework.as_str()) {
        
        (_, true, "typescript") => {
            println!("🧪 Running TypeScript tests (configured preference)");
            run_typescript_tests(&config)?;
        }
         
        (true, _, "mollusk") => {
            println!("🧪 Running Rust tests with Mollusk (configured preference)");
            run_rust_tests(&config)?;
        }
       
        (true, _, _) => {
            println!("🧪 Running Rust tests with Mollusk (auto-detected)");
            run_rust_tests(&config)?;
        }
        
        (false, true, _) => {
            println!("🧪 Running TypeScript tests (auto-detected)");
            run_typescript_tests(&config)?;
        }
       
        (false, false, _) => {
            return Err(Error::new(io::Error::new(
                io::ErrorKind::NotFound,
                "❌ No test configuration found. Expected either Cargo.toml or package.json",
            )));
        }
    }

    println!("✅ Tests completed successfully!");
    Ok(())
}

fn run_rust_tests(config: &SbpfConfig) -> Result<(), Error> {
    let mut cmd = Command::new("cargo");
    cmd.arg("test-sbf")
       .arg("--")
       .arg("--nocapture")
       .env("RUST_BACKTRACE", "1");
    
    
    for arg in &config.test.validator_args {
        cmd.arg(arg);
    }
    
    let output = cmd.status()?;

    if !output.success() {
        eprintln!("Failed to run Rust tests");
        return Err(Error::new(io::Error::new(
            io::ErrorKind::Other,
            "❌ Rust tests failed",
        )));
    }
    
    Ok(())
}

fn run_typescript_tests(config: &SbpfConfig) -> Result<(), Error> {
    crate::commands::deploy(None, None)?;

    let mut cmd = Command::new("yarn");
    cmd.arg("test");
    
    if !config.test.validator_args.is_empty() {
        let validator_args = config.test.validator_args.join(" ");
        cmd.env("VALIDATOR_ARGS", validator_args);
    }

    let status = cmd.status()?;

    if !status.success() {
        eprintln!("Failed to run TypeScript tests");
        return Err(Error::new(io::Error::new(
            io::ErrorKind::Other,
            "❌ TypeScript tests failed",
        )));
    }
    
    Ok(())
}