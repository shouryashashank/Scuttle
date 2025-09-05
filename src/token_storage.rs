use yup_oauth2::AccessToken;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use anyhow::{Context, Result};

const TOKEN_FILE: &str = "token.json";

/// Saves the AccessToken to a file in the user's home directory.
///
/// This function serializes the AccessToken struct directly into a JSON file
/// named `token.json` within a `.scuttle` directory.
pub fn save_token(token: &AccessToken) -> Result<()> {
    let path = get_token_path()?;
    let json_string = serde_json::to_string_pretty(token).context("Failed to serialize token")?;
    fs::write(&path, json_string).context("Failed to write token file")?;
    Ok(())
}

/// Loads the AccessToken from a file in the user's home directory.
///
/// This function reads the `token.json` file from the `.scuttle` directory and
/// deserializes its JSON content directly into an AccessToken struct.
pub fn load_token() -> Result<AccessToken> {
    let path = get_token_path()?;
    let mut file = fs::File::open(&path).context("Failed to open token file")?;
    let mut content = String::new();
    file.read_to_string(&mut content).context("Failed to read token file")?;
    let token: AccessToken = serde_json::from_str(&content).context("Failed to parse token file")?;
    Ok(token)
}

/// Gets the full path to the token file.
///
/// It creates the `.scuttle` directory if it doesn't exist.
fn get_token_path() -> Result<PathBuf> {
    let mut path = dirs::home_dir().context("Could not find home directory")?;
    path.push(".scuttle");

    if !path.exists() {
        fs::create_dir_all(&path).context("Failed to create .scuttle directory")?;
    }
    path.push(TOKEN_FILE);
    Ok(path)
}