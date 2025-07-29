use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SbpfConfig {
    pub project: ProjectConfig,

    #[serde(default)]
    pub build: BuildConfig,

    #[serde(default)]
    pub deploy: DeployConfig,

    #[serde(default)]
    pub test: TestConfig,
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
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeployConfig {
    #[serde(default = "default_cluster")]
    pub cluster: String,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program_id: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upgrade_authority: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TestConfig {
    #[serde(default = "default_test_framework")]
    pub framework: String,

    #[serde(default)]
    pub validator_args: Vec<String>,
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

fn default_test_framework() -> String {
    "mollusk".to_string()
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            optimization: default_optimization(),
            target: default_target(),
            flags: Vec::new(),
            linker_script: None,
        }
    }
}

impl Default for DeployConfig {
    fn default() -> Self {
        Self {
            cluster: default_cluster(),
            program_id: None,
            upgrade_authority: None,
        }
    }
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            framework: default_test_framework(),
            validator_args: Vec::new(),
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
            
            let config: Self = toml::from_str(&content)
                .with_context(|| {
                    format!("Failed to parse sbpf.toml: {}\n\n💡 Common TOML syntax issues:\n• Missing closing brackets [ ]\n• Unquoted strings that should be quoted\n• Invalid key names or values\n\nCheck your TOML syntax at: https://www.toml.io/", config_path.display())
                })?;
            
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
        }
    }
    
    pub fn save(&self, dir: impl AsRef<Path>) -> Result<()> {
        let config_path = dir.as_ref().join("sbpf.toml");
        let content = toml::to_string_pretty(self)
            .context("Failed to serialize configuration to TOML")?;
        
        fs::write(&config_path, content)
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;
        
        println!("✅ Configuration saved to {}", config_path.display());
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
}