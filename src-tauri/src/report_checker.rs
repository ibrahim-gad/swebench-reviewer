use serde::{Deserialize, Serialize};
use std::fs;
use tempfile::TempDir;
use crate::auth::{GoogleTokens, tokens_path, save_google_tokens, refresh_access_token};
use crate::drive::{extract_drive_folder_id, get_folder_contents, get_folder_metadata};
// load_setting is not used in this module

#[derive(Serialize, Deserialize)]
pub struct FileInfo {
    pub id: String,
    pub name: String,
    pub path: String,
}

#[derive(Serialize, Deserialize)]
pub struct ValidationResult {
    pub files_to_download: Vec<FileInfo>,
    pub folder_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct DownloadResult {
    pub temp_directory: String,
    pub downloaded_files: Vec<FileInfo>,
}

pub async fn validate_deliverable(folder_link: String) -> Result<ValidationResult, String> {
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
    
    // Rule 5: Check for results folder (optional, but if present, we'll use it)
    let results_folder = files.iter().find(|file| {
        let file_name = file["name"].as_str().unwrap_or("").to_lowercase();
        file_name == "results" &&
        file["mimeType"].as_str() == Some("application/vnd.google-apps.folder")
    });
    
    println!("Results folder found: {}", results_folder.is_some());

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
    
    // 3. Add report.json from results folder if it exists
    if let Some(results_folder) = results_folder {
        println!("Found results folder, attempting to get contents...");
        let results_folder_id = results_folder["id"].as_str().unwrap_or("");
        
        // Get results folder contents
        let mut results_contents = get_folder_contents(results_folder_id, &access_token).await;
        if results_contents.is_err() {
            println!("First attempt to get results folder contents failed, retrying...");
            results_contents = get_folder_contents(results_folder_id, &access_token).await;
        }
        
        if let Ok(results_contents) = results_contents {
            let empty_vec = vec![];
            let results_files = results_contents["files"].as_array().unwrap_or(&empty_vec);
            println!("Found {} files in results folder", results_files.len());
            
            // Debug: List all files in results folder
            for file in results_files {
                let file_name = file["name"].as_str().unwrap_or("unknown");
                println!("Results folder file: {}", file_name);
            }
            
            // Look for report.json file
            if let Some(report_file) = results_files.iter().find(|file| {
                let file_name = file["name"].as_str().unwrap_or("").to_lowercase();
                file_name == "report.json" &&
                file["mimeType"].as_str() != Some("application/vnd.google-apps.folder")
            }) {
                println!("Found report.json file in results folder, adding to download list");
                files_to_download.push(FileInfo {
                    id: report_file["id"].as_str().unwrap_or("").to_string(),
                    name: report_file["name"].as_str().unwrap_or("").to_string(),
                    path: format!("results/{}", report_file["name"].as_str().unwrap_or("")),
                });
            } else {
                println!("No report.json file found in results folder");
            }
        } else {
            println!("Failed to get results folder contents: {:?}", results_contents.err());
        }
    } else {
        println!("No results folder found in the deliverable");
    }
    
    
    Ok(ValidationResult {
        files_to_download,
        folder_id: folder_id.to_string(),
    })
}

pub async fn download_deliverable(files_to_download: Vec<FileInfo>, folder_id: String) -> Result<DownloadResult, String> {
    use reqwest::header::AUTHORIZATION;
    
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
        // Check if ALL files exist in cache, not just some
        let mut cached_files = Vec::new();
        let mut all_files_cached = true;
        
        for file_info in &files_to_download {
            let cached_file_path = persist_dir.join(&file_info.path);
            if cached_file_path.exists() {
                cached_files.push(FileInfo {
                    id: file_info.id.clone(),
                    name: file_info.name.clone(),
                    path: cached_file_path.to_string_lossy().to_string(),
                });
            } else {
                println!("Cache miss: file not found at {}", cached_file_path.display());
                all_files_cached = false;
                break;
            }
        }
        
        // Only return cached result if ALL files are present
        if all_files_cached && !cached_files.is_empty() {
            println!("All {} files found in cache, returning cached result", cached_files.len());
            return Ok(DownloadResult {
                temp_directory: persist_dir.to_string_lossy().to_string(),
                downloaded_files: cached_files,
            });
        } else {
            println!("Cache incomplete, will re-download all files");
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
        
        println!("Copying file: {} -> {}", source.display(), dest.display());
        
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create dest dir: {}", e))?;
        }
        
        fs::copy(source, &dest).map_err(|e| format!("Failed to copy file {}: {}", file_info.name, e))?;
        println!("Successfully copied: {}", file_info.name);
    }
    
