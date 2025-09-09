use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use dirs;


#[derive(Serialize, Deserialize, Default, Clone)]
struct GoogleTokens {
    access_token: String,
    refresh_token: String,
    id_token: String,
    expires_in: Option<u64>,
    scope: Option<String>,
    token_type: Option<String>,
}

fn tokens_path() -> PathBuf {
    // Use the user's home directory, like swebench-debugger
    let mut home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.push(".swe-reviewer");
    if !home.exists() {
        let _ = fs::create_dir_all(&home);
    }
    home.join("google_tokens.json")
}

fn settings_path() -> PathBuf {
    let mut home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.push(".swe-reviewer");
    if !home.exists() {
        let _ = fs::create_dir_all(&home);
    }
    home.join("settings.json")
}

#[tauri::command]
fn get_auth_state() -> Result<Option<String>, String> {
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

#[tauri::command]
fn get_google_client_id() -> Result<String, String> {
    std::env::var("GOOGLE_CLIENT_ID").map_err(|e| e.to_string())
}

#[tauri::command]
fn save_google_tokens(tokens: GoogleTokens) -> Result<(), String> {
    let path = tokens_path();
    let data = serde_json::to_string(&tokens).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())
}

// Utility to extract Google Drive file ID from a link
fn extract_drive_file_id(link: &str) -> Option<String> {
    // Handles links like:
    // https://drive.google.com/file/d/FILE_ID/view?usp=sharing
    // https://drive.google.com/open?id=FILE_ID
    // https://drive.google.com/uc?id=FILE_ID&export=download
    let patterns = [
        ("/file/d/", "/"),
        ("open?id=", "&"),
        ("uc?id=", "&"),
    ];
    for (start_pat, end_pat) in patterns.iter() {
        if let Some(start) = link.find(start_pat) {
            let after = &link[start + start_pat.len()..];
            let end = after.find(end_pat).unwrap_or(after.len());
            return Some(after[..end].to_string());
        }
    }
    None
}

// Utility to extract Google Drive folder ID from a link
fn extract_drive_folder_id(link: &str) -> Option<String> {
    // Handles links like:
    // https://drive.google.com/drive/folders/FOLDER_ID?usp=sharing
    // https://drive.google.com/drive/u/0/folders/FOLDER_ID
    // https://drive.google.com/open?id=FOLDER_ID
    let patterns = [
        ("/folders/", "?"),
        ("/folders/", "&"), 
        ("/folders/", "#"),
        ("open?id=", "&"),
        ("open?id=", "#"),
    ];
    for (start_pat, end_pat) in patterns.iter() {
        if let Some(start) = link.find(start_pat) {
            let after = &link[start + start_pat.len()..];
            let end = after.find(end_pat).unwrap_or(after.len());
            return Some(after[..end].to_string());
        }
    }
    None
}

// Get all shared drives accessible to the user
async fn get_shared_drives(access_token: &str) -> Result<Vec<(String, String)>, String> {
    use reqwest::header::AUTHORIZATION;
    
    let client = reqwest::Client::new();
    let url = "https://www.googleapis.com/drive/v3/drives?fields=drives(id,name)";
    
    let resp = client
        .get(url)
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| format!("Shared drives API error: {}", e))?;
        
    if !resp.status().is_success() {
        return Ok(vec![]); // Return empty if can't get shared drives
    }
    
    let result: serde_json::Value = resp.json().await
        .map_err(|e| format!("Shared drives JSON parse error: {}", e))?;
        
    let drives = result["drives"].as_array().unwrap_or(&vec![])
        .iter()
        .filter_map(|drive| {
            let name = drive["name"].as_str()?;
            let id = drive["id"].as_str()?;
            Some((name.to_string(), id.to_string()))
        })
        .collect();
        
    Ok(drives)
}

// Get folder contents from Google Drive
async fn get_folder_contents(folder_id: &str, access_token: &str) -> Result<serde_json::Value, String> {
    use reqwest::header::AUTHORIZATION;
    
    let client = reqwest::Client::new();
    let query = format!("'{}' in parents", folder_id);
    let encoded_query = urlencoding::encode(&query);
    
    // First try personal drive
    let personal_url = format!(
        "https://www.googleapis.com/drive/v3/files?q={}&fields=files(id,name,mimeType)&supportsAllDrives=true",
        encoded_query
    );
    
    let resp = client
        .get(&personal_url)
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| format!("Personal drive API error: {}", e))?;
        
    if resp.status().is_success() {
        let result: serde_json::Value = resp.json().await
            .map_err(|e| format!("Personal drive JSON parse error: {}", e))?;
            
        if let Some(files) = result["files"].as_array() {
            if !files.is_empty() {
                return Ok(serde_json::json!({
                    "files": files,
                    "debug_info": {
                        "successful_query": query,
                        "drive": "personal",
                        "files_count": files.len()
                    }
                }));
            }
        }
    }
    
    // If not found in personal drive, dynamically get and try all shared drives
    let shared_drives = get_shared_drives(access_token).await.unwrap_or_else(|_| vec![]);
    
    for (drive_name, drive_id) in shared_drives {
        let shared_url = format!(
            "https://www.googleapis.com/drive/v3/files?q={}&fields=files(id,name,mimeType)&driveId={}&includeItemsFromAllDrives=true&supportsAllDrives=true&corpora=drive",
            encoded_query, drive_id
        );
        
        let resp = client
            .get(&shared_url)
            .header(AUTHORIZATION, format!("Bearer {}", access_token))
            .send()
            .await
            .map_err(|e| format!("Shared drive '{}' API error: {}", drive_name, e))?;
            
        if resp.status().is_success() {
            let result: serde_json::Value = resp.json().await
                .map_err(|e| format!("Shared drive '{}' JSON parse error: {}", drive_name, e))?;
                
            if let Some(files) = result["files"].as_array() {
                if !files.is_empty() {
                    return Ok(serde_json::json!({
                        "files": files,
                        "debug_info": {
                            "successful_query": query,
                            "drive": drive_name,
                            "drive_id": drive_id,
                            "files_count": files.len()
                        }
                    }));
                }
            }
        }
    }
    
    Err("Folder not found in personal drive or any accessible shared drives".to_string())
}

