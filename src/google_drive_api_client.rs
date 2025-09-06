use google_drive3::{api::Scope,DriveHub};
use hyper::client::HttpConnector;
use hyper_rustls::HttpsConnector;
use std::fs;
use yup_oauth2::{
    read_application_secret, InstalledFlowAuthenticator, InstalledFlowReturnMethod,
};
use anyhow::{Context, Result};



pub async fn get_drive_client(remote_server_name: &String) -> Result<DriveHub<HttpsConnector<HttpConnector>>> {
    let config_dir = dirs::config_dir().context("Could not find config directory")?;
    let app_config_dir = config_dir.join("scuttle");
    if !app_config_dir.exists() {
        fs::create_dir_all(&app_config_dir).context("Failed to create config directory")?;
    }
    let secret = read_application_secret("credentials.json")
        .await
        .expect("Failed to read credentials.json. Make sure it's in the correct path.");
    let auth = InstalledFlowAuthenticator::builder(
        secret,
        InstalledFlowReturnMethod::HTTPRedirect,
    )
    .persist_tokens_to_disk(app_config_dir.join(format!("{}_token.json", remote_server_name)))
    .build()
    .await
    .expect("Failed to create authenticator");
    let client = hyper::Client::builder().build(
        hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .https_or_http()
            .enable_http1()
            .build(),
    );
    let drive_client = DriveHub::new(client, auth);
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