use google_oauth2::{AccessToken, Authenticator, ConsoleApplicationSecret, read_application_secret};
use google_drive3::api::Scope;
use google_drive3::Drive;
use hyper::Client::Client;
use hyper_rustls::HttpsConnectorBuilder;
use hyper::body::Incoming;
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use crate::token_storage::{save_token, load_token};


pub async fn get_drive_client(secret_path: &Path) -> Result<Drive<HttpsConnectorBuilder,hyper_rustls::HttpsConnector<Incoming>>> {
    if let Ok(token) = load_token() {
        let secret: ConsoleApplicationSecret = read_application_secret(secret_path)
            .await
            .context("Failed to read application secret")?;
            
            
    }
}