mod google_drive_api_client;
mod config;
mod utils;

use anyhow::{Context, Result};
use std::fs;
use crate::google_drive_api_client::{get_drive_client, upload_file, download_file, find_folder_by_name, find_file_in_folder, download_file_by_id, upload_file_with_parent, create_folder, delete_file_by_id, ensure_remote_path};
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
        // If path does not exist, mark it deleted instead of returning an error
        if !path.exists() {
            let path_stripped = if let Ok(stripped) = path.strip_prefix(".") {
                stripped.to_path_buf()
            } else {
                path.to_path_buf()
            };
            add_file_to_db(&db, &ignore_patterns, &path_stripped)?; // add_file_to_db will mark deleted when missing
            continue;
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

    // After processing provided paths, check all tracked files to see if any are missing locally and mark them deleted
    for (tracked_path, _) in tracked_map.iter() {
        let local_path = Path::new(".").join(tracked_path);
        if !local_path.exists() {
            let stripped = if let Ok(s) = local_path.strip_prefix(".") { s.to_path_buf() } else { local_path };
            add_file_to_db(&db, &ignore_patterns, &stripped)?; // will mark deleted
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

pub async fn process_push(remote_name: Option<&str>) -> anyhow::Result<()> {
    // Minimal scaffold: verify config exists and print a message.
    let confug_data = get_config_detail(remote_name)?;
    if confug_data.is_none() {
        return Err(anyhow::anyhow!("No configuration found. Please run init first."));
    }
    let config = confug_data.unwrap();
    let remote_server = config.get("remote_name")
        .and_then(|n| n.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("Remote server name not found in config"))?;
    println!("Starting push for remote: {}", remote_server);

    // Resolve remote root folder by name (best-effort)
    // Determine remote folder name by checking the parent of the current directory
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let folder_name = current_dir.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&remote_server);
    println!("Using remote root folder name: {}", folder_name);
    let remote_root_folder = match find_folder_by_name(folder_name, &remote_server).await {
        Ok(Some(id)) => {
            println!("Found remote root folder id: {}", id);
            Some(id)
        }
        Ok(None) => {
            println!("Remote root folder not found by name {}. Initial upload expected.", folder_name);
            None
        }
        Err(e) => {
            println!("Error searching for remote folder: {}", e);
            None
        }
    };

    // If we have a remote root folder, look for scuttle.db inside it
    if let Some(root_id) = remote_root_folder {
        // Prefer `.scuttle/scuttle.db` inside the project root folder on remote
        let scuttle_folder_id = match find_file_in_folder(".scuttle", &root_id, &remote_server).await {
            Ok(Some(id)) => Some(id),
            Ok(None) => None,
            Err(e) => {
                println!("Error searching for remote .scuttle folder: {}", e);
                None
            }
        };

        let search_parent = scuttle_folder_id.as_deref().unwrap_or(&root_id);
        match find_file_in_folder("scuttle.db", search_parent, &remote_server).await {
             Ok(Some(file_id)) => {
                 println!("Found remote scuttle.db with id {}. Downloading...", file_id);
                 let dest = PathBuf::from(".scuttle/remote_scuttle.db.tmp");
                 std::fs::create_dir_all(PathBuf::from(".scuttle")).ok();
                 if let Err(e) = download_file_by_id(&file_id, &dest, &remote_server).await {
                     println!("Failed to download remote scuttle.db: {}", e);
                     return Err(anyhow::anyhow!("Failed to download remote DB"));
                 }
                 println!("Downloaded remote DB to {}", dest.display());

                 // Compute diff between remote DB and local DB
                 let local_db_path = PathBuf::from(".scuttle/scuttle.db");
                 if !local_db_path.exists() {
                     println!("Local scuttle DB not found at {}", local_db_path.display());
                     return Err(anyhow::anyhow!("Local DB missing"));
                 }

                 match ScuttleDb::diff_dbs(&dest, &local_db_path) {
                     Ok((added, modified, deleted)) => {
                         println!("Diff results - added: {}, modified: {}, deleted: {}", added.len(), modified.len(), deleted.len());
                         println!("Added: {:?}\nModified: {:?}\nDeleted: {:?}", added, modified, deleted);
                         // Apply deltas: deletes first
                        // Helper: find remote file id by path under root_id
                        async fn find_remote_id_by_path(root_id: &str, rel_path: &str, remote_server: &str) -> Option<String> {
                            // Traverse folders
                            let mut parent = root_id.to_string();
                            let path = rel_path.replace("\\", "/");
                            let comps: Vec<&str> = path.split('/').collect();
                            if comps.is_empty() { return None; }
                            for (i, comp) in comps.iter().enumerate() {
                                if comp.is_empty() { continue; }
                                let name = comp.to_string();
                                if i == comps.len()-1 {
                                    // last -> file; search in parent
                                    match find_file_in_folder(&name, &parent, &remote_server.to_string()).await {
                                        Ok(Some(id)) => return Some(id),
                                        _ => return None,
                                    }
                                } else {
                                    // folder
                                    match find_file_in_folder(&name, &parent, &remote_server.to_string()).await {
                                        Ok(Some(id)) => parent = id,
                                        _ => return None,
                                    }
                                }
                            }
                            None
                        }

                        // Execute deletes
                        for path in &deleted {
                            println!("Deleting remote: {}", path);
                            if let Some(remote_id) = find_remote_id_by_path(&root_id, path, &remote_server).await {
                                if let Err(e) = delete_file_by_id(&remote_id, &remote_server).await {
                                    println!("Failed to delete {}: {}", path, e);
                                } else {
                                    println!("Deleted remote {}", path);
                                }
                            } else {
                                println!("Remote file not found for deletion: {}", path);
                            }
                        }

                        // Upload added and modified (treat both similarly)
                        for path in added.iter().chain(modified.iter()) {
                            let local_path = Path::new(".").join(path);
                            if !local_path.exists() {
                                println!("Local file missing for upload: {}", path);
                                continue;
                            }
                            // Ensure remote parent folders exist under project root
                            let parent_dir = Path::new(path).parent().map(|p| p.to_string_lossy().to_string());
                            let target_parent = if let Some(dir) = parent_dir {
                                if dir.is_empty() || dir == "." {
                                    root_id.clone()
                                } else {
                                    match crate::google_drive_api_client::ensure_remote_path(&root_id, &dir, &remote_server).await {
                                        Ok(id) => id,
                                        Err(e) => {
                                            println!("Failed to ensure remote dir {}: {}. Uploading to root.", dir, e);
                                            root_id.clone()
                                        }
                                    }
                                }
                            } else { root_id.clone() };

                            println!("Uploading {} to remote...", path);
                            match upload_file_with_parent(&local_path, Some(&target_parent), &remote_server).await {
                                Ok(id) => println!("Uploaded {} as id {}", path, id),
                                Err(e) => println!("Failed to upload {}: {}", path, e),
                            }
                        }

                        // Safe DB swap: find existing scuttle.db id first, upload local DB, then delete old
                        println!("Preparing DB swap: locating existing scuttle.db (if any)...");
                        let scuttle_folder_id = match crate::google_drive_api_client::ensure_remote_path(&root_id, ".scuttle", &remote_server).await {
                            Ok(id) => id,
                            Err(e) => {
                                println!("Failed to ensure remote .scuttle folder: {}. Aborting DB swap.", e);
                                return Err(anyhow::anyhow!("Failed to ensure remote .scuttle"));
                            }
                        };
                        // capture old id before upload
                        let old_scuttle_id = match find_file_in_folder("scuttle.db", &scuttle_folder_id, &remote_server).await {
                            Ok(Some(id)) => Some(id),
                            _ => None,
                        };

                        // Upload local DB (this will create a new scuttle.db in the folder)
                        println!("Uploading local scuttle DB...");
                        match upload_file_with_parent(&local_db_path, Some(&scuttle_folder_id), &remote_server).await {
                            Ok(new_id) => {
                                println!("Uploaded new scuttle DB as id {}", new_id);
                                // delete old if present and different from new
                                if let Some(old_id) = old_scuttle_id {
                                    if old_id != new_id {
                                        if let Err(e) = delete_file_by_id(&old_id, &remote_server).await {
                                            println!("Failed to delete old remote scuttle.db: {}", e);
                                        } else {
                                            println!("Deleted old remote scuttle.db (id={})", old_id);
                                        }
                                    } else {
                                        println!("Old and new scuttle.db IDs are same; no delete needed");
                                    }
                                }
                                println!("DB swap completed.");
                            }
                            Err(e) => println!("Failed to upload new scuttle DB: {}", e),
                        }

                        println!("Push apply complete.");
                     }
                     Err(e) => {
                         println!("Failed to compute DB diff: {}", e);
                         return Err(anyhow::anyhow!("DB diff failed"));
                     }
                 }

              }
             Ok(None) => {
                println!("No remote scuttle.db found in remote root or .scuttle folder. Initial upload required.");
                // TODO: implement initial upload: create .scuttle folder on remote and upload files
             }
             Err(e) => {
                 println!("Error searching for scuttle.db in remote root: {}", e);
                 return Err(anyhow::anyhow!("Failed during remote DB lookup"));
             }
         }
     } else {
        println!("Remote root not found; creating remote root folder and performing initial upload.");

        // Create remote root folder with the remote_server name
        let created_root = match create_folder(&folder_name, None, &remote_server).await {
            Ok(id) => {
                println!("Created remote root folder '{}' with id {}", remote_server, id);
                id
            }
            Err(e) => {
                println!("Failed to create remote root folder: {}", e);
                return Err(anyhow::anyhow!("Failed to create remote root folder"));
            }
        };

        // Load local tracked files from DB and upload each file that exists locally into the created folder
        let db_path = Path::new(".scuttle/scuttle.db");
        let tracked_files = ScuttleDb::load_tracked_files(db_path)?;
        let mut uploaded = 0usize;
        let mut skipped = 0usize;
        for tf in tracked_files {
            // Skip deleted entries
            if let Some(status) = &tf.status {
                if status == "deleted" {
                    skipped += 1;
                    continue;
                }
            }

            let local_path = PathBuf::from(".").join(&tf.path);
            if local_path.exists() {
                println!("Uploading {}...", tf.path);
                // Determine parent folder path on remote and ensure it exists
                let parent_dir = Path::new(&tf.path).parent().map(|p| p.to_string_lossy().to_string());
                let target_parent_id = if let Some(dir) = parent_dir {
                    if dir.is_empty() || dir == "." {
                        created_root.clone()
                    } else {
                        match crate::google_drive_api_client::ensure_remote_path(&created_root, &dir, &remote_server).await {
                            Ok(id) => id,
                            Err(e) => {
                                println!("Failed to ensure remote dir {}: {}", dir, e);
                                created_root.clone()
                            }
                        }
                    }
                } else {
                    created_root.clone()
                };

                let res = upload_file_with_parent(&local_path, Some(&target_parent_id), &remote_server).await;
                if res.is_ok() {
                    uploaded += 1;
                } else {
                    println!("Failed to upload {}", tf.path);
                }
            } else {
                println!("Local file missing, skipping: {}", tf.path);
                skipped += 1;
            }
        }

        // Finally, upload the scuttle DB itself into the created root
        let db_path = PathBuf::from(".scuttle/scuttle.db");
        if db_path.exists() {
            println!("Uploading scuttle DB...");
            // Ensure remote has a `.scuttle` folder and upload DB there
            let scuttle_folder_id = match crate::google_drive_api_client::ensure_remote_path(&created_root, ".scuttle", &remote_server).await {
                Ok(id) => id,
                Err(e) => {
                    println!("Failed to ensure remote .scuttle folder: {}. Falling back to root.", e);
                    created_root.clone()
                }
            };
            let res = upload_file_with_parent(&db_path, Some(&scuttle_folder_id), &remote_server).await;
            if res.is_ok() { println!("Uploaded remote scuttle.db"); } else { println!("Failed to upload scuttle.db"); }
        } else {
            println!("Local scuttle DB not found at {}", db_path.display());
        }

        println!("Initial upload completed: uploaded={}, skipped={}", uploaded, skipped);
    }

    Ok(())
}

