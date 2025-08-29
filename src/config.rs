use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SbpfConfig {
    pub project: ProjectConfig,

    #[serde(default)]
    pub scripts: ScriptsConfig,

    #[serde(default)]
    pub deploy: DeployConfig,
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
pub struct ScriptsConfig {
    #[serde(flatten)]
    pub scripts: HashMap<String, String>,
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

fn default_version() -> String {
    "0.1.0".to_string()
}

fn default_cluster() -> String {
    "localhost".to_string()
}

fn default_wallet() -> String {
    "~/.config/solana/id.json".to_string()
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
            scripts: ScriptsConfig::default(),
            deploy: DeployConfig::default(),
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
}