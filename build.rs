use std::{env, fs, path::Path};

const DEPENDENCY_VERSIONS: [(&str, &str); 4] = [
    ("mollusk-svm", "MOLLUSK_SVM_VERSION"),
    ("solana-account", "SOLANA_ACCOUNT_VERSION"),
    ("solana-address", "SOLANA_ADDRESS_VERSION"),
    ("solana-instruction", "SOLANA_INSTRUCTION_VERSION"),
];

fn main() {
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=Cargo.toml.orig");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is not set");
    let manifest_path = Path::new(&manifest_dir).join("Cargo.toml.orig");
    let manifest_path = if manifest_path.exists() {
        manifest_path
    } else {
        Path::new(&manifest_dir).join("Cargo.toml")
    };

    let manifest = fs::read_to_string(&manifest_path)
        .unwrap_or_else(|error| panic!("Failed to read {}: {error}", manifest_path.display()));
    let manifest = manifest
        .parse::<toml::Table>()
        .unwrap_or_else(|error| panic!("Failed to parse {}: {error}", manifest_path.display()));
    let workspace_dependencies = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(toml::Value::as_table)
        .unwrap_or_else(|| {
            panic!(
                "Missing [workspace.dependencies] in {}",
                manifest_path.display()
            )
        });

    for (dependency_name, env_name) in DEPENDENCY_VERSIONS {
        let version =
            dependency_version(workspace_dependencies, dependency_name).unwrap_or_else(|| {
                panic!(
                    "Missing version for `{dependency_name}` in {}",
                    manifest_path.display()
                )
            });
        println!("cargo:rustc-env={env_name}={version}");
    }
}

fn dependency_version<'a>(dependencies: &'a toml::Table, dependency_name: &str) -> Option<&'a str> {
    match dependencies.get(dependency_name)? {
        toml::Value::String(version) => Some(version),
        toml::Value::Table(dependency) => dependency.get("version")?.as_str(),
        _ => None,
    }
}
