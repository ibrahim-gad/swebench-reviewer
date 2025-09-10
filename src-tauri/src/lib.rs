// Module declarations
mod auth;
mod drive;
mod settings;
mod report_checker;
mod analysis;

// Re-export commonly used types
pub use auth::GoogleTokens;
pub use report_checker::{FileInfo, ValidationResult, DownloadResult};
pub use analysis::{AnalysisResult, TestLists, LogSearchResults};

// Tauri command entry points - Authentication
#[tauri::command]
fn get_auth_state() -> Result<Option<String>, String> {
    auth::get_auth_state()
}

#[tauri::command]
fn get_google_client_secret() -> Result<String, String> {
    auth::get_google_client_secret()
}

#[tauri::command]
fn save_google_tokens(tokens: GoogleTokens) -> Result<(), String> {
    auth::save_google_tokens(tokens)
}

#[tauri::command]
fn logout() -> Result<(), String> {
    auth::logout()
}

// Tauri command entry points - Google Drive
#[tauri::command]
async fn download_drive_file(link: String) -> Result<serde_json::Value, String> {
    drive::download_drive_file(link).await
}

#[tauri::command]
async fn upload_drive_file(link: String, content: String) -> Result<(), String> {
    drive::upload_drive_file(link, content).await
}

// Tauri command entry points - Settings
#[tauri::command]
fn save_setting(key: String, value: String) -> Result<(), String> {
    settings::save_setting(key, value)
}

#[tauri::command]
fn load_setting(key: String) -> Result<String, String> {
    settings::load_setting(key)
}

#[tauri::command]
fn debug_settings() -> Result<String, String> {
    settings::debug_settings()
}

#[tauri::command]
fn get_temp_dir_size() -> Result<u64, String> {
    settings::get_temp_dir_size()
}

#[tauri::command]
fn clear_temp_dir() -> Result<(), String> {
    settings::clear_temp_dir()
}

// Tauri command entry points - Report Checker
#[tauri::command]
async fn validate_deliverable(folder_link: String) -> Result<ValidationResult, String> {
    report_checker::validate_deliverable(folder_link).await
}

#[tauri::command]
async fn download_deliverable(files_to_download: Vec<FileInfo>, folder_id: String) -> Result<DownloadResult, String> {
    report_checker::download_deliverable(files_to_download, folder_id).await
}

#[tauri::command]
async fn process_deliverable(downloaded_files: Vec<FileInfo>) -> Result<serde_json::Value, String> {
    report_checker::process_deliverable(downloaded_files).await
}

#[tauri::command]
fn get_file_content(file_type: String, file_paths: Vec<String>) -> Result<String, String> {
    report_checker::get_file_content(file_type, file_paths)
}

// Tauri command entry points - Analysis
#[tauri::command]
async fn analyze_files(file_paths: Vec<String>) -> Result<AnalysisResult, String> {
    analysis::analyze_files(file_paths).await
}

#[tauri::command]
fn read_analysis_file(file_path: String) -> Result<String, String> {
    analysis::read_analysis_file(file_path)
}

#[tauri::command]
fn get_test_lists(file_paths: Vec<String>) -> Result<TestLists, String> {
    analysis::get_test_lists(file_paths)
}

#[tauri::command]
fn search_logs(file_paths: Vec<String>, test_name: String) -> Result<LogSearchResults, String> {
    analysis::search_logs(file_paths, test_name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_auth_state,
            get_google_client_secret,
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
            get_test_lists,
            search_logs,
            debug_settings,
            get_temp_dir_size,
            clear_temp_dir
        ])
        .on_window_event(|_window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                // Clear temp directory when app is closing
                if let Err(e) = settings::clear_temp_dir() {
                    eprintln!("Failed to clear temp directory on app close: {}", e);
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}