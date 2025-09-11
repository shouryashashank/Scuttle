use google_drive3::{api::Scope, DriveHub};
use hyper::client::HttpConnector;
use hyper_rustls::HttpsConnector;
use std::fs;
use yup_oauth2::{
    read_application_secret, InstalledFlowAuthenticator, InstalledFlowReturnMethod,
};
use anyhow::{Context, Result};

/// Creates and authenticates a new Google Drive client.
/// This function handles the OAuth2 flow and token persistence.
async fn create_drive_client(remote_server_name: &String) -> Result<DriveHub<HttpsConnector<HttpConnector>>> {
    let config_dir = dirs::config_dir().context("Could not find config directory")?;
    let app_config_dir = config_dir.join("scuttle");
    if !app_config_dir.exists() {
        fs::create_dir_all(&app_config_dir).context("Failed to create config directory")?;
    }

    // Read application secret from a file.
    // Ensure `credentials.json` is in the same directory as your executable.
    let secret = read_application_secret("credentials.json")
        .await
        .context("Failed to read credentials.json. Make sure it's in the correct path.")?;

    // Build the authenticator, which will handle token storage.
    let auth = InstalledFlowAuthenticator::builder(
        secret,
        InstalledFlowReturnMethod::HTTPRedirect,
    )
    .persist_tokens_to_disk(app_config_dir.join(format!("{}_token.json", remote_server_name)))
    .build()
    .await
    .context("Failed to create authenticator")?;

    // Build the HTTPS client.
    let client = hyper::Client::builder().build(
        hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .https_or_http()
            .enable_http1()
            .build(),
    );

    Ok(DriveHub::new(client, auth))
}

/// A test function to get a drive client and list the first 10 files.
pub async fn get_drive_client(remote_server_name: &String) -> Result<DriveHub<HttpsConnector<HttpConnector>>> {
    let drive_client = create_drive_client(remote_server_name).await?;
    println!("Drive client created and authenticated.");
    println!("Testing client by fetching file list...");

    // List files to test the connection.
    let result = drive_client
        .files()
        .list()
        // Use Scope::Readonly to see files.
        .add_scope(Scope::Readonly)
        .page_size(10)
        .param("fields", "nextPageToken, files(id, name)")
        .doit()
        .await;

    match result {
        Ok((_response, file_list)) => {
            if let Some(files) = file_list.files {
                if files.is_empty() {
                    println!("No files found.");
                } else {
                    println!("\nFiles:");
                    for file in files {
                        let name = file.name.unwrap_or_else(|| "Unnamed".to_string());
                        let id = file.id.unwrap_or_else(|| "No ID".to_string());
                        println!("- {} ({})", name, id);
                    }
                }
            } else {
                println!("No files found.");
            }
        }
        Err(e) => {
            println!("\nAn error occurred: {}", e);
        }
    }
    Ok(drive_client)
}

/// Uploads a single file to Google Drive.
pub async fn upload_file(path: &std::path::Path, remote_server_name: &String) -> bool {
    println!("Uploading file: {:?}", path);
    let drive_client = match create_drive_client(remote_server_name).await {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Failed to create drive client: {}", e);
            return false;
        }
    };

    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to open file {:?}: {}", path, e);
            return false;
        }
    };

    // Define file metadata.
    let metadata = google_drive3::api::File {
        name: Some(path.file_name().unwrap().to_string_lossy().to_string()),
        ..Default::default()
    };

    let mime_type = "application/octet-stream".parse::<mime::Mime>().unwrap();

    // Perform the upload request.
    let request = drive_client
        .files()
        .create(metadata)
        // Add the correct scope for uploading files.
        .add_scope(Scope::Full)
        .upload(file, mime_type);

    match request.await {
        Ok((_response, file)) => {
            println!("File uploaded successfully: {}", file.name.unwrap_or_default());
            true
        }
        Err(e) => {
            eprintln!("Failed to upload file: {}", e);
            false
        }
    }
}

/// Downloads a file by name from Google Drive, searching across all drives.
pub async fn download_file(file_name: &str, destination_folder: &std::path::Path, remote_server_name: &String) -> bool {
    use std::io::Write;
    use hyper::body::HttpBody;

    println!("Downloading file '{}' with shared drive support...", file_name);
    let drive_client = match create_drive_client(remote_server_name).await {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Failed to create drive client: {}", e);
            return false;
        }
    };

    // Search for the file by name to get its ID.
    let result = drive_client.files().list()
        .q(&format!("name = '{}' and trashed = false", file_name))
        .param("fields", "files(id, name)")
        .supports_all_drives(true)
        .include_items_from_all_drives(true)
        .add_scope(Scope::Readonly) // Scope to read metadata
        .doit()
        .await;

    let file_id = match result {
        Ok((_resp, file_list)) => {
            if let Some(files) = file_list.files {
                if let Some(file) = files.first() {
                     file.id.clone().unwrap_or_default()
                } else {
                    eprintln!("No file found with name: {}", file_name);
                    return false;
                }
            } else {
                eprintln!("No files found in the API response.");
                return false;
            }
        }
        Err(e) => {
            eprintln!("Failed to search for file by name: {}", e);
            return false;
        }
    };

    if file_id.is_empty() {
        eprintln!("Could not find a valid file ID for '{}'", file_name);
        return false;
    }
    println!("Found file ID: {}", file_id);


    let destination_path = destination_folder.join(file_name);
    let mut file = match std::fs::File::create(&destination_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to create destination file {:?}: {}", destination_path, e);
            return false;
        }
    };

    // Request file content with shared drive support.
    let request = drive_client.files().get(&file_id)
        .param("alt", "media")
        .supports_all_drives(true)
        // **FIX**: Add the correct scope to read file content.
        .add_scope(Scope::Readonly);

    match request.doit().await {
        Ok((mut response, _)) => {
            let mut downloaded: u64 = 0;
            while let Some(chunk) = response.body_mut().data().await {
                match chunk {
                    Ok(bytes) => {
                        if let Err(e) = file.write_all(&bytes) {
                            eprintln!("Failed to write chunk to file: {}", e);
                            return false;
                        }
                        downloaded += bytes.len() as u64;
                        println!("Downloaded {} bytes...", downloaded);
                    }
                    Err(e) => {
                        eprintln!("Error while downloading chunk: {}", e);
                        return false;
                    }
                }
            }
            println!("File downloaded successfully to {:?}", destination_path);
            true
        }
        Err(e) => {
            eprintln!("Failed to download file content: {}", e);
            false
        }
    }
}