    // Update file paths to point to persistent directory
    let mut updated_files = Vec::new();
    for file_info in downloaded_files {
        let source = std::path::Path::new(&file_info.path);
        let relative_path = source.strip_prefix(&temp_path).unwrap();
        let new_path = persist_dir.join(relative_path);
        
        println!("Updating file path: {} -> {}", file_info.path, new_path.display());
        
        updated_files.push(FileInfo {
            id: file_info.id,
            name: file_info.name.clone(),
            path: new_path.to_string_lossy().to_string(),
        });
        println!("Added to final result: {} at {}", file_info.name, new_path.display());
    }
    
    println!("Final download result contains {} files:", updated_files.len());
    for file in &updated_files {
        println!("  - {}: {}", file.name, file.path);
    }
    
    Ok(DownloadResult {
        temp_directory: persist_dir.to_string_lossy().to_string(),
        downloaded_files: updated_files,
    })
}

pub async fn process_deliverable(downloaded_files: Vec<FileInfo>) -> Result<serde_json::Value, String> {
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

pub fn get_file_content(file_type: String, file_paths: Vec<String>) -> Result<String, String> {
    println!("=== GET_FILE_CONTENT DEBUG ===");
    println!("Looking for file type '{}' in paths:", file_type);
    for (i, path) in file_paths.iter().enumerate() {
        println!("  [{}]: {}", i, path);
    }
    
    // Find the file with the matching type in the file paths
    let file_extensions = match file_type.as_str() {
        "base" => vec!["base.log", "base.txt"],
        "before" => vec!["before.log", "before.txt"],
        "after" => vec!["after.log", "after.txt"],
        "agent" => vec!["post_agent_patch"],
        "main_json" => vec!["main/", "report.json", "summary.json"],
        "report" => vec!["results/report.json", "report.json", "results/", "analysis.json", "results.json"],
        _ => return Err(format!("Unknown file type: {}", file_type)),
    };

    println!("Using search patterns for '{}': {:?}", file_type, file_extensions);

    // Look for a file that matches the expected extensions
    for path in &file_paths {
        let path_lower = path.to_lowercase();
        println!("Checking path: {} (lowercase: {})", path, path_lower);
        
        for extension in &file_extensions {
            let extension_lower = extension.to_lowercase();
            println!("  Testing pattern '{}' against path", extension_lower);
            
            if path_lower.contains(&extension_lower) {
                println!("✓ MATCH FOUND: {} contains '{}'", path, extension_lower);
                match fs::read_to_string(path) {
                    Ok(content) => {
                        println!("✓ Successfully read file: {} ({} bytes)", path, content.len());
                        println!("=== END GET_FILE_CONTENT DEBUG ===");
                        return Ok(content);
                    },
                    Err(e) => {
                        eprintln!("✗ Failed to read file {}: {}", path, e);
                        continue;
                    }
                }
            } else {
                println!("  ✗ No match: '{}' not found in '{}'", extension_lower, path_lower);
            }
        }
    }

    // If no file found, return a placeholder message
    println!("✗ No {} file found in the provided paths", file_type);
    println!("=== END GET_FILE_CONTENT DEBUG ===");
    Ok(format!("No {} file found in the provided paths", file_type))
}
