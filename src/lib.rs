mod google_drive_api_client;
mod config;
mod utils;

use anyhow::{Context, Result,anyhow};
use std::fs;
use crate::google_drive_api_client::{get_drive_client, upload_file,download_file};
use std::io::{self, Write};
use google_drive3::DriveHub;
use std::fs::File;
mod sqlite_db;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::sqlite_db::{ScuttleDb, TrackedFile};

use crate::config::service::{get_config_detail, get_config_path};
use crate::utils::hashing::hash_file;
use crate::utils::filesystem::{load_scuttleignore, visit_dirs, add_file_to_db};

#[derive(Debug)]
pub enum Service {
    GoogleDrive,
    Dropbox,
    OneDrive,
    SMB,
}

impl Service {
    pub fn from_number(num: u32) -> Option<Self> {
        match num {
            1 => Some(Service::GoogleDrive),
            2 => Some(Service::Dropbox),
            3 => Some(Service::OneDrive),
            4 => Some(Service::SMB),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Service::GoogleDrive => "google_drive",
            Service::Dropbox => "dropbox",
            Service::OneDrive => "onedrive",
            Service::SMB => "smb",
        }
    }
}
pub async fn get_server_client(config: &serde_json::Value) -> Result<DriveHub<hyper_rustls::HttpsConnector<hyper::client::HttpConnector>>> {
    if let Some(service) = config.get("service").and_then(|s| s.as_str()) {
        match service {
            "google_drive" => {
                if let Some(remote_name) = config.get("remote_name").and_then(|n| n.as_str()) {
                    let drive_client = get_drive_client(&remote_name.to_string()).await?;
                    return Ok(drive_client);
                } else {
                    return Err(anyhow::anyhow!("remote_name not found in config"));
                }
            }
            "dropbox" => {
                // Initialize Dropbox client here
                unimplemented!("Dropbox client not implemented yet");
            }
            "onedrive" => {
                // Initialize OneDrive client here
                unimplemented!("OneDrive client not implemented yet");
            }
            "smb" => {
                // Initialize SMB client here
                unimplemented!("SMB client not implemented yet");
            }
            _ => {
                return Err(anyhow::anyhow!("Unsupported service: {}", service));
            }
        }
    } else {
        return Err(anyhow::anyhow!("service not found in config"));
    }
}

pub async fn process_upload(file_path: &Path, remote_name: Option<&str>) -> Result<()> {
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
    let confug_data = get_config_detail(remote_name)?;
    if confug_data.is_none() {
        return Err(anyhow::anyhow!("No configuration found. Please run setup first."));
    }
    let config = confug_data.unwrap();
    let remote_server = config.get("remote_name")
        .and_then(|n| n.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("Remote server name not found in config"))?;
    let uploaded = upload_file(file_path, &remote_server).await;
    if !uploaded {
        return Err(anyhow::anyhow!("File upload failed"));
    }
    println!("Uploaded!");

    Ok(())
}

pub async fn process_download(remote_path: &String, remote_name: Option<&str>) -> Result<()> {
    let confug_data = get_config_detail(remote_name)?;
    if confug_data.is_none() {
        return Err(anyhow::anyhow!("No configuration found. Please run setup first."));
    }
    let config = confug_data.unwrap();
    let remote_server = config.get("remote_name")
        .and_then(|n| n.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("Remote server name not found in config"))?;
    let downloaded = download_file(remote_path, &std::path::Path::new(".").to_path_buf(), &remote_server).await;
    if !downloaded {
        return Err(anyhow::anyhow!("File download failed"));
    }
    println!("Downloaded!");
    Ok(())
}

