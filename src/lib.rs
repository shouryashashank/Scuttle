// lib.rs is where the core logic of our application lives.
// This is called a "library" and can be reused by other projects.
use std::path::Path;
use anyhow::{Context, Result};
use std::fs;

/// This is our main function for processing an upload.
/// We'll move the actual file handling logic here later.
/// It takes a file path and fakes the upload process.
pub fn process_upload(file_path: &Path) -> Result<()> {
    // Check if the file exists before we "upload" it.
    if !file_path.exists() {
        return Err(anyhow::anyhow!("File not found: {}", file_path.display()));
    }

    // Read the file's contents into a vector of bytes.
    // The `?` operator is a shortcut for error handling. It returns the error if `fs::read` fails.
    let file_contents = fs::read(file_path).context("Failed to read file")?;
    
    // Use a placeholder message for now.
    println!("File name: {}", file_path.file_name().unwrap().to_str().unwrap());
    println!("File size: {} bytes", file_contents.len());
    println!("Uploaded!");

    Ok(())
}

pub fn process_download(file_path: &Path) -> Result<()> {
    println!{"Downloaded! {}", file_path.display()};
    Ok(())
}

pub fn process_init() -> Result<()> {
    
    println!{"Initialized!"};
    Ok(())
}