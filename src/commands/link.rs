use anyhow::{Result, Context};
use sbpf_assembler::link_program;

pub fn link(source: &str) -> Result<()> {
    let program = std::fs::read(source).context("Failed to read bytecode")?;
    let bytecode = link_program(&program)
        .map_err(|e| anyhow::anyhow!("Link error: {}", e))?;
    let src_name = std::path::Path::new(source)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("main");
    let output_path = std::path::Path::new(source)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join(format!("{}.so", src_name));
    std::fs::write(output_path, bytecode)?;
    Ok(())
}