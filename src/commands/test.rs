use {
    anyhow::{Error, Result},
    std::{fs, io, path::Path, process::Command},
};

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
        crate::commands::build::build(false, false)?;
    }

    let has_cargo = Path::new("Cargo.toml").exists();
    let has_package_json = Path::new("package.json").exists();

    match (has_cargo, has_package_json) {
        (true, _) => {
            let output = Command::new("cargo")
                .arg("test-sbf")
                .arg("--")
                .arg("--nocapture")
                .env("RUST_BACKTRACE", "1")
                .status()?;

            if !output.success() {
                eprintln!("Failed to run Rust tests");
                return Err(Error::new(io::Error::other("❌ Rust tests failed")));
            }
        }
        (false, true) => {
            crate::commands::deploy(None, None)?;

            let status = Command::new("yarn").arg("test").status()?;

            if !status.success() {
                eprintln!("Failed to run tests");
                return Err(Error::new(io::Error::other("❌ Test failed")));
            }
        }
        (false, false) => {
            return Err(Error::new(io::Error::new(
                io::ErrorKind::NotFound,
                "❌ No test configuration found. Expected either Cargo.toml or package.json",
            )));
        }
    }

    println!("✅ Tests completed successfully!");
    Ok(())
}
