use {
    anyhow::{Error, Result},
    std::{io, path::Path, process::Command},
};

fn deploy_program(program_name: &str, url: &str) -> Result<(), Error> {
    let program_id_file = format!("./deploy/{}-keypair.json", program_name);
    let program_file = format!("./deploy/{}.so", program_name);

    if Path::new(&program_file).exists() {
        println!("🔄 Deploying \"{}\"", program_name);

        let status = Command::new("solana")
            .arg("program")
            .arg("deploy")
            .arg(&program_file)
            .arg("--program-id")
            .arg(&program_id_file)
            .arg("-u")
            .arg(url)
            .status()?;

        if !status.success() {
            eprintln!("Failed to deploy program for {}", program_name);
            return Err(Error::new(io::Error::other("❌ Deployment failed")));
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

fn deploy_all_programs(url: &str) -> Result<(), Error> {
    let deploy_path = Path::new("deploy");

    for entry in deploy_path.read_dir()? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && path.extension().and_then(|ext| ext.to_str()) == Some("so")
            && let Some(filename) = path.file_stem().and_then(|name| name.to_str())
        {
            deploy_program(filename, url)?;
        }
    }

    Ok(())
}

pub fn deploy(name: Option<String>, url: Option<String>) -> Result<(), Error> {
    let url = url.unwrap_or_else(|| "localhost".to_string());

    if let Some(program_name) = name {
        deploy_program(&program_name, &url)
    } else {
        deploy_all_programs(&url)
    }
}