// Get folder metadata from Google Drive
async fn get_folder_metadata(folder_id: &str, access_token: &str) -> Result<serde_json::Value, String> {
    use reqwest::header::AUTHORIZATION;
    
    let url = format!(
        "https://www.googleapis.com/drive/v3/files/{}?fields=id,name,mimeType&supportsAllDrives=true",
        folder_id
    );
    
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| format!("Drive API error: {}", e))?;
        
    if !resp.status().is_success() {
        return Err(format!("Failed to get folder metadata: {}", resp.status()));
    }
    
    resp.json().await.map_err(|e| format!("JSON parse error: {}", e))
}

// Google OAuth client secret for refresh (should be kept private)
const GOOGLE_CLIENT_SECRET: &str = "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";

async fn refresh_access_token(tokens: &GoogleTokens) -> Result<GoogleTokens, String> {
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

#[tauri::command]
async fn download_drive_file(link: String) -> Result<serde_json::Value, String> {
    use reqwest::header::AUTHORIZATION;
    use serde_json::Value;

    // Extract file ID
    let file_id = extract_drive_file_id(&link).ok_or("Invalid Google Drive link")?;

    // Load tokens
    let path = tokens_path();
    let data = std::fs::read_to_string(&path).map_err(|e| format!("Token read error: {}", e))?;
    let mut tokens: GoogleTokens = serde_json::from_str(&data).map_err(|e| format!("Token parse error: {}", e))?;
    let mut access_token = tokens.access_token.clone();

    // Get file metadata to check MIME type
    let meta_url = format!("https://www.googleapis.com/drive/v3/files/{}?fields=mimeType,name&supportsAllDrives=true", file_id);
    let client = reqwest::Client::new();
    let mut meta_resp = client
        .get(&meta_url)
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| format!("Drive API error: {}", e))?;
    if meta_resp.status() == 403 || meta_resp.status() == 401 {
        // Try refresh
        tokens = refresh_access_token(&tokens).await?;
        access_token = tokens.access_token.clone();
        // Save new tokens
        let _ = save_google_tokens(tokens.clone());
        // Retry
        meta_resp = client
            .get(&meta_url)
            .header(AUTHORIZATION, format!("Bearer {}", access_token))
            .send()
            .await
            .map_err(|e| format!("Drive API error: {}", e))?;
        if meta_resp.status() == 403 || meta_resp.status() == 401 {
            return Err("Permission denied or token expired".to_string());
        }
    }
    if !meta_resp.status().is_success() {
        return Err(format!("Failed to fetch file metadata: {}", meta_resp.status()));
    }
    let meta: Value = meta_resp.json().await.map_err(|e| format!("Metadata parse error: {}", e))?;
    let mime_type = meta["mimeType"].as_str().unwrap_or("");
    let name = meta["name"].as_str().unwrap_or("");
    // Only allow text/*, application/json, application/xml, etc.
    let allowed = mime_type.starts_with("text/") || mime_type == "application/json" || mime_type == "application/xml";
    if !allowed {
        return Err(format!("File '{}' is not a supported text file (MIME: {})", name, mime_type));
    }

    // Download file content
    let download_url = format!("https://www.googleapis.com/drive/v3/files/{}?alt=media&supportsAllDrives=true", file_id);
    let mut file_resp = client
        .get(&download_url)
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| format!("Download error: {}", e))?;
    if file_resp.status() == 403 || file_resp.status() == 401 {
        // Try refresh
        tokens = refresh_access_token(&tokens).await?;
        access_token = tokens.access_token.clone();
        // Save new tokens
        let _ = save_google_tokens(tokens.clone());
        // Retry
        file_resp = client
            .get(&download_url)
            .header(AUTHORIZATION, format!("Bearer {}", access_token))
            .send()
            .await
            .map_err(|e| format!("Download error: {}", e))?;
        if file_resp.status() == 403 || file_resp.status() == 401 {
            return Err("Permission denied or token expired".to_string());
        }
    }
    if !file_resp.status().is_success() {
        return Err(format!("Failed to download file: {}", file_resp.status()));
    }
    let content = file_resp.text().await.map_err(|e| format!("File read error: {}", e))?;
    Ok(serde_json::json!({ "content": content, "name": name }))
}

