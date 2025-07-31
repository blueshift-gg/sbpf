use sbpf_assembler::assemble;

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use std::fs;

use anyhow::{Error, Result};
use std::path::Path;
use std::time::Instant;
use std::fs::create_dir_all;

pub fn light_build() -> Result<()> {
    let src = "src";
    let deploy = "deploy";

    // Create deploy directory if it doesn't exist
    if !Path::new(deploy).exists() {
        fs::create_dir_all(deploy)?;
    }

    // Find all assembly files in src directory
    let src_path = Path::new(src);
    if !src_path.exists() {
        return Err(Error::msg("Source directory 'src' not found"));
    }

    let mut compiled_files = Vec::new();

    for entry in fs::read_dir(src_path)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_dir() {
            if let Some(subdir) = path.file_name().and_then(|name| name.to_str()) {
                let asm_file = format!("{}/{}/{}.s", src, subdir, subdir);
                if Path::new(&asm_file).exists() {
                    println!("🔄 Building \"{}\"", subdir);
                    let start = std::time::Instant::now();
                    
                    // Use the new assembler API
                    let source_code = fs::read_to_string(&asm_file)?;
                    let (bytecode, metrics) = sbpf_assembler::assemble_with_validation(&source_code, deploy)?;
                    
                    // Write the compiled bytecode
                    let output_file = format!("{}/{}.so", deploy, subdir);
                    fs::write(&output_file, bytecode)?;
                    
                    let duration = start.elapsed();
                    println!(
                        "✅ \"{}\" built successfully in {}ms!",
                        subdir,
                        duration.as_micros() as f64 / 1000.0
                    );
                    
                    // Print metrics
                    metrics.print_report();
                    
                    compiled_files.push(subdir.to_string());
                }
            }
        }
    }

    if compiled_files.is_empty() {
        println!("⚠️  No assembly files found in src directory");
    } else {
        println!("🎉 Successfully compiled {} files", compiled_files.len());
    }

    Ok(())
}

// Legacy function for backward compatibility
fn compile_assembly(src: &str, deploy: &str) -> Result<()> {
    let source_code = fs::read_to_string(src)?;
    let (_, metrics) = sbpf_assembler::assemble_with_validation(&source_code, deploy)?;
    
    // Print metrics for debugging
    metrics.print_report();
    
    Ok(())
}
