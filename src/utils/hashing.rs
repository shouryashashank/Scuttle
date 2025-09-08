use sha2::{Sha256, Digest};
use std::path::Path;
use std::fs;
use anyhow::Result;

pub fn hash_file(path: &Path) -> Result<String> {
    let data = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Ok(format!("{:x}", hasher.finalize()))
}