#[tauri::command]
async fn upload_drive_file(link: String, content: String) -> Result<(), String> {
    use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
    use serde_json::Value;

    // Extract file ID
    let file_id = extract_drive_file_id(&link).ok_or("Invalid Google Drive link")?;

    // Load tokens
    let path = tokens_path();
    let data = std::fs::read_to_string(&path).map_err(|e| format!("Token read error: {}", e))?;
    let mut tokens: GoogleTokens = serde_json::from_str(&data).map_err(|e| format!("Token parse error: {}", e))?;
    let mut access_token = tokens.access_token.clone();

    // Get file metadata to check MIME type
    let meta_url = format!("https://www.googleapis.com/drive/v3/files/{}?fields=mimeType,name&supportsAllDrives=true", file_id);
    let client = reqwest::Client::new();
    let mut meta_resp = client
        .get(&meta_url)
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| format!("Drive API error: {}", e))?;
    if meta_resp.status() == 403 || meta_resp.status() == 401 {
        // Try refresh
        tokens = refresh_access_token(&tokens).await?;
        access_token = tokens.access_token.clone();
        // Save new tokens
        let _ = save_google_tokens(tokens.clone());
        // Retry
        meta_resp = client
            .get(&meta_url)
            .header(AUTHORIZATION, format!("Bearer {}", access_token))
            .send()
            .await
            .map_err(|e| format!("Drive API error: {}", e))?;
        if meta_resp.status() == 403 || meta_resp.status() == 401 {
            return Err("Permission denied or token expired".to_string());
        }
    }
    if !meta_resp.status().is_success() {
        return Err(format!("Failed to fetch file metadata: {}", meta_resp.status()));
    }
    let meta: Value = meta_resp.json().await.map_err(|e| format!("Metadata parse error: {}", e))?;
    let mime_type = meta["mimeType"].as_str().unwrap_or("");
    let name = meta["name"].as_str().unwrap_or("");
    // Only allow text/*, application/json, application/xml, etc.
    let allowed = mime_type.starts_with("text/") || mime_type == "application/json" || mime_type == "application/xml";
    if !allowed {
        return Err(format!("File '{}' is not a supported text file (MIME: {})", name, mime_type));
    }

    // Upload (replace) file content
    let upload_url = format!("https://www.googleapis.com/upload/drive/v3/files/{}?uploadType=media&supportsAllDrives=true", file_id);
    let mut upload_resp = client
        .patch(&upload_url)
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .header(CONTENT_TYPE, mime_type)
        .body(content.clone())
        .send()
        .await
        .map_err(|e| format!("Upload error: {}", e))?;
    if upload_resp.status() == 403 || upload_resp.status() == 401 {
        // Try refresh
        tokens = refresh_access_token(&tokens).await?;
        access_token = tokens.access_token.clone();
        // Save new tokens
        let _ = save_google_tokens(tokens.clone());
        // Retry
        upload_resp = client
            .patch(&upload_url)
            .header(AUTHORIZATION, format!("Bearer {}", access_token))
            .header(CONTENT_TYPE, mime_type)
            .body(content)
            .send()
            .await
            .map_err(|e| format!("Upload error: {}", e))?;
        if upload_resp.status() == 403 || upload_resp.status() == 401 {
            return Err("Permission denied or token expired".to_string());
        }
    }
    if !upload_resp.status().is_success() {
        return Err(format!("Failed to upload file: {}", upload_resp.status()));
    }
    Ok(())
}

#[tauri::command]
fn save_setting(key: String, value: String) -> Result<(), String> {
    let settings_path = settings_path();
    let mut settings = if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)
            .map_err(|e| format!("Failed to read settings: {}", e))?;
        serde_json::from_str::<serde_json::Value>(&content)
            .unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };
    
    if let Some(obj) = settings.as_object_mut() {
        obj.insert(key, serde_json::Value::String(value));
    }
    
    fs::write(&settings_path, serde_json::to_string_pretty(&settings).unwrap())
        .map_err(|e| format!("Failed to save settings: {}", e))?;
    
    Ok(())
}

#[tauri::command]
fn load_setting(key: String) -> Result<String, String> {
    let settings_path = settings_path();
    if !settings_path.exists() {
        return Ok(String::new());
    }
    
    let content = fs::read_to_string(&settings_path)
        .map_err(|e| format!("Failed to read settings: {}", e))?;
    let settings: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse settings: {}", e))?;
    
    Ok(settings.get(&key).and_then(|v| v.as_str()).unwrap_or("").to_string())
}

#[tauri::command]
fn logout() -> Result<(), String> {
    let tokens_path = tokens_path();
    if tokens_path.exists() {
        fs::remove_file(tokens_path)
            .map_err(|e| format!("Failed to remove tokens: {}", e))?;
    }
    Ok(())
}

// Report Checker Commands
#[derive(Serialize, Deserialize)]
struct FileInfo {
    id: String,
    name: String,
    path: String,
}