/// Find a folder by name at the root or across drives. Returns the file ID if found.
pub async fn find_folder_by_name(folder_name: &str, remote_server_name: &String) -> Result<Option<String>> {
    let drive_client = create_drive_client(remote_server_name).await?;
    let q = format!("name = '{}' and mimeType = 'application/vnd.google-apps.folder' and trashed = false", folder_name);
    let result = drive_client.files().list()
        .q(&q)
        .param("fields", "files(id, name)")
        .supports_all_drives(true)
        .include_items_from_all_drives(true)
        .add_scope(Scope::Readonly)
        .doit()
        .await;

    match result {
        Ok((_resp, list)) => {
            if let Some(files) = list.files {
                if let Some(file) = files.first() {
                    Ok(file.id.clone())
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        }
        Err(e) => Err(anyhow::anyhow!("Failed to search for folder: {}", e)),
    }
}

/// Find a file by name under a given parent folder id. Returns file id if found.
pub async fn find_file_in_folder(file_name: &str, parent_id: &str, remote_server_name: &String) -> Result<Option<String>> {
    let drive_client = create_drive_client(remote_server_name).await?;
    // Query for name and parent
    let q = format!("name = '{}' and '{}' in parents and trashed = false", file_name, parent_id);
    let result = drive_client.files().list()
        .q(&q)
        .param("fields", "files(id, name)")
        .supports_all_drives(true)
        .include_items_from_all_drives(true)
        .add_scope(Scope::Readonly)
        .doit()
        .await;

    match result {
        Ok((_resp, list)) => {
            if let Some(files) = list.files {
                if let Some(file) = files.first() {
                    Ok(file.id.clone())
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        }
        Err(e) => Err(anyhow::anyhow!("Failed to search for file in folder: {}", e)),
    }
}

use std::path::Path;
use hyper::body::HttpBody;

/// Upload a file to Drive under optional parent_id. Returns the uploaded file ID on success.
pub async fn upload_file_with_parent(path: &Path, parent_id: Option<&str>, remote_server_name: &String) -> Result<String> {
    let drive_client = create_drive_client(remote_server_name).await?;
    let file = std::fs::File::open(path).context("Failed to open file for upload")?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();

    let mut metadata = google_drive3::api::File::default();
    metadata.name = Some(name.clone());
    if let Some(parent) = parent_id {
        metadata.parents = Some(vec![parent.to_string()]);
    }

    let mime_type = "application/octet-stream".parse::<mime::Mime>().unwrap();
    let request = drive_client.files().create(metadata).add_scope(Scope::Full).upload(file, mime_type);

    match request.await {
        Ok((_resp, file)) => Ok(file.id.unwrap_or_default()),
        Err(e) => Err(anyhow::anyhow!("Failed to upload file: {}", e)),
    }
}

/// Delete a file by id. Returns true if deleted.
pub async fn delete_file_by_id(file_id: &str, remote_server_name: &String) -> Result<bool> {
    let drive_client = create_drive_client(remote_server_name).await?;
    let res = drive_client.files().delete(file_id).add_scope(Scope::Full).doit().await;
    if let Err(e) = res {
        return Err(anyhow::anyhow!("Failed to delete file: {}", e));
    }
    Ok(true)
}

/// Download a file by id into the specified destination path.
pub async fn download_file_by_id(file_id: &str, destination: &Path, remote_server_name: &String) -> Result<()> {
    use std::io::Write;
    let drive_client = create_drive_client(remote_server_name).await?;
    let request = drive_client.files().get(file_id).param("alt", "media").supports_all_drives(true).add_scope(Scope::Readonly);
    match request.doit().await {
        Ok((mut response, _)) => {
            let mut file = std::fs::File::create(destination).context("Failed to create destination file")?;
            while let Some(chunk) = response.body_mut().data().await {
                let bytes = chunk.context("Error reading response chunk")?;
                file.write_all(&bytes).context("Failed to write to destination file")?;
            }
            Ok(())
        }
        Err(e) => Err(anyhow::anyhow!("Failed to download file by id: {}", e)),
    }
}
