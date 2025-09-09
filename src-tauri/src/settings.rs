use std::fs;
use std::path::PathBuf;
use dirs;

pub fn settings_path() -> PathBuf {
    let mut home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.push(".swe-reviewer");
    if !home.exists() {
        let _ = fs::create_dir_all(&home);
    }
    home.join("settings.json")
}

pub fn save_setting(key: String, value: String) -> Result<(), String> {
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

pub fn load_setting(key: String) -> Result<String, String> {
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

pub fn debug_settings() -> Result<String, String> {
    let settings_path = settings_path();
    if !settings_path.exists() {
        return Ok("Settings file does not exist".to_string());
    }
    
    let content = fs::read_to_string(&settings_path)
        .map_err(|e| format!("Failed to read settings: {}", e))?;
    
    Ok(format!("Settings file path: {:?}\nContent: {}", settings_path, content))
}