pub async fn process_init() -> anyhow::Result<()> {
    // Create .scuttle directory
    let scuttle_dir = PathBuf::from(".scuttle");
    if !scuttle_dir.exists() {
        std::fs::create_dir(&scuttle_dir)?;
        println!("Created .scuttle directory");
    } else {
        println!(".scuttle directory already exists");
    }

    // Create .scuttleignore file in root directory
    let ignore_file = PathBuf::from(".scuttleignore");
    if !ignore_file.exists() {
        let mut file = File::create(&ignore_file)?;
        let ignore_content = "# Ignore .scuttle directory\n.scuttle\n# Ignore temporary files\n*.tmp\n*.temp\n*.bak\n*.swp\n# Ignore system files\n.DS_Store\nThumbs.db\n# Ignore build directories\ntarget/\nbuild/\n# Ignore logs\n*.log\n# Ignore credentials\ncredentials.json\ntoken.json\n";
        file.write_all(ignore_content.as_bytes())?;
        println!("Created .scuttleignore file");
    } else {
        println!(".scuttleignore file already exists");
    }

    // Initialize SQLite database inside .scuttle
    let db_path = scuttle_dir.join("scuttle.db");
    let _db = ScuttleDb::new(&db_path)?;
    println!("Initialized SQLite database at {}", db_path.display());

    Ok(())
}