#[derive(Serialize, Deserialize)]
struct ValidationResult {
    files_to_download: Vec<FileInfo>,
    folder_id: String,
}

#[tauri::command]
async fn validate_deliverable(folder_link: String) -> Result<ValidationResult, String> {
    // Rule 1: The link should be accessible and Rule 2: Should be to a folder not a file
    let folder_id = extract_drive_folder_id(&folder_link)
        .ok_or("Invalid Google Drive folder link. Please provide a valid folder URL.")?;
    
    // Load tokens for API access
    let path = tokens_path();
    if !path.exists() {
        return Err("Please authenticate with Google Drive first".to_string());
    }
    
    let data = std::fs::read_to_string(&path)
        .map_err(|e| format!("Token read error: {}", e))?;
    let mut tokens: GoogleTokens = serde_json::from_str(&data)
        .map_err(|e| format!("Token parse error: {}", e))?;
    let mut access_token = tokens.access_token.clone();
    
    // Get folder metadata to verify it's accessible and is a folder
    let mut folder_meta = get_folder_metadata(&folder_id, &access_token).await;
    if folder_meta.is_err() {
        // Try refreshing token
        tokens = refresh_access_token(&tokens).await?;
        access_token = tokens.access_token.clone();
        let _ = save_google_tokens(tokens.clone());
        folder_meta = get_folder_metadata(&folder_id, &access_token).await;
    }
    
    let folder_meta = folder_meta?;
    let mime_type = folder_meta["mimeType"].as_str().unwrap_or("");
    let folder_name = folder_meta["name"].as_str().unwrap_or("");
    
    // Rule 2: Check if it's a folder
    if mime_type != "application/vnd.google-apps.folder" {
        return Err("The provided link is not a folder. Please provide a Google Drive folder link.".to_string());
    }
    
    // Preparation step: Extract instance name from folder name
    let instance_name = folder_name.split_whitespace()
        .next()
        .ok_or("Could not extract instance name from folder name")?;
    
    // Get folder contents
    let mut folder_contents = get_folder_contents(&folder_id, &access_token).await;
    if folder_contents.is_err() {
        folder_contents = get_folder_contents(&folder_id, &access_token).await;
    }
    let folder_contents = folder_contents?;
    
    let files = folder_contents["files"].as_array()
        .ok_or("Invalid folder contents response")?;
    
    // Rule 3: Check for {instance_name}.json file
    let instance_json_name = format!("{}.json", instance_name);
    
    // Debug: List all files found in the folder and debug info
    let file_names: Vec<String> = files.iter()
        .filter_map(|file| file["name"].as_str())
        .map(|name| name.to_string())
        .collect();
    
    let debug_info = folder_contents.get("debug_info")
        .map(|d| format!("Query: {}, Attempt: {}, Files count: {}", 
            d["successful_query"].as_str().unwrap_or("unknown"),
            d["attempt"].as_u64().unwrap_or(0),
            d["files_count"].as_u64().unwrap_or(0)))
        .unwrap_or_else(|| "No debug info".to_string());
    
    let has_instance_json = files.iter().any(|file| {
        let file_name = file["name"].as_str().unwrap_or("");
        let file_mime = file["mimeType"].as_str().unwrap_or("");
        file_name == instance_json_name && file_mime != "application/vnd.google-apps.folder"
    });
    
    if !has_instance_json {
        return Err(format!(
            "Missing required file: {}. Found files: [{}]. Debug: {}",
            instance_json_name,
            file_names.join(", "),
            debug_info
        ));
    }
    
    // Rule 4: Check for logs folder and required log files (case insensitive)
    let logs_folder = files.iter().find(|file| {
        let file_name = file["name"].as_str().unwrap_or("").to_lowercase();
        file_name == "logs" &&
        file["mimeType"].as_str() == Some("application/vnd.google-apps.folder")
    });
    
    let logs_folder_id = match logs_folder {
        Some(folder) => folder["id"].as_str().ok_or("Invalid logs folder ID")?,
        None => return Err("Missing required 'logs' folder (case insensitive search)".to_string()),
    };
    
    // Get logs folder contents
    let mut logs_contents = get_folder_contents(logs_folder_id, &access_token).await;
    if logs_contents.is_err() {
        logs_contents = get_folder_contents(logs_folder_id, &access_token).await;
    }
    let logs_contents = logs_contents?;
    
    let log_files = logs_contents["files"].as_array()
        .ok_or("Invalid logs folder contents response")?;
    
    // Required log file suffixes
    let required_suffixes = vec![
        "_after.log",
        "_before.log", 
        "_base.log",
        "_post_agent_patch.log",
    ];
    
    for suffix in &required_suffixes {
        let suffix_lower = suffix.to_lowercase();
        let has_file = log_files.iter().any(|file| {
            let file_name = file["name"].as_str().unwrap_or("").to_lowercase();
            file_name.ends_with(&suffix_lower) &&
            file["mimeType"].as_str() != Some("application/vnd.google-apps.folder")
        });
        
        if !has_file {
            return Err(format!("Missing required log file ending with: {} (case insensitive search)", suffix));
        }
    }
    
    
    // Now collect all the files we need to download
    let mut files_to_download = Vec::new();
    
    // 1. Add the main {instance_name}.json file
    if let Some(instance_file) = files.iter().find(|file| {
        let file_name = file["name"].as_str().unwrap_or("");
        file_name == instance_json_name
    }) {
        files_to_download.push(FileInfo {
            id: instance_file["id"].as_str().unwrap_or("").to_string(),
            name: instance_file["name"].as_str().unwrap_or("").to_string(),
            path: format!("main/{}", instance_file["name"].as_str().unwrap_or("")),
        });
    }
    
    // 2. Add the 4 log files
    for suffix in &required_suffixes {
        if let Some(log_file) = log_files.iter().find(|file| {
            let file_name = file["name"].as_str().unwrap_or("").to_lowercase();
            file_name.ends_with(&suffix.to_lowercase())
        }) {
            files_to_download.push(FileInfo {
                id: log_file["id"].as_str().unwrap_or("").to_string(),
                name: log_file["name"].as_str().unwrap_or("").to_string(),
                path: format!("logs/{}", log_file["name"].as_str().unwrap_or("")),
            });
        }
    }
    
    
    Ok(ValidationResult {
        files_to_download,
        folder_id: folder_id.to_string(),
    })
}

