use google_drive3::{api::Scope,DriveHub};
use hyper::client::HttpConnector;
use hyper_rustls::HttpsConnector;
use std::fs;
use yup_oauth2::{
    read_application_secret, InstalledFlowAuthenticator, InstalledFlowReturnMethod,
};
use anyhow::{Context, Result};


async fn create_drive_client(remote_server_name: &String) -> Result<DriveHub<HttpsConnector<HttpConnector>>> {
    let config_dir = dirs::config_dir().context("Could not find config directory")?;
    let app_config_dir = config_dir.join("scuttle");
    if !app_config_dir.exists() {
        fs::create_dir_all(&app_config_dir).context("Failed to create config directory")?;
    }
    let secret = read_application_secret("credentials.json")
        .await
        .context("Failed to read credentials.json. Make sure it's in the correct path.")?;
    let auth = InstalledFlowAuthenticator::builder(
        secret,
        InstalledFlowReturnMethod::HTTPRedirect,
    )
    .persist_tokens_to_disk(app_config_dir.join(format!("{}_token.json", remote_server_name)))
    .build()
    .await
    .context("Failed to create authenticator")?;
    let client = hyper::Client::builder().build(
        hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .https_or_http()
            .enable_http1()
            .build(),
    );
    Ok(DriveHub::new(client, auth))
}


pub async fn get_drive_client(remote_server_name: &String) -> Result<DriveHub<HttpsConnector<HttpConnector>>> {
    let drive_client = create_drive_client(remote_server_name).await?;
    print!("Drive client created and authenticated.");
    print!("test clinet by fething file list...");
    let result = drive_client
        .files()
        .list()
        .add_scope(Scope::MetadataReadonly)
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

pub async fn upload_file(path: &std::path::Path, remote_server_name: &String) -> bool {
    println!("Uploading file...");
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
            eprintln!("Failed to open file: {}", e);
            return false;
        }
    };
    let metadata = google_drive3::api::File {
        name: Some(path.file_name().unwrap().to_string_lossy().to_string()),
        ..Default::default()
    };
    let mime_type = match "application/octet-stream".parse::<mime::Mime>() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to parse mime type: {}", e);
            return false;
        }
    };
    let request = drive_client.files().create(metadata).upload(file, mime_type);
    match request.await {
        Ok((_response, _file)) => {
            println!("File uploaded successfully.");
            true
        }
        Err(e) => {
            eprintln!("Failed to upload file: {}", e);
            false
        }
    }
}