use crate::android::DEFAULT_MODDIR;
use crate::types::GatewayConfig;
use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info};

pub struct ConfigManager {
    mod_dir: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Self {
        let mod_dir = std::env::var("MODDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_MODDIR));
        Self { mod_dir }
    }

    pub fn json_path(&self) -> PathBuf {
        self.mod_dir.join("config.json")
    }

    pub fn load(&self) -> GatewayConfig {
        let path = self.json_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str::<GatewayConfig>(&content) {
                    debug!("Loaded {:?}", path);
                    return config;
                }
            }
        }
        info!("Using default configuration");
        GatewayConfig::default()
    }

    pub fn save(&self, config: &GatewayConfig) -> Result<()> {
        let path = self.json_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
        fs::write(&tmp, serde_json::to_string_pretty(config)?)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }
}
