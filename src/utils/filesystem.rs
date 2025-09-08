use anyhow::Result;
use std::path::{Path, PathBuf};
use std::fs;
use std::time::UNIX_EPOCH;
use crate::utils::hashing::hash_file;
use crate::ScuttleDb;

pub fn load_scuttleignore() -> Result<Vec<String>> {
    let ignore_file = PathBuf::from(".scuttleignore");
    if !ignore_file.exists() {
        return Ok(vec![]);
    }
    let content = fs::read_to_string(ignore_file)?;
    let patterns = content.lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|s| s.to_string())
        .collect();
    Ok(patterns)
}

pub fn visit_dirs(dir: &Path, ignore_patterns: &[String], files: &mut Vec<PathBuf>) -> Result<()> {
    if dir.ends_with(".scuttle") {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Check ignore patterns
        if ignore_patterns.iter().any(|pattern| {
            if pattern.ends_with('/') {
                // Directory pattern
                path.is_dir() && file_name == pattern.trim_end_matches('/')
            } else {
                // File pattern
                file_name == pattern
            }
        }) {
            continue;
        }

        if path.is_dir() {
            visit_dirs(&path, ignore_patterns, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}

pub fn add_file_to_db(db: &ScuttleDb, ignore_patterns: &[String], path: &Path) -> anyhow::Result<()> {
    // Check if ignored
    if is_ignored(path, ignore_patterns)? {
        return Ok(());
    }

    // Get metadata
    let metadata = fs::metadata(path)?;
    let modified = metadata.modified()?.duration_since(UNIX_EPOCH)?.as_secs() as i64;

    // Calculate hash
    let hash = hash_file(path)?;

    // Insert or update in DB with status 'staged'
    db.add_file(&path.to_string_lossy(), &hash, modified, "staged")?;
    println!("Staged: {}", path.display());
    Ok(())
}

pub fn is_ignored(path: &Path, ignore_patterns: &[String]) -> anyhow::Result<bool> {
    let path_str = path.to_string_lossy();
    for pattern in ignore_patterns {
        if pattern.ends_with('/') {
            // Directory pattern: check if path starts with this directory
            if path_str.starts_with(pattern) {
                return Ok(true);
            }
        } else {
            // File pattern: check if file name matches
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if file_name == pattern {
                return Ok(true);
            }
        }
    }
    Ok(false)
}