#[derive(Serialize, Deserialize)]
struct DownloadResult {
    temp_directory: String,
    downloaded_files: Vec<FileInfo>,
}

#[tauri::command]
async fn download_deliverable(files_to_download: Vec<FileInfo>, folder_id: String) -> Result<DownloadResult, String> {
    use reqwest::header::AUTHORIZATION;
    use std::fs;
    use tempfile::TempDir;
    
    // Create a temporary directory
    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp directory: {}", e))?;
    let temp_path = temp_dir.path().to_string_lossy().to_string();
    
    // Load tokens for API access
    let path = tokens_path();
    if !path.exists() {
        return Err("Please authenticate with Google Drive first".to_string());
    }
    
    let data = std::fs::read_to_string(&path)
        .map_err(|e| format!("Token read error: {}", e))?;
    let mut tokens: GoogleTokens = serde_json::from_str(&data)
        .map_err(|e| format!("Token parse error: {}", e))?;
    let mut access_token = tokens.access_token.clone();
    
    // We need to persist the temp directory. Use folder_id as the subfolder name for caching
    let base_temp_dir = std::path::Path::new(&temp_path).parent().unwrap().join("swe-reviewer-temp");
    // Create base temp directory if it doesn't exist
    if !base_temp_dir.exists() {
        fs::create_dir_all(&base_temp_dir).map_err(|e| format!("Failed to create base temp dir: {}", e))?;
    }
    
    // Use folder_id as the subfolder name to allow caching and concurrent usage
    let persist_dir = base_temp_dir.join(&folder_id);
    
    // Check if we already have this deliverable downloaded
    if persist_dir.exists() {
        // Return the cached result
        let mut cached_files = Vec::new();
        for file_info in &files_to_download {
            let cached_file_path = persist_dir.join(&file_info.path);
            if cached_file_path.exists() {
                cached_files.push(FileInfo {
                    id: file_info.id.clone(),
                    name: file_info.name.clone(),
                    path: cached_file_path.to_string_lossy().to_string(),
                });
            }
        }
        
        if !cached_files.is_empty() {
            return Ok(DownloadResult {
                temp_directory: persist_dir.to_string_lossy().to_string(),
                downloaded_files: cached_files,
            });
        }
    }
    
    let mut downloaded_files = Vec::new();
    let client = reqwest::Client::new();
    
    for file_info in files_to_download {
        // Create subdirectories if needed
        let file_path = std::path::Path::new(&temp_path).join(&file_info.path);
        let file_dir_path = file_path.parent().unwrap_or(std::path::Path::new(""));
        if !file_dir_path.exists() {
            fs::create_dir_all(&file_dir_path)
                .map_err(|e| format!("Failed to create directory {}: {}", file_dir_path.display(), e))?;
        }
        
        // Download file content
        let download_url = format!("https://www.googleapis.com/drive/v3/files/{}?alt=media&supportsAllDrives=true", file_info.id);
        let mut file_resp = client
            .get(&download_url)
            .header(AUTHORIZATION, format!("Bearer {}", access_token))
            .send()
            .await
            .map_err(|e| format!("Download error for {}: {}", file_info.name, e))?;
            
        if file_resp.status() == 403 || file_resp.status() == 401 {
            // Try refresh
            tokens = refresh_access_token(&tokens).await?;
            access_token = tokens.access_token.clone();
            let _ = save_google_tokens(tokens.clone());
            // Retry
            file_resp = client
                .get(&download_url)
                .header(AUTHORIZATION, format!("Bearer {}", access_token))
                .send()
                .await
                .map_err(|e| format!("Download error for {}: {}", file_info.name, e))?;
        }
        
        if !file_resp.status().is_success() {
            return Err(format!("Failed to download file {}: {}", file_info.name, file_resp.status()));
        }
        
        let content = file_resp.bytes().await
            .map_err(|e| format!("File read error for {}: {}", file_info.name, e))?;
        
        // Write file to temp directory
        fs::write(&file_path, content)
            .map_err(|e| format!("Failed to write file {}: {}", file_info.name, e))?;
        
        downloaded_files.push(FileInfo {
            id: file_info.id,
            name: file_info.name,
            path: file_path.to_string_lossy().to_string(),
        });
    }
    
    // Move temp contents to persistent location
    fs::create_dir_all(&persist_dir).map_err(|e| format!("Failed to create persist dir: {}", e))?;
    
    // Copy all files to the persistent directory
    for file_info in &downloaded_files {
        let source = std::path::Path::new(&file_info.path);
        let relative_path = source.strip_prefix(&temp_path).unwrap();
        let dest = persist_dir.join(relative_path);
        
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create dest dir: {}", e))?;
        }
        
        fs::copy(source, &dest).map_err(|e| format!("Failed to copy file: {}", e))?;
    }
    
    // Update file paths to point to persistent directory
    let mut updated_files = Vec::new();
    for file_info in downloaded_files {
        let source = std::path::Path::new(&file_info.path);
        let relative_path = source.strip_prefix(&temp_path).unwrap();
        let new_path = persist_dir.join(relative_path);
        
        updated_files.push(FileInfo {
            id: file_info.id,
            name: file_info.name,
            path: new_path.to_string_lossy().to_string(),
        });
    }
    
    Ok(DownloadResult {
        temp_directory: persist_dir.to_string_lossy().to_string(),
        downloaded_files: updated_files,
    })
}

