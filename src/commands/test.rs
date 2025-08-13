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
            
            println!("📋 No sbpf.toml found, using default test command");
            SbpfConfig::default_for_project(project_name)
        }
    };

    if let Some(test_script) = config.scripts.get_script("test") {
        println!("🧪 Running test script: {}", test_script);
        return run_script_command(test_script);
    }
    
    println!("🧪 Running default test command: cargo test-sbf");
    run_default_test()
}

fn run_script_command(command: &str) -> Result<(), Error> {
    let status = Command::new("sh")
        .arg("-c")
        .arg(command)
        .status()?;

    if !status.success() {
        return Err(Error::new(io::Error::new(
            io::ErrorKind::Other,
            "❌ Test script failed",
        )));
    }
    
    println!("✅ Tests completed successfully!");
    Ok(())
}

fn run_default_test() -> Result<(), Error> {
    let status = Command::new("cargo")
        .arg("test-sbf")
        .arg("--")
        .arg("--nocapture")
        .env("RUST_BACKTRACE", "1")
        .status()?;

    if !status.success() {
        return Err(Error::new(io::Error::new(
            io::ErrorKind::Other,
            "❌ Default tests failed",
        )));
    }
    
    println!("✅ Tests completed successfully!");
    Ok(())
}