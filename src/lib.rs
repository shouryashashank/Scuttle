mod google_drive_api_client;
use std::path::Path;
use anyhow::{Context, Result};
use std::fs;
use crate::google_drive_api_client::get_drive_client;
use std::io::{self, Write};

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
    println!("Downloaded! {}", file_path.display());
    Ok(())
}

pub async fn process_init() -> Result<()> {
    let _drive_client = get_drive_client(&"hello".to_string()).await?;
    
    println!("Initialized!");
    Ok(())
}

pub async fn process_setup() -> Result<()> {
    // start the setup process ask user to select service: google drive, dropbox, onedrive, smb
    // for now, only google drive is supported
    // later, we can add more services
    // then ask fo the remote server name
    // create a json file with the configuration
    // save it to the config directory  

    let config_dir = dirs::config_dir().context("Could not find config directory")?;
    let app_config_dir = config_dir.join("scuttle");
    if !app_config_dir.exists() {
        fs::create_dir_all(&app_config_dir).context("Failed to create config directory")?;
    }
    let config_file_path = app_config_dir.join("config.json");

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