#[tauri::command]
async fn process_deliverable(downloaded_files: Vec<FileInfo>) -> Result<serde_json::Value, String> {
    // Dummy processing with 5 second delay
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    
    // For now, just pass the file paths to the result
    // Later, this will do actual processing
    let file_paths: Vec<String> = downloaded_files.iter().map(|f| f.path.clone()).collect();
    
    // Simulate processing results
    Ok(serde_json::json!({
        "status": "completed",
        "message": "Report processing completed successfully",
        "files_processed": downloaded_files.len(),
        "issues_found": 3,
        "score": 85,
        "file_paths": file_paths
    }))
}

#[tauri::command]
async fn get_file_content(file_type: String, file_paths: Vec<String>) -> Result<String, String> {
    // Find the file with the matching type in the file paths
    let file_extensions = match file_type.as_str() {
        "base" => vec!["base.log", "base.txt"],
        "before" => vec!["before.log", "before.txt"],
        "after" => vec!["after.log", "after.txt"],
        "agent" => vec!["post_agent_patch"],
        "main_json" => vec!["main/", "report.json", "summary.json"],
        "report" => vec!["report.json", "analysis.json", "results.json"],
        _ => return Err(format!("Unknown file type: {}", file_type)),
    };

    // Look for a file that matches the expected extensions
    for path in &file_paths {
        let path_lower = path.to_lowercase();
        for extension in &file_extensions {
            if path_lower.contains(extension) {
                match fs::read_to_string(path) {
                    Ok(content) => return Ok(content),
                    Err(e) => {
                        eprintln!("Failed to read file {}: {}", path, e);
                        continue;
                    }
                }
            }
        }
    }
    
    // If no file found, return a placeholder message
    Ok(format!("No {} file found in the provided paths", file_type))
}

#[derive(Serialize, Deserialize)]
struct AnalysisResult {
    status: String,
    message: String,
    analysis_files: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize)]
struct TestItem {
    test_name: String,
    status: String, // "success" or "fail"
    occurences: u32,
}

#[derive(Serialize, Deserialize)]
struct TestStatus {
    test_name: String,
    status: String, // "passed", "failed", or "non_existing"
    r#type: String, // "fail_to_pass" or "pass_to_pass"
}

// Temporary struct for parsing AI response without type field
#[derive(Serialize, Deserialize)]
struct TestStatusWithoutType {
    test_name: String,
    status: String,
}

// Struct for parsing structured OpenAI response
#[derive(Serialize, Deserialize)]
struct StructuredTestResponse {
    test_results: Vec<TestStatusWithoutType>,
}

