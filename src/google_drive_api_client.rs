// use yup_oauth2::{read_application_secret, InstalledFlowAuthenticator, InstalledFlowReturnMethod};
// use google_drive3::DriveHub;
// use hyper_rustls::HttpsConnector;
// use reqwest::Client;
// use hyper::client::connect::HttpConnector;
// use anyhow::{Context, Result};
// use std::path::Path;

// pub async fn get_drive_client(secret_path: &Path) -> Result<DriveHub<HttpsConnector<HttpConnector>>> {
//     // 1. Read the client secret file.
//     let secret = read_application_secret(secret_path)
//         .await
//         .context("Failed to read client secret file")?;

//     // Create the InstalledFlowAuthenticator
//     let auth = InstalledFlowAuthenticator::builder(
//         secret,
//         InstalledFlowReturnMethod::HTTPRedirect,
//     )
//     .persist_tokens_to_disk(".scuttle/token.json")
//     .build()
//     .await
//     .context("Failed to create installed flow authenticator")?;

//     // Build the HTTPS connector
//     let https = hyper_rustls::HttpsConnectorBuilder::new()
//         .with_native_roots()
//         .https_or_http()
//         .enable_http1()
//         .build();

//     // Build the hyper client
//     let client = Client::builder().build();

//     // Create the DriveHub client
//     let drive_client = DriveHub::new(client, auth);

//     println!("Drive client created and authenticated.");

//     Ok(drive_client)
// }
use google_drive3::{api::Scope, DriveHub};
use hyper::client::HttpConnector;
use hyper_rustls::HttpsConnector;
use yup_oauth2::{
    read_application_secret, InstalledFlowAuthenticator, InstalledFlowReturnMethod,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Authenticate and create a DriveHub client.
    let secret = read_application_secret("credentials.json")
        .await
        .expect("Failed to read credentials.json. Make sure it's in the correct path.");

    let auth = InstalledFlowAuthenticator::builder(
        secret,
        InstalledFlowReturnMethod::HTTPRedirect,
    )
    .persist_tokens_to_disk("token.json")
    .build()
    .await
    .expect("Failed to create authenticator");

    // FIX #1: The authenticator no longer needs a special wrapper.
    // We can build a standard hyper client and pass the authenticator directly to the DriveHub.
    let client = hyper::Client::builder().build(
        hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .https_or_http()
            .enable_http1()
            .build(),
    );

    let client: DriveHub<HttpsConnector<HttpConnector>> = DriveHub::new(client, auth);

    // 2. Call the Drive v3 API to list files.
    println!("Fetching files from Google Drive...");

    // The `doit()` method executes the request.
    let result = client
        .files()
        .list()
        .add_scope(Scope::MetadataReadonly)
        .page_size(10)
        // FIX #2: The method to specify fields was renamed.
        // `.fields()` is now a generic `.param()` call for the "fields" query parameter.
        .param("fields", "nextPageToken, files(id, name)")
        .doit()
        .await;

    // 3. Process the API response.
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

    Ok(())
}

