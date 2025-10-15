/// A simple RAII guard that resets an env var after the test exits
pub struct EnvVarGuard {
    key: String,
    original: Option<String>,
}

impl EnvVarGuard {
    pub fn new<K: Into<String>, V: Into<String>>(key: K, value: V) -> Self {
        let key = key.into();
        let original = std::env::var(&key).ok();
        std::env::set_var(&key, value.into());
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(ref val) = self.original {
            std::env::set_var(&self.key, val);
        } else {
            std::env::remove_var(&self.key);
        }
    }
}

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

/// Test environment setup for SBPF tests
pub struct TestEnv {
    pub sbpf_bin: String,
    pub temp_dir: PathBuf,
    pub project_dir: PathBuf,
    pub _guard: EnvVarGuard,
}

impl TestEnv {
    /// Set up a test environment with SBPF binary and temporary directory
    pub fn new(project_name: &str) -> Self {
        let current_dir = std::env::current_dir().expect("Failed to get current directory");
        let sbpf_path = current_dir.join("target").join("debug").join("sbpf");
        let guard = EnvVarGuard::new("SBPF_BIN", sbpf_path.to_string_lossy());
        let sbpf_bin = std::env::var("SBPF_BIN").expect("SBPF_BIN not set");

        let temp_dir = std::env::temp_dir().join(format!("sbpf_test_{}_e2e", project_name));
        if temp_dir.exists() {
            fs::remove_dir_all(&temp_dir).expect("Failed to remove existing test directory");
        }
        fs::create_dir_all(&temp_dir).expect("Failed to create test directory");

        let project_dir = temp_dir.join(project_name);

        println!("Using test directory: {:?}", temp_dir);
        println!("Using project directory: {:?}", project_dir);

        Self {
            sbpf_bin,
            temp_dir,
            project_dir,
            _guard: guard,
        }
    }

    /// Clean up the test environment
    pub fn cleanup(self) {
        fs::remove_dir_all(&self.temp_dir).expect("Failed to remove test directory");
    }
}

/// Run a command and return the output, panicking on failure
pub fn run_command(cmd: &mut Command, operation_name: &str) -> Output {
    let output = cmd
        .output()
        .unwrap_or_else(|_| panic!("Failed to execute {}", operation_name));

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        panic!(
            "{} failed:\nSTDOUT: {}\nSTDERR: {}",
            operation_name, stdout, stderr
        );
    }

    println!("✅ {} completed successfully", operation_name);
    println!("STDOUT: {}", String::from_utf8_lossy(&output.stdout));

    output
}

/// Initialize a new SBPF project
pub fn init_project(env: &TestEnv, project_name: &str) {
    println!("Step 1: Initializing {} project...", project_name);

    let init_output = run_command(
        Command::new(&env.sbpf_bin)
            .current_dir(&env.temp_dir)
            .arg("init")
            .arg(project_name),
        &format!("target/debug/sbpf init {}", project_name),
    );

    println!("✅ Project initialized successfully");
    println!("STDOUT: {}", String::from_utf8_lossy(&init_output.stdout));
}

/// Verify that the project structure is correct
pub fn verify_project_structure(env: &TestEnv, project_name: &str) {
    let project_dir = &env.project_dir;

    assert!(
        project_dir.join("src").exists(),
        "src directory should exist"
    );
    assert!(
        project_dir.join("deploy").exists(),
        "deploy directory should exist"
    );
    assert!(
        project_dir.join(format!("src/{}", project_name)).exists(),
        "src/{} directory should exist",
        project_name
    );
    assert!(
        project_dir
            .join(format!("src/{}/{}.s", project_name, project_name))
            .exists(),
        "src/{}/{}.s should exist",
        project_name,
        project_name
    );
    assert!(
        project_dir.join("src/lib.rs").exists(),
        "src/lib.rs should exist"
    );
    assert!(
        project_dir.join("Cargo.toml").exists(),
        "Cargo.toml should exist"
    );
}

/// Run build on the project
pub fn run_build(env: &TestEnv) {
    println!("Step 3: Running build...");

    run_command(
        Command::new(&env.sbpf_bin)
            .current_dir(&env.project_dir)
            .arg("build"),
        "target/debug/sbpf build",
    );
}

/// Verify that .so files were created in the deploy directory
pub fn verify_so_files(env: &TestEnv) {
    let deploy_dir = env.project_dir.join("deploy");
    assert!(deploy_dir.exists(), "deploy directory should exist");

    let so_files: Vec<_> = fs::read_dir(&deploy_dir)
        .expect("Failed to read deploy directory")
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "so")
                .unwrap_or(false)
        })
        .collect();

    assert!(
        !so_files.is_empty(),
        "At least one .so file should be created in deploy directory"
    );
    println!("Found {} .so file(s) in deploy directory", so_files.len());
}

/// Run tests on the project
#[allow(dead_code)]
pub fn run_tests(env: &TestEnv) {
    println!("Step 4: Running tests...");

    run_command(
        Command::new(&env.sbpf_bin)
            .current_dir(&env.project_dir)
            .arg("test"),
        "target/debug/sbpf test",
    );
}

/// Update the project's assembly file content
pub fn update_assembly_file(env: &TestEnv, project_name: &str, content: &str) {
    let assembly_path = env
        .project_dir
        .join(format!("src/{}/{}.s", project_name, project_name));
    fs::write(&assembly_path, content)
        .unwrap_or_else(|_| panic!("Failed to write new {}.s content", project_name));
    println!("✅ Updated {}.s with specified content", project_name);
}
