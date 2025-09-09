// No serde imports needed in this module
use crate::auth::{GoogleTokens, tokens_path, save_google_tokens, refresh_access_token};

// Utility to extract Google Drive file ID from a link
pub fn extract_drive_file_id(link: &str) -> Option<String> {
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
pub fn extract_drive_folder_id(link: &str) -> Option<String> {
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
pub async fn get_shared_drives(access_token: &str) -> Result<Vec<(String, String)>, String> {
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
pub async fn get_folder_contents(folder_id: &str, access_token: &str) -> Result<serde_json::Value, String> {
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
pub async fn get_folder_metadata(folder_id: &str, access_token: &str) -> Result<serde_json::Value, String> {
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

pub async fn download_drive_file(link: String) -> Result<serde_json::Value, String> {
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

pub async fn upload_drive_file(link: String, content: String) -> Result<(), String> {
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
