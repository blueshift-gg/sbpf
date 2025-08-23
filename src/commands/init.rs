use super::common::{
    CARGO_TOML, GITIGNORE, PACKAGE_JSON, PROGRAM, README, RUST_TESTS, TSCONFIG, TS_TESTS,
};
use anyhow::{Error, Result};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use std::fs;
use std::io::{self, Write};
use std::process::Command;
use crate::config::SbpfConfig; 

pub fn init(name: Option<String>, ts_tests: bool) -> Result<(), Error> {
    let project_name = match name {
        Some(name) => name.clone(),
        None => loop {
            print!("What is the name of your project? ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim().to_string();

            if !input.is_empty() {
                break input.replace(' ', "-");
            } else {
                println!("Project name cannot be empty. Please enter a valid name.");
            }
        },
    };

    let current_dir = std::env::current_dir()?;
    let project_path = current_dir.join(&project_name);

    if !project_path.exists() {
        fs::create_dir_all(&project_path)?;
        fs::create_dir_all(project_path.join("src").join(&project_name))?;
        fs::create_dir_all(project_path.join("deploy"))?;

        fs::write(
            project_path.join("README.md"),
            README.replace("default_project_name", &project_name),
        )?;
        fs::write(project_path.join(".gitignore"), GITIGNORE)?;

        fs::write(
            project_path
                .join("src")
                .join(&project_name)
                .join(format!("{}.s", project_name)),
            PROGRAM,
        )?;

        let mut rng = OsRng;
        fs::write(
            project_path
                .join("deploy")
                .join(format!("{}-keypair.json", project_name)),
            serde_json::json!(SigningKey::generate(&mut rng).to_keypair_bytes()[..]).to_string(),
        )?;

        let mut config = SbpfConfig::default_for_project(&project_name);
        config.scripts.scripts = if ts_tests {
            let mut scripts_map = std::collections::HashMap::new();
            scripts_map.insert("test".to_string(), "yarn test".to_string());
            scripts_map
        } else {
            let mut scripts_map = std::collections::HashMap::new();
            scripts_map.insert("test".to_string(), "cargo test".to_string());
            scripts_map
        };
    
        config.save(&project_path)?;

        if ts_tests {
            fs::write(
                project_path.join("package.json"),
                PACKAGE_JSON.replace("default_project_name", &project_name),
            )?;
            fs::write(project_path.join("tsconfig.json"), TSCONFIG)?;
            fs::create_dir_all(project_path.join("tests"))?;
            fs::write(
                project_path
                    .join("tests")
                    .join(format!("{}.test.ts", project_name)),
                TS_TESTS.replace("default_project_name", &project_name),
            )?;

            Command::new("yarn")
                .current_dir(&project_path)
                .arg("install")
                .status()?;
        } else {
            fs::write(
                project_path.join("src").join("lib.rs"),
                RUST_TESTS.replace("default_project_name", &project_name),
            )?;
            fs::write(
                project_path.join("Cargo.toml"),
                CARGO_TOML.replace("default_project_name", &project_name),
            )?;
        }

        println!(
            "✅ Project '{}' initialized successfully with {} tests and configuration file",
            project_name,
            if ts_tests { "TypeScript" } else { "Rust" }
        );
        println!("📋 Configuration saved to {}/sbpf.toml", project_name);
        println!("💡 You can customize settings with 'sbpf config' commands");
        
        Ok(())
    } else {
        println!("⚠️ Project '{}' already exists!", project_name);
        Ok(())
    }
}