pub async fn process_setup() -> Result<()> {
    // start the setup process ask user to select service: google drive, dropbox, onedrive, smb
    // for now, only google drive is supported
    // later, we can add more services
    // then ask fo the remote server name
    // create a json file with the configuration
    // save it to the config directory  
    let config_file_path = get_config_path()?;

    // If config file does not exist, create default config list
    if !config_file_path.exists() {
        let default_config = r#"[]"#;
        fs::write(&config_file_path, default_config).context("Failed to write default config file")?;
        println!("Created default config file at {}", config_file_path.display());
    }

    // Read existing config file
    let config_data = fs::read_to_string(&config_file_path).context("Failed to read config file")?;

    // Parse JSON array
    let mut configs: Vec<serde_json::Value> = match serde_json::from_str(&config_data) {
        Ok(configs) => configs,
        Err(e) => {
            println!("Error parsing config file: {}", e);
            println!("Do you want to recreate the config file with defaults? (y/n): ");
            io::stdout().flush().unwrap();
            let mut input = String::new();
            io::stdin().read_line(&mut input).context("Failed to read input")?;
            
            vec![]
        }
    };


    // Ask user if they want to update config
    println!("Current configs: {}
        ", serde_json::to_string_pretty(&configs).unwrap());
    println!("Do you want to update the config? (y/n): ");
    io::stdout().flush().unwrap();
    let mut update_input = String::new();
    io::stdin().read_line(&mut update_input).context("Failed to read update input")?;
    let update_input = update_input.trim().to_lowercase();

    if update_input == "y" {
        // Ask user for service by number
        println!("Select service by number:\n1. google_drive\n2. dropbox\n3. onedrive\n4. smb");
        io::stdout().flush().unwrap();
        let mut service_input = String::new();
        io::stdin().read_line(&mut service_input).context("Failed to read service input")?;
        let service_num: u32 = service_input.trim().parse().unwrap_or(1);
        let service = Service::from_number(service_num).unwrap_or(Service::GoogleDrive);

        // Ask user for remote name
        println!("Enter remote server name: ");
        io::stdout().flush().unwrap();
        let mut remote_name = String::new();
        io::stdin().read_line(&mut remote_name).context("Failed to read remote name input")?;
        let remote_name = remote_name.trim();
        // Ask user if they want to make this the default
        println!("Do you want to make this the default configuration? (y/n): ");
        io::stdout().flush().unwrap();
        let mut default_input = String::new();
        io::stdin().read_line(&mut default_input).context("Failed to read default input")?;
        let make_default = default_input.trim().to_lowercase() == "y";

        if make_default {
            // Clear default flags from existing configs
            for config in &mut configs {
            if let Some(obj) = config.as_object_mut() {
                obj.insert("default".to_string(), serde_json::json!(false));
            }
            }
        }

        // Add new config
        let new_config = serde_json::json!({
            "service": service.as_str(),
            "remote_name": remote_name,
            "default": make_default
        });
        configs.push(new_config);

        // Write updated configs
        let updated_json = serde_json::to_string_pretty(&configs).context("Failed to serialize updated config")?;
        fs::write(&config_file_path, updated_json).context("Failed to write updated config file")?;
        println!("Config file updated with user settings.");
        let _drive_client = get_drive_client(&remote_name.to_string()).await?;
        println!("Initialized!");
    } else {
        println!("Using existing config.");
    }


    Ok(())
}


pub async fn process_status() -> Result<()> {
    // Load tracked files from database
    let db = ScuttleDb::new(&PathBuf::from(".scuttle/scuttle.db"))?;
    let tracked_files = db.get_tracked_files()?;
    let mut tracked_map: HashMap<String, &TrackedFile> = HashMap::new();
    for file in &tracked_files {
        tracked_map.insert(file.path.clone(), file);
    }

    // Scan local files recursively excluding .scuttle and respecting .scuttleignore
    let ignore_patterns = load_scuttleignore()?;
    let mut local_files = Vec::new();
    visit_dirs(Path::new("."), &ignore_patterns, &mut local_files)?;

    // Map local files by relative path
    let mut local_map: HashMap<String, PathBuf> = HashMap::new();
    for path in &local_files {
        if let Ok(rel_path) = path.strip_prefix(".") {
            local_map.insert(rel_path.to_string_lossy().to_string(), path.clone());
        }
    }

    // Compare and print status
    println!("Status:");

    // Check for new or modified files
    for (rel_path, local_path) in &local_map {
        if let Some(tracked) = tracked_map.get(rel_path) {
            // Compare hash
            let local_hash = hash_file(local_path)?;
            if Some(local_hash) != tracked.hash {
                println!("Modified: {}", rel_path);
            } else {
                println!("Unchanged: {}", rel_path);
            }
        } else {
            println!("New: {}", rel_path);
        }
    }

    // Check for deleted files
    for rel_path in tracked_map.keys() {
        if !local_map.contains_key(rel_path) {
            println!("Deleted: {}", rel_path);
        }
    }

    Ok(())
}

pub async fn process_add(paths: &[PathBuf]) -> anyhow::Result<()> {
    let db = ScuttleDb::new(Path::new(".scuttle/scuttle.db"))?;

    // Load ignore patterns
    let ignore_patterns = load_scuttleignore()?;
    let paths_to_process = paths.to_vec();

    // Get currently tracked files and map by path
    let tracked_files = db.get_tracked_files()?;
    let mut tracked_map = std::collections::HashMap::new();
    for file in tracked_files {
        tracked_map.insert(file.path.clone(), file);
    }

    for path in &paths_to_process {
        if !path.exists() {
            return Err(anyhow!("File not found: {}", path.display()));
        }
        if path.is_dir() {
            // Recursively add files in directory
            let mut files = Vec::new();
            visit_dirs(path, &ignore_patterns, &mut files)?;
            for file_path in files {
                let file_path_stripped = if let Ok(stripped) = file_path.strip_prefix(".") {
                    stripped.to_path_buf()
                } else {
                    file_path.to_path_buf()
                };

                // Calculate hash
                let hash = hash_file(&file_path_stripped)?;

                // Check if tracked and hash matches
                if let Some(tracked) = tracked_map.get(&file_path_stripped.to_string_lossy().to_string()) {
                    if tracked.hash.as_deref() == Some(&hash) {
                        // File unchanged, skip
                        continue;
                    }
                }

                add_file_to_db(&db, &ignore_patterns, &file_path_stripped)?;
            }
        } else {
            let path_stripped = if let Ok(stripped) = path.strip_prefix(".") {
                stripped.to_path_buf()
            } else {
                path.to_path_buf()
            };

            // Calculate hash
            let hash = hash_file(&path_stripped)?;

            // Check if tracked and hash matches
            if let Some(tracked) = tracked_map.get(&path_stripped.to_string_lossy().to_string()) {
                if tracked.hash.as_deref() == Some(&hash) {
                    // File unchanged, skip
                    continue;
                }
            }

            add_file_to_db(&db, &ignore_patterns, &path_stripped)?;
        }
    }

    Ok(())
}

pub async fn process_commit(message: &str) -> anyhow::Result<()> {
    let db = ScuttleDb::new(&std::path::PathBuf::from(".scuttle/scuttle.db"))?;
    db.commit(message)?;
    println!("Committed with message: {}", message);
    Ok(())
}

