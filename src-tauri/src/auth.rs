use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use dirs;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct GoogleTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: String,
    pub expires_in: Option<u64>,
    pub scope: Option<String>,
    pub token_type: Option<String>,
}

pub fn tokens_path() -> PathBuf {
    // Use the user's home directory, like swebench-debugger
    let mut home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.push(".swe-reviewer");
    if !home.exists() {
        let _ = fs::create_dir_all(&home);
    }
    home.join("google_tokens.json")
}

pub fn get_auth_state() -> Result<Option<String>, String> {
    let path = tokens_path();
    if path.exists() {
        let data = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let tokens: GoogleTokens = serde_json::from_str(&data).map_err(|e| e.to_string())?;
        // For now, just return the id_token (can be decoded for user info on frontend)
        Ok(Some(tokens.id_token))
    } else {
        Ok(None)
    }
}

pub fn get_google_client_id() -> Result<String, String> {
    std::env::var("GOOGLE_CLIENT_ID").map_err(|e| e.to_string())
}

pub fn save_google_tokens(tokens: GoogleTokens) -> Result<(), String> {
    let path = tokens_path();
    let data = serde_json::to_string(&tokens).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())
}

// Google OAuth client secret for refresh (should be kept private)
const GOOGLE_CLIENT_SECRET: &str = "";

pub async fn refresh_access_token(tokens: &GoogleTokens) -> Result<GoogleTokens, String> {
    let client = reqwest::Client::new();
    let params = [
        ("client_id", "917256818414-pcsi1favsuki4crrmd5st51ebp6ghl3g.apps.googleusercontent.com"),
        ("client_secret", GOOGLE_CLIENT_SECRET),
        ("refresh_token", &tokens.refresh_token),
        ("grant_type", "refresh_token"),
    ];
    let resp = client
        .post("https://oauth2.googleapis.com/token")
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Token refresh error: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("Failed to refresh token: {}", resp.status()));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| format!("Token refresh parse error: {}", e))?;
    let access_token = json["access_token"].as_str().ok_or("No access_token in refresh response")?.to_string();
    let id_token = json["id_token"].as_str().unwrap_or("").to_string();
    let expires_in = json["expires_in"].as_u64();
    let scope = json["scope"].as_str().map(|s| s.to_string());
    let token_type = json["token_type"].as_str().map(|s| s.to_string());
    Ok(GoogleTokens {
        access_token,
        refresh_token: tokens.refresh_token.clone(),
        id_token,
        expires_in,
        scope,
        token_type,
    })
}

pub fn logout() -> Result<(), String> {
    let tokens_path = tokens_path();
    if tokens_path.exists() {
        fs::remove_file(tokens_path)
            .map_err(|e| format!("Failed to remove tokens: {}", e))?;
    }
    Ok(())
}
