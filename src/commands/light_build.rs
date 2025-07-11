use sbpf_assembler::assemble;

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use std::fs;

use anyhow::Result;
use std::path::Path;
use std::time::Instant;
use std::fs::create_dir_all;

use crate::config::SbpfConfig;

pub fn light_build() -> Result<()> {
    // The original light_build function for backward compatibility
    light_build_internal(None)
}

pub fn light_build_with_project_name(project_name: &str) -> Result<()> {
    light_build_internal(Some(project_name))
}

pub fn light_build_with_config(config: &SbpfConfig) -> Result<()> {
    println!("   Project: {}", config.project.name);
    
    light_build_internal(Some(&config.project.name))
}

fn light_build_internal(project_name_override: Option<&str>) -> Result<()> {
    // Set src/out directory
    let src = "src";
    let deploy = "deploy";

    // Create necessary directories
    create_dir_all(deploy)?;

    // Function to compile assembly
    fn compile_assembly(src: &str, deploy: &str) -> Result<()> {
        assemble(src, deploy)
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
        let project_name = if let Some(name) = project_name_override {
            name.to_string()
        } else {
            std::env::current_dir()?
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("program")
                .to_string()
        };

        let mut rng = OsRng;
        fs::write(
            deploy_path.join(format!("{}-keypair.json", project_name)),
            serde_json::json!(SigningKey::generate(&mut rng).to_keypair_bytes()[..]).to_string(),
        )?;
        println!("🔑 Generated keypair for project '{}'", project_name);
    }

    // Processing directories
    let src_path = Path::new(src);
    for entry in src_path.read_dir()? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(subdir) = path.file_name().and_then(|name| name.to_str()) {
                let asm_file = format!("{}/{}/{}.s", src, subdir, subdir);
                if Path::new(&asm_file).exists() {
                    println!("⚡️ Light building \"{}\"", subdir);
                    let start = Instant::now();
                    compile_assembly(&asm_file, deploy)?;
                    let duration = start.elapsed();
                    println!(
                        "✔️ \"{}\" built successfully in {}ms! (light mode)",
                        subdir,
                        duration.as_micros() as f64 / 1000.0
                    );
                }
            }
        }
    }

    Ok(())
}
