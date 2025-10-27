use {
    super::common::{
        CARGO_TOML, GITIGNORE, PACKAGE_JSON, PROGRAM, README, RUST_TESTS, TS_TESTS, TSCONFIG,
    },
    anyhow::{Error, Result},
    ed25519_dalek::SigningKey,
    rand::rngs::OsRng,
    std::{
        fs,
        io::{self, Write},
        process::Command,
    },
};

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
            "✅ Project '{}' initialized successfully with {} tests",
            project_name,
            if ts_tests { "TypeScript" } else { "Rust" }
        );
        Ok(())
    } else {
        println!("⚠️ Project '{}' already exists!", project_name);
        Ok(())
    }
}
