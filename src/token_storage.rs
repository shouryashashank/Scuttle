use google_oauth2::AccessToken;
use std::fs::{self, File};
use std::io::{self,Read, Write};
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};

const TOKEN_FILE: &str = "token.json";

pub fn save_token(token: &AccessToken) -> Result<()> {
    let path = get_token_path()?;
    let json_string = serde_json::to_string_pretty(token).context("Failed to serialize token")?;
    fs::write(&path, json_string).context("Failed to write token file")?;
    Ok(())
}

pub fn load_token() -> Result<AccessToken> {
    let path = get_token_path()?;
    let mut file = File::open(&path).context("Failed to open token file")?;
    let mut content = String::new();
    file.read_to_string(&mut content).context("Failed to read token file")?;
    let token: AccessToken = serde_json::from_str(&content).context("Failed to parse token file")?;
    Ok(token)
}

fn get_token_path() -> Result<Protobuf> {
    let mut path = dirs::home_dir().context("Could not find home directory")?;
    path.push(".scuttle");

    if !path.exists() {
        fs::create_dir_all(&path).context("Failed to create .scuttle directory")?;
    }
    path.push(TOKEN_FILE);
    Ok(path)
}