#[tauri::command]
async fn analyze_files(file_paths: Vec<String>) -> Result<AnalysisResult, String> {
    println!("Starting analysis with file paths: {:?}", file_paths);
    
    // Step 1: Find and parse main.json
    let main_json_path = file_paths.iter()
        .find(|path| path.to_lowercase().contains("main/"))
        .ok_or("main.json file not found in provided paths".to_string())?;
    
    println!("Found main.json at: {}", main_json_path);
    
    let main_json_content = fs::read_to_string(main_json_path)
        .map_err(|e| format!("Failed to read main.json: {}", e))?;
    
    println!("Parsing main.json content...");
    let main_json: serde_json::Value = serde_json::from_str(&main_json_content)
        .map_err(|e| format!("Failed to parse main.json: {}", e))?;
    
    // Extract fail_to_pass and pass_to_pass arrays
    let fail_to_pass = main_json.get("fail_to_pass")
        .and_then(|v| v.as_array())
        .ok_or("Missing or invalid fail_to_pass array in main.json".to_string())?;
    
    let pass_to_pass = main_json.get("pass_to_pass")
        .and_then(|v| v.as_array())
        .ok_or("Missing or invalid pass_to_pass array in main.json".to_string())?;
    
    // Convert to string arrays
    let mut all_tests = Vec::new();
    for test in fail_to_pass {
        if let Some(test_name) = test.as_str() {
            all_tests.push(("fail_to_pass", test_name.to_string()));
        }
    }
    for test in pass_to_pass {
        if let Some(test_name) = test.as_str() {
            all_tests.push(("pass_to_pass", test_name.to_string()));
        }
    }
    
    if all_tests.is_empty() {
        return Ok(AnalysisResult {
            status: "rejected".to_string(),
            message: "Rejected: No tests found in main.json".to_string(),
            analysis_files: None,
        });
    }
    
    println!("Found {} tests to analyze", all_tests.len());
    
    // Find log files
    let base_log = file_paths.iter().find(|path| path.to_lowercase().contains("base.log"));
    let before_log = file_paths.iter().find(|path| path.to_lowercase().contains("before.log"));
    let after_log = file_paths.iter().find(|path| path.to_lowercase().contains("after.log"));
    
    let mut analysis_files = Vec::new();
    
    // Load OpenAI API token from settings
    println!("Loading OpenAI API token from settings...");
    let openai_token = load_setting("openai_api_key".to_string())?;
    if openai_token.is_empty() {
        println!("OpenAI API token is empty!");
        return Err("OpenAI API token not found in settings. Please configure it first.".to_string());
    }
    println!("OpenAI API token loaded successfully (length: {})", openai_token.len());
    
    // Process each log file with OpenAI
    if let Some(base_path) = base_log {
        println!("Processing base log: {}", base_path);
        let output_path = base_path.replace(".log", ".json");
        println!("Output path will be: {}", output_path);
        analyze_log_with_openai_new(base_path, &output_path, &openai_token, &all_tests).await?;
        analysis_files.push(output_path);
        println!("Successfully processed base log");
    }
    
    if let Some(before_path) = before_log {
        let output_path = before_path.replace(".log", ".json");
        analyze_log_with_openai_new(before_path, &output_path, &openai_token, &all_tests).await?;
        analysis_files.push(output_path);
    }
    
    if let Some(after_path) = after_log {
        let output_path = after_path.replace(".log", ".json");
        analyze_log_with_openai_new(after_path, &output_path, &openai_token, &all_tests).await?;
        analysis_files.push(output_path);
    }
    
    println!("Analysis completed successfully! Generated {} analysis files", analysis_files.len());
    println!("Analysis file paths: {:?}", analysis_files);
    
    Ok(AnalysisResult {
        status: "accepted".to_string(),
        message: "Analysis completed successfully".to_string(),
        analysis_files: Some(analysis_files),
    })
}

