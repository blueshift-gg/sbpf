use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use regex::Regex;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SbpfConfig {
    pub project: ProjectConfig,

    #[serde(default)]
    pub build: BuildConfig,

    #[serde(default)]
    pub deploy: DeployConfig,

    #[serde(default)]
    pub test: TestConfig,

    #[serde(default)]
    pub scripts: ScriptsConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ProjectConfig {
    pub name: String,

    #[serde(default = "default_version")]
    pub version: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BuildConfig {
    #[serde(default = "default_optimization")]
    pub optimization: String,

    #[serde(default = "default_target")]
    pub target: String,

    #[serde(default)]
    pub flags: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub linker_script: Option<PathBuf>,

    #[serde(default = "default_build_mode")]
    pub mode: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeployConfig {
    #[serde(default = "default_cluster")]
    pub cluster: String,

    #[serde(default = "default_wallet")]
    pub wallet: String,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program_id: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upgrade_authority: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TestConfig {
    #[serde(default)]
    pub validator_args: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ScriptsConfig {
    #[serde(flatten)]
    pub scripts: HashMap<String, String>,
}

#[derive(Debug)]
pub enum SecurityWarning {
    MainnetRPC(String),
    PrivateRPC(String),
    CustomWallet(String),
}

fn default_version() -> String {
    "0.1.0".to_string()
}

fn default_optimization() -> String {
    "debug".to_string()
}

fn default_target() -> String {
    "sbf".to_string()
}

fn default_cluster() -> String {
    "localhost".to_string()
}

fn default_wallet() -> String {
    "~/.config/solana/id.json".to_string()
}

fn default_build_mode() -> String {
    "full".to_string()
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            optimization: default_optimization(),
            target: default_target(),
            flags: Vec::new(),
            linker_script: None,
            mode: default_build_mode(),
        }
    }
}

impl Default for DeployConfig {
    fn default() -> Self {
        Self {
            cluster: default_cluster(),
            wallet: default_wallet(),
            program_id: None,
            upgrade_authority: None,
        }
    }
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            validator_args: Vec::new(),
        }
    }
}

impl Default for ScriptsConfig {
    fn default() -> Self {
        let mut scripts_map = std::collections::HashMap::new();
        scripts_map.insert("test".to_string(), "cargo test".to_string());
        Self {
            scripts: scripts_map,
        }
    }
}

impl ScriptsConfig {
    pub fn get_script(&self, name: &str) -> Option<&String> {
        self.scripts.get(name)
    }
}

impl SecurityWarning {
    pub fn message(&self) -> String {
        match self {
            SecurityWarning::MainnetRPC(url) => {
                format!("Mainnet RPC detected: {}\n   Consider using environment variables for production endpoints", url)
            }
            SecurityWarning::PrivateRPC(url) => {
                format!("Private RPC detected: {}\n   This would be committed to version control", url)
            }
            SecurityWarning::CustomWallet(path) => {
                format!("Custom wallet path: {}\n   Ensure this path cannot be publicly accessed after commit", path)
            }
        }
    }
}

impl SbpfConfig {
    pub fn load() -> Result<Self> {
        Self::load_from_path(".")
    }
    
    pub fn load_from_path(dir: impl AsRef<Path>) -> Result<Self> {
        let config_path = dir.as_ref().join("sbpf.toml");
        
        if config_path.exists() {
            let content = fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;
            
            let mut config: Self = toml::from_str(&content)
                .with_context(|| {
                    format!("Failed to parse sbpf.toml: {}\n\n💡 Common TOML syntax issues:\n• Missing closing brackets [ ]\n• Unquoted strings that should be quoted\n• Invalid key names or values\n\nCheck your TOML syntax at: https://www.toml.io/", config_path.display())
                })?;

            config.resolve_all_env_vars();

            Ok(config)
        } else {
            let current_path = std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "<unknown>".to_string());
            
            anyhow::bail!(
                "No sbpf.toml found in current directory.\n\
                 Current directory: {}\n\
                 Searched for: {}/sbpf.toml\n\n\
                 💡 To fix this:\n\
                 • Run 'sbpf config init' to create a configuration file in this directory\n\
                 • Or navigate to a directory that contains an sbpf.toml file\n\
                 • Or create a new project with 'sbpf init <project-name>'",
                current_path,
                dir.as_ref().display()
            )
        }
    }
    
    pub fn load_or_default(project_name: &str) -> Self {
        Self::load().unwrap_or_else(|_| Self::default_for_project(project_name))
    }
    
    pub fn default_for_project(project_name: &str) -> Self {
        Self {
            project: ProjectConfig {
                name: project_name.to_string(),
                version: default_version(),
                authors: None,
                description: None,
            },
            build: BuildConfig::default(),
            deploy: DeployConfig::default(),
            test: TestConfig::default(),
            scripts: ScriptsConfig::default(),
        }
    }
    
    pub fn save(&self, dir: impl AsRef<Path>) -> Result<()> {
        let warnings = self.security_warnings();
        if !warnings.is_empty() {
            println!("⚠️  Security Warnings:");
            for warning in &warnings {
                println!("   {}", warning.message());
            }
            println!("💡 Consider using environment variables for sensitive values");
            println!();
        }

        let config_path = dir.as_ref().join("sbpf.toml");
        let content = toml::to_string_pretty(self)
            .context("Failed to serialize configuration to TOML")?;
        
        fs::write(&config_path, content)
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;
        
        println!("✔️ Configuration saved to {}", config_path.display());
        Ok(())
    }

    pub fn project_name(&self) -> &str {
        &self.project.name
    }

    pub fn is_release_build(&self) -> bool {
        self.build.optimization == "release"
    }

    pub fn compiler_args(&self) -> Vec<String> {
        let mut args = vec![
            "-target".to_string(),
            self.build.target.clone(),
        ];
        
        if self.is_release_build() {
            args.extend(["-O3".to_string(), "--strip".to_string()]);
        }
        
        args.extend(self.build.flags.clone());
        
        args
    }

    fn resolve_env_vars(value: &str) -> String {
        // Regex pattern:
        // \$\{           - Match literal ${
        // ([^:}]+)       - Capture variable name (group 1) - any chars except : and }
        // (?::-([^}]+))? - Optional non-capturing group for default value
        //   :-           - Literal :-
        //   ([^}]+)      - Capture default value (group 2) - any chars except }
        // \}             - Match literal }
        let re = Regex::new(r"\$\{([^:}]+)(?::-([^}]+))?\}").unwrap();
        
        re.replace_all(value, |caps: &regex::Captures| {
            let var_name = &caps[1];
            let default_value = caps.get(2).map(|m| m.as_str()).unwrap_or("");

            match std::env::var(var_name) {
                Ok(env_value) => {
                    println!("🔧 Using environment variable {}={}", var_name, env_value);
                    env_value
                }
                Err(_) => {
                    if !default_value.is_empty() {
                        println!("⚠️ Environment variable {} not found, using default: {}", var_name, default_value);
                    }
                    default_value.to_string()
                }
            }
        }).to_string()
    }
    
    fn resolve_all_env_vars(&mut self) {
        self.deploy.cluster = Self::resolve_env_vars(&self.deploy.cluster);
        
        if let Some(program_id) = &self.deploy.program_id {
            self.deploy.program_id = Some(Self::resolve_env_vars(program_id));
        }
        
        if let Some(upgrade_authority) = &self.deploy.upgrade_authority {
            self.deploy.upgrade_authority = Some(Self::resolve_env_vars(upgrade_authority));
        }

        self.build.target = Self::resolve_env_vars(&self.build.target);

        self.build.flags = self.build.flags.iter()
            .map(|flag| Self::resolve_env_vars(flag))
            .collect();

        self.test.validator_args = self.test.validator_args.iter()
            .map(|arg| Self::resolve_env_vars(arg))
            .collect();

        let resolved_scripts: HashMap<String, String> = self.scripts.scripts.iter()
            .map(|(name, command)| (name.clone(), Self::resolve_env_vars(command)))
            .collect();
        self.scripts.scripts = resolved_scripts;
    }

    pub fn resolve_cluster_to_url(cluster: &str) -> String {
        match cluster {
            "localhost" => "http://localhost:8899".to_string(),
            "devnet" => "https://api.devnet.solana.com".to_string(),
            "testnet" => "https://api.testnet.solana.com".to_string(),
            "mainnet" => "https://api.mainnet-beta.solana.com".to_string(),
            url if url.starts_with("http") => url.to_string(),
            _ => {
                println!("Using unique URL: '{}', as cluster", cluster);
                cluster.to_string()
            }
        }
    }


    pub fn resolve_cluster_url(&self) -> String {
        Self::resolve_cluster_to_url(&self.deploy.cluster)
    }

    pub fn has_env_vars(value: &str) -> bool {
        value.contains("${")
    }
    
    pub fn has_script(&self, name: &str) -> bool {
        self.scripts.scripts.contains_key(name)
    }
    
    pub fn set_script(&mut self, name: String, command: String) {
        self.scripts.scripts.insert(name, command);
    }
    
    pub fn remove_script(&mut self, name: &str) -> Option<String> {
        self.scripts.scripts.remove(name)
    }
    
    pub fn list_scripts(&self) -> Vec<&String> {
        self.scripts.scripts.keys().collect()
    }
    
    pub fn has_builtin_override(&self, command: &str) -> bool {
        match command {
            "test" | "build" | "deploy" => self.has_script(command),
            _ => false,
        }
    }
    
    pub fn get_builtin_override(&self, command: &str) -> Option<&String> {
        if self.has_builtin_override(command) {
            self.scripts.get_script(command)
        } else {
            None
        }
    }

    pub fn check_security_warnings_for_cluster(cluster_url: &str) -> Vec<SecurityWarning> {
        let mut warnings = Vec::new();
        
        if cluster_url.contains("mainnet") && !cluster_url.contains("localhost") {
            warnings.push(SecurityWarning::MainnetRPC(cluster_url.to_string()));
        }
        
        if cluster_url.starts_with("https") && 
           !cluster_url.contains("api.devnet") &&
           !cluster_url.contains("api.testnet") &&
           !cluster_url.contains("api.mainnet-beta") &&
           !cluster_url.contains("localhost") &&
           !cluster_url.contains(":8899") &&
           !Self::has_env_vars(cluster_url) {
            warnings.push(SecurityWarning::PrivateRPC(cluster_url.to_string()));
        }
        
        warnings
    }

    pub fn security_warnings(&self) -> Vec<SecurityWarning> {
        let resolved_url = self.resolve_cluster_url();
        Self::check_security_warnings_for_cluster(&resolved_url)
    }

    pub fn validate_build_mode(&self) -> Result<()> {
        match self.build.mode.as_str() {
            "full" | "light" => Ok(()),
            invalid => Err(anyhow::anyhow!(
                "Invalid build mode: '{}'\n\n💡 Valid modes:\n  • 'full' - Uses Solana toolchain (clang/ld)\n  • 'light' - Uses built-in sbpf-assembler\n\nUpdate your configuration:\n  sbpf config set build.mode full",
                invalid
            ))
        }
    }
    
    pub fn validate(&self) -> Result<()> {
        self.validate_build_mode()?;

        if !["debug", "release"].contains(&self.build.optimization.as_str()) {
            return Err(anyhow::anyhow!("build.optimization must be 'debug' or 'release'"));
        }
        
        Ok(())
    }
}