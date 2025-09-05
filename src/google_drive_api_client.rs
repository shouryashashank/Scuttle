use yup_oauth2::{read_application_secret, InstalledFlowAuthenticator, InstalledFlowReturnMethod};
use google_drive3::DriveHub;
use hyper_rustls::HttpsConnector;
use reqwest::Client;
use hyper::client::connect::HttpConnector;
use anyhow::{Context, Result};
use std::path::Path;

pub async fn get_drive_client(secret_path: &Path) -> Result<DriveHub<HttpsConnector<HttpConnector>>> {
    // 1. Read the client secret file.
    let secret = read_application_secret(secret_path)
        .await
        .context("Failed to read client secret file")?;

    // Create the InstalledFlowAuthenticator
    let auth = InstalledFlowAuthenticator::builder(
        secret,
        InstalledFlowReturnMethod::HTTPRedirect,
    )
    .persist_tokens_to_disk(".scuttle/token.json")
    .build()
    .await
    .context("Failed to create installed flow authenticator")?;

    // Build the HTTPS connector
    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()
        .https_or_http()
        .enable_http1()
        .build();

    // Build the hyper client
    let client = Client::builder().build();

    // Create the DriveHub client
    let drive_client = DriveHub::new(client, auth);

    println!("Drive client created and authenticated.");

    Ok(drive_client)
}
