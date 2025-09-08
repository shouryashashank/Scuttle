use anyhow::{Context, Result};
use std::fs;

pub fn get_config_path() -> Result<std::path::PathBuf> {
    let config_dir = dirs::config_dir().context("Could not find config directory")?;
    let app_config_dir = config_dir.join("scuttle");
    if !app_config_dir.exists() {
        fs::create_dir_all(&app_config_dir).context("Failed to create config directory")?;
    }
    let config_file_path = app_config_dir.join("config.json");
    Ok(config_file_path)
}
pub fn get_configs() -> Result<Vec<serde_json::Value>> {
    let config_path = get_config_path()?;
    if !config_path.exists() {
        return Ok(vec![]);
    }
    let config_data = fs::read_to_string(&config_path).context("Failed to read config file")?;
    let configs: Vec<serde_json::Value> = serde_json::from_str(&config_data).context("Failed to parse config file")?;
    Ok(configs)
}
pub fn get_config_detail(remote_name: Option<&str>) -> Result<Option<serde_json::Value>> {
    let configs = get_configs()?;
    
    match remote_name {
        Some(name) if !name.is_empty() => {
            // Search for config with matching remote name
            for config in &configs {
                if let Some(config_name) = config.get("remote_name") {
                    if config_name == name {
                        return Ok(Some(config.clone()));
                    }
                }
            }
        }
        _ => {
            // Return the default config if it exists
            for config in &configs {
                if let Some(is_default) = config.get("default") {
                    if is_default.as_bool().unwrap_or(false) {
                        return Ok(Some(config.clone()));
                    }
                }
            }
        }
    }
    
    Ok(None)
}

