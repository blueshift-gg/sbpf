use std::io;
use std::path::Path;
use std::process::Command;

use anyhow::{Error, Result};
use crate::config::SbpfConfig;  

fn deploy_program(program_name: &str, url: &str, config: &SbpfConfig) -> Result<(), Error> {
    
    let program_id_file = if let Some(custom_id) = &config.deploy.program_id {
        format!("./deploy/{}-keypair.json", custom_id)
    } else {
        format!("./deploy/{}-keypair.json", program_name)
    };

    let program_file = format!("./deploy/{}.so", program_name);

    if Path::new(&program_file).exists() {
        println!("🔄 Deploying \"{}\" to {}", program_name, url);

        let mut cmd = Command::new("solana");
        cmd.arg("program")
           .arg("deploy")
           .arg(&program_file)
           .arg("--program-id")
           .arg(&program_id_file)
            .arg("--keypair")
           .arg(&config.deploy.wallet)
           .arg("-u")
           .arg(url);

        let status = cmd.status()?;

        if !status.success() {
            eprintln!("Failed to deploy program for {}", program_name);
            return Err(Error::new(io::Error::new(
                io::ErrorKind::Other,
                "❌ Deployment failed",
            )));
        }

        println!("✅ \"{}\" deployed successfully!", program_name);
    } else {
        eprintln!("Program file {} not found", program_file);
        return Err(Error::new(io::Error::new(
            io::ErrorKind::NotFound,
            "❌ Program file not found",
        )));
    }

    Ok(())
}

fn deploy_all_programs(url: &str, config: &SbpfConfig) -> Result<(), Error> {
    let deploy_path = Path::new("deploy");

    for entry in deploy_path.read_dir()? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("so") {
            if let Some(filename) = path.file_stem().and_then(|name| name.to_str()) {
                deploy_program(filename, url, config)?;
            }
        }
    }

    Ok(())
}

pub fn deploy(name: Option<String>, url: Option<String>) -> Result<(), Error> {
    
    let config = match SbpfConfig::load() {
        Ok(config) => {
            if url.is_none() { 
                println!("📋 Using deployment configuration from sbpf.toml");
            }
            config
        }
        Err(_) => {
            
            let current_dir = std::env::current_dir()?;
            let project_name = current_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("sbpf-project");
            
            println!("📋 No sbpf.toml found, using default deployment settings");
            println!("   💡 Run 'sbpf config init' to create a configuration file");
            SbpfConfig::default_for_project(project_name)
        }
    };

    
    let deployment_url = match url {
        Some(override_url) => {
            println!("🌐 Detected command-line cluster: {} (Now overriding config: {})", override_url, config.deploy.cluster);
            override_url
        }
        None => {
            let resolved_url = config.resolve_cluster_url();
            println!("🌐 Using cluster '{}' → {}", config.deploy.cluster, resolved_url);
            resolved_url
        }
    };

    if let Some(program_name) = name {
        
        deploy_program(&program_name, &deployment_url, &config)
    } else {
        
        deploy_all_programs(&deployment_url, &config)
    }
}