async fn analyze_log_with_openai_new(
    log_path: &str,
    output_path: &str,
    openai_token: &str,
    all_tests: &[(&str, String)],
) -> Result<(), String> {
    println!("Starting new OpenAI analysis for log: {}", log_path);
    
    // Read the log file
    let log_content = fs::read_to_string(log_path)
        .map_err(|e| format!("Failed to read log file {}: {}", log_path, e))?;
    
    println!("Log file read successfully, size: {} bytes", log_content.len());
    
    // Truncate log content if it's too long (Gemini has limits)
    let max_log_length = 100000; // 100KB limit
    let truncated_log = if log_content.len() > max_log_length {
        println!("Log content too long ({} bytes), truncating to {} bytes", log_content.len(), max_log_length);
        log_content.chars().take(max_log_length).collect::<String>()
    } else {
        log_content
    };
    
    // Construct the prompt for OpenAI
    println!("Constructing prompt...");
    let test_list: Vec<String> = all_tests.iter().map(|(_, name)| name.clone()).collect();
    let prompt = format!(
        "You are a test log analyzer. Analyze the following log content and determine the status of each test from the provided list.

Test list to analyze: {}

For each test, determine if it:
- passed: the test completed successfully
- failed: the test completed with failures
- non_existing: the test does not appear in the log at all

Return a JSON object with a 'test_results' array containing objects in this exact format:
{{\"test_results\": [{{\"test_name\": \"string\", \"status\": \"passed\" or \"failed\" or \"non_existing\"}}]}}

Log content to analyze:
{}",
        serde_json::to_string(&test_list).unwrap_or_default(),
        truncated_log
    );
    
    println!("Prompt constructed, length: {} characters", prompt.len());
    
    // Use manual OpenAI API call
    println!("Calling OpenAI API with GPT-4o...");
    
    // Create HTTP client
    let client = reqwest::Client::new();
    
    // Create the request body for OpenAI with structured output
    let request_body = serde_json::json!({
        "model": "gpt-4o-mini",
        "messages": [
            {
                "role": "user",
                "content": prompt
            }
        ],
        "temperature": 0.1,
        "max_tokens": 8192,
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "test_status_response",
                "strict": true,
                "schema": {
                    "type": "object",
                    "properties": {
                        "test_results": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "test_name": {
                                        "type": "string",
                                        "description": "The name of the test"
                                    },
                                    "status": {
                                        "type": "string",
                                        "enum": ["passed", "failed", "non_existing"],
                                        "description": "The status of the test"
                                    }
                                },
                                "required": ["test_name", "status"],
                                "additionalProperties": false
                            }
                        }
                    },
                    "required": ["test_results"],
                    "additionalProperties": false
                }
            }
        }
    });
    
    println!("Making OpenAI API call...");
    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", openai_token))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Failed to call OpenAI API: {}", e))?;
    
    let status = response.status();
    println!("OpenAI API responded with status: {}", status);
    
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();
        println!("Error response body: {}", error_body);
        return Err(format!("OpenAI API error: {} - {}", status, error_body));
    }
    
    let response_json: serde_json::Value = response.json().await
        .map_err(|e| format!("Failed to parse OpenAI response: {}", e))?;
    
    // Extract the content from the response
    let content = response_json
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .ok_or("Failed to extract content from OpenAI response")?;
    
    println!("Response extracted, length: {} characters", content.len());
    
    // Debug: Print the first 200 characters of the response
    let preview = if content.len() > 200 {
        format!("{}...", &content[..200])
    } else {
        content.to_string()
    };
    println!("Response preview: {}", preview);
    
    // Try to parse and validate the JSON response, then enhance with type information
    println!("Attempting to parse JSON response...");
    let final_content = match serde_json::from_str::<StructuredTestResponse>(&content) {
        Ok(structured_response) => {
            let test_statuses_raw = structured_response.test_results;
            println!("Successfully parsed {} test status items", test_statuses_raw.len());
            
            // Create a lookup map for test types
            let mut test_type_map = std::collections::HashMap::new();
            for (test_type, test_name) in all_tests {
                test_type_map.insert(test_name.clone(), test_type.to_string());
            }
            
            // Enhance each test status with type information
            let enhanced_test_statuses: Vec<TestStatus> = test_statuses_raw.into_iter().map(|ts| {
                let test_type = test_type_map.get(&ts.test_name)
                    .unwrap_or(&"unknown".to_string())
                    .clone();
                TestStatus {
                    test_name: ts.test_name,
                    status: ts.status,
                    r#type: test_type,
                }
            }).collect();
            
            // Validate that all tests from our list are present in the response
            let mut missing_tests = Vec::new();
            for (_, expected_test) in all_tests {
                if !enhanced_test_statuses.iter().any(|ts| ts.test_name == *expected_test) {
                    missing_tests.push(expected_test.clone());
                }
            }
            
            if !missing_tests.is_empty() {
                println!("Warning: Missing tests in response: {:?}", missing_tests);
                // For now, we'll continue but this could be enhanced with retry logic
            }
            
            // Convert enhanced statuses back to JSON
            match serde_json::to_string_pretty(&enhanced_test_statuses) {
                Ok(enhanced_json) => {
                    println!("Successfully enhanced response with type information");
                    enhanced_json
                },
                Err(e) => {
                    println!("Warning: Failed to serialize enhanced response: {}", e);
                    println!("Falling back to original response");
                    content.to_string()
                }
            }
        },
        Err(e) => {
            println!("Warning: Failed to parse LLM response as JSON: {}", e);
            println!("This is not fatal - we'll save the raw response");
            content.to_string()
        }
    };
    
    // Write the enhanced JSON to the output file
    println!("Writing analysis results to: {}", output_path);
    
    // Ensure the directory exists
    if let Some(parent) = std::path::Path::new(&output_path).parent() {
        if !parent.exists() {
            println!("Creating directory: {:?}", parent);
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {:?}: {}", parent, e))?;
        }
    }
    
    match fs::write(&output_path, &final_content) {
        Ok(_) => println!("Successfully wrote analysis file: {}", output_path),
        Err(e) => {
            println!("Error writing file: {}", e);
            return Err(format!("Failed to write analysis file {}: {}", output_path, e));
        }
    }
    
    println!("Successfully completed OpenAI analysis for: {}", log_path);
    Ok(())
}


#[tauri::command]
fn read_analysis_file(file_path: String) -> Result<String, String> {
    println!("Attempting to read analysis file: {}", file_path);
    
    // Check if file exists
    if !std::path::Path::new(&file_path).exists() {
        println!("File does not exist: {}", file_path);
        return Err(format!("File does not exist: {}", file_path));
    }
    
    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read analysis file {}: {}", file_path, e))?;
    
    println!("Successfully read analysis file: {} ({} bytes)", file_path, content.len());
    Ok(content)
}

#[tauri::command]
fn debug_settings() -> Result<String, String> {
    let settings_path = settings_path();
    if !settings_path.exists() {
        return Ok("Settings file does not exist".to_string());
    }
    
    let content = fs::read_to_string(&settings_path)
        .map_err(|e| format!("Failed to read settings: {}", e))?;
    
    Ok(format!("Settings file path: {:?}\nContent: {}", settings_path, content))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_auth_state,
            get_google_client_id,
            save_google_tokens,
            download_drive_file,
            upload_drive_file,
            save_setting,
            load_setting,
            logout,
            validate_deliverable,
            download_deliverable,
            process_deliverable,
            get_file_content,
            analyze_files,
            read_analysis_file,
            debug_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
