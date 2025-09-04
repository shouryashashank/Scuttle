use yup_oauth2::{AccessToken, authenticator, ConsoleApplicationSecret, read_application_secret};
use google_drive3::api::Scope;
use google_drive3::api::Drive;
use hyper_rustls::HttpsConnectorBuilder;
use hyper::body::Incoming;
use reqwest::Client;
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};

use crate::token_storage::{load_token, save_token};


pub async fn get_drive_client(secret_path: &Path) -> Result<Drive<HttpsConnectorBuilder, hyper_rustls::HttpsConnector<Incoming>>> {
    // 1. Read the client secret file.
    let secret: ConsoleApplicationSecret = read_application_secret(secret_path)
        .await
        .context("Failed to read client secret file");
    
    // Check for a previously saved token.
    if let Ok(token) = load_token() {
        println!("Found existing token. Attempting to get authenticated client...");
        
        // Use the saved token to create an Authenticator.
        let auth = authenticator::InnerAuthenticator::builder()
            .client_secret(&secret)
            .access_token(token)
            .build()
            .await
            .context("Failed to create authenticator with existing token")?;

        // Build the Drive client. This will automatically refresh the token if it's expired.
        let drive_client = Drive::new(hyper::Client::builder()
            .build(HttpsConnectorBuilder::new().with_native_roots().https_or_http().enable_http1().enable_http2().build()), auth);
        
        println!("Using existing token. Authorization is complete.");
        return Ok(drive_client);
    }
    
    // If no token exists, start the new authorization flow.
    println!("No existing token found. Starting new authorization flow...");
    
    // Create a new Authenticator without a pre-existing token.
    let auth = authenticator::InnerAuthenticator::builder()
        .client_secret(&secret)
        .scopes(&[Scope::Full])
        .build()
        .await
        .context("Failed to create authenticator")?;

    // This is the core of the authorization process. This method will open a browser,
    // wait for the user to authorize, and then handle the token exchange.
    let token = auth.token(&[Scope::Full])
        .await
        .context("Failed to authenticate and get token")?;

    // Save the token for future use
    save_token(&token).context("Failed to save token")?;

    // Build the Drive client
    let drive_client = Drive::new(hyper::Client::builder()
        .build(HttpsConnectorBuilder::new().with_native_roots().https_or_http().enable_http1().enable_http2().build()), auth);
    
    println!("Authorization successful! Token saved for future use.");
    
    Ok(drive_client)
}
