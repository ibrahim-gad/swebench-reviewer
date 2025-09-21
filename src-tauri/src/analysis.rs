use serde::{Deserialize, Serialize};
use std::fs;
use crate::settings::load_setting;
use lazy_static::lazy_static;
use regex::Regex;
use std::cmp::min;

// Compile regex patterns once at module level to avoid repeated compilation
lazy_static! {
    // Case-insensitive, include error status, allow trailing whitespace
    static ref TEST_LINE_RE: Regex = Regex::new(r"(?i)\btest\s+(.+?)\s+\.\.\.\s+(ok|FAILED|ignored|error)\s*$")
        .expect("Failed to compile TEST_LINE_RE regex");

    static ref TEST_START_RE: Regex = Regex::new(r"(?i)\btest\s+(.+?)\s+\.\.\.\s*(.*?)$")
        .expect("Failed to compile TEST_START_RE regex");

    static ref STATUS_RE: Regex = Regex::new(r"(?i)\b(ok|failed|ignored|error)\b")
        .expect("Failed to compile STATUS_RE regex");

    static ref STATUS_AT_END_RE: Regex = Regex::new(r"(?i)\b(ok|failed|ignored|error)\s*$")
        .expect("Failed to compile STATUS_AT_END_RE regex");

    static ref ANOTHER_TEST_RE: Regex = Regex::new(r"(?i)\btest\s+[\w:]+\s+\.\.\.\s*")
        .expect("Failed to compile ANOTHER_TEST_RE regex");

    static ref TEST_WITH_O_RE: Regex = Regex::new(r"(?i)\btest\s+([\w:]+(?:::\w+)*)\s+\.\.\.\s*o\s*$")
        .expect("Failed to compile TEST_WITH_O_RE regex");

    static ref TEST_STARTS_RE: Regex = Regex::new(r"(?i)\btest\s+([\w:]+(?:::\w+)*)\s+\.\.\.\s*")
        .expect("Failed to compile TEST_STARTS_RE regex");

    static ref STATUS_IN_TEXT_RE: Regex = Regex::new(r"(?i)\b(ok|failed|ignored|error)\b")
        .expect("Failed to compile STATUS_IN_TEXT_RE regex");

    // Additional patterns
    static ref CORRUPTED_TEST_LINE_RE: Regex = Regex::new(r"(?i)(?:line)?test\s+([\w:]+(?:::\w+)*)\s+\.\.\.\s*")
        .expect("Failed to compile CORRUPTED_TEST_LINE_RE regex");

    // File boundary hints
    static ref FILE_BOUNDARY_RE_1: Regex = Regex::new(r"(?i)Running\s+([^/\s]+(?:/[^/\s]+)*\.rs)\s*\(").unwrap();
    static ref FILE_BOUNDARY_RE_2: Regex = Regex::new(r"(?i)===\s*Running\s+(.+\.rs)").unwrap();
    static ref FILE_BOUNDARY_RE_3: Regex = Regex::new(r"(?i)test\s+result:\s+ok\.\s+\d+\s+passed.*for\s+(.+\.rs)").unwrap();

    // Enhanced extraction patterns
    static ref ENH_TEST_RE_1: Regex = Regex::new(r"(?i)\btest\s+([^\s.]+(?:::[^\s.]+)*)\s*\.{2,}\s*(ok|FAILED|ignored|error)").unwrap();
    static ref ENH_TEST_RE_2: Regex = Regex::new(r"(?i)test\s+([a-zA-Z_][a-zA-Z0-9_:]*)\s+\.\.\.\s+(ok|FAILED|ignored|error)").unwrap();

    // ANSI escape detection
    static ref ANSI_RE: Regex = Regex::new(r"\x1B(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~])").unwrap();

    static ref FAILURES_BLOCK_RE: Regex = Regex::new(r"^\s{4}(.+?)\s*$")
        .expect("Failed to compile FAILURES_BLOCK_RE regex");
}

#[derive(Serialize, Deserialize)]
pub struct AnalysisResult {
    pub status: String,
    pub message: String,
    pub analysis_files: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize)]
pub struct TestItem {
    pub test_name: String,
    pub status: String, // "success" or "fail"
    pub occurences: u32,
}

#[derive(Serialize, Deserialize)]
pub struct TestStatus {
    pub test_name: String,
    pub status: String, // "passed", "failed", or "non_existing"
    pub r#type: String, // "fail_to_pass" or "pass_to_pass"
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

pub async fn analyze_files(file_paths: Vec<String>) -> Result<AnalysisResult, String> {
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
        analyze_log_with_openai(base_path, &output_path, &openai_token, &all_tests).await?;
        analysis_files.push(output_path);
        println!("Successfully processed base log");
    }
    
    if let Some(before_path) = before_log {
        let output_path = before_path.replace(".log", ".json");
        analyze_log_with_openai(before_path, &output_path, &openai_token, &all_tests).await?;
        analysis_files.push(output_path);
    }
    
    if let Some(after_path) = after_log {
        let output_path = after_path.replace(".log", ".json");
        analyze_log_with_openai(after_path, &output_path, &openai_token, &all_tests).await?;
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

// Helper function to chunk log content into manageable pieces
fn chunk_log_content(log_content: &str, chunk_size: usize) -> Vec<String> {
    if log_content.len() <= chunk_size {
        return vec![log_content.to_string()];
    }
    
    let mut chunks = Vec::new();
    let mut start = 0;
    
    while start < log_content.len() {
        let potential_end = start + chunk_size;
        
        let end = if potential_end >= log_content.len() {
            // Last chunk - take everything remaining
            log_content.len()
        } else {
            // Find the best split point within the chunk size
            let search_start = start + (chunk_size * 3 / 4); // Start looking from 75% of chunk size
            let search_end = potential_end;
            
            // Look for newlines in the last 25% of the chunk
            if let Some(newline_pos) = log_content[search_start..search_end].rfind('\n') {
                search_start + newline_pos + 1
            } else {
                // If no newline found in the last 25%, look in the entire chunk
                if let Some(newline_pos) = log_content[start..search_end].rfind('\n') {
                    start + newline_pos + 1
                } else {
                    // If absolutely no newline found, split at chunk boundary
                    // This should be rare for log files
                    potential_end
                }
            }
        };
        
        // Ensure we don't create empty chunks
        if end > start {
            chunks.push(log_content[start..end].to_string());
        }
        start = end;
        
        // Safety check to prevent infinite loops
        if start >= log_content.len() {
            break;
        }
    }
    
    chunks
}

// Helper function to process a single chunk with OpenAI
async fn process_log_chunk(
    chunk: &str,
    openai_token: &str,
    all_tests: &[(&str, String)],
    chunk_number: usize,
) -> Result<Vec<TestStatusWithoutType>, String> {
    println!("Processing chunk {} with OpenAI...", chunk_number);
    
    // Construct the prompt for this chunk
    let test_list: Vec<String> = all_tests.iter().map(|(_, name)| name.clone()).collect();
    let prompt = format!(
        "You are a test log analyzer. Analyze the following log content chunk and determine the status of each test from the provided list.

Test list to analyze: {}

For each test, determine if it:
- passed: the test completed successfully
- failed: the test completed with failures
- non_existing: the test does not appear in the log at all

Return a JSON object with a 'test_results' array containing objects in this exact format:
{{\"test_results\": [{{\"test_name\": \"string\", \"status\": \"passed\" or \"failed\" or \"non_existing\"}}]}}

Log content chunk to analyze:
{}",
        serde_json::to_string(&test_list).unwrap_or_default(),
        chunk
    );
    
    // Create HTTP client with timeout
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(240)) // 4 minute timeout
        .build()
        .map_err(|e| format!("Failed to create HTTP client for chunk {}: {}", chunk_number, e))?;
    
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
        "max_tokens": 16384, // Increased from 8192
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
    
    println!("Making OpenAI API request for chunk {} (prompt length: {} chars)...", chunk_number, prompt.len());
    
    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", openai_token))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Failed to call OpenAI API for chunk {}: {}", chunk_number, e))?;
    
    println!("Received response for chunk {}", chunk_number);
    
    let status = response.status();
    println!("OpenAI API responded with status: {} for chunk {}", status, chunk_number);
    
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();
        println!("Error response body for chunk {}: {}", chunk_number, error_body);
        return Err(format!("OpenAI API error for chunk {}: {} - {}", chunk_number, status, error_body));
    }
    
    println!("Parsing JSON response for chunk {}...", chunk_number);
    let response_json: serde_json::Value = response.json().await
        .map_err(|e| format!("Failed to parse OpenAI response for chunk {}: {}", chunk_number, e))?;
    
    println!("Extracting content from response for chunk {}...", chunk_number);
    // Extract the content from the response
    let content = response_json
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .ok_or("Failed to extract content from OpenAI response")?;
    
    println!("Content extracted for chunk {} (length: {} chars)", chunk_number, content.len());
    
    // Parse the structured response
    println!("Parsing structured response for chunk {}...", chunk_number);
    let structured_response: StructuredTestResponse = serde_json::from_str(content)
        .map_err(|e| format!("Failed to parse chunk {} response as JSON: {}", chunk_number, e))?;
    
    println!("Successfully processed chunk {} with {} test results", chunk_number, structured_response.test_results.len());
    Ok(structured_response.test_results)
}

// Helper function to merge results from multiple chunks
fn merge_chunk_results(
    chunk_results: Vec<Vec<TestStatusWithoutType>>,
    all_tests: &[(&str, String)],
) -> Vec<TestStatus> {
    use std::collections::HashMap;
    
    // Create a map to store the final status for each test
    let mut test_status_map: HashMap<String, String> = HashMap::new();
    
    // Process each chunk's results
    for chunk_result in chunk_results {
        for test_status in chunk_result {
            let test_name = test_status.test_name;
            let status = test_status.status;
            
            // Apply conflict resolution rules
            match test_status_map.get(&test_name) {
                Some(existing_status) => {
                    // Conflict resolution:
                    // 1. If one is "failed" and other is "passed", choose "failed"
                    // 2. If one is "non_existing" and other has a value, choose the value
                    // 3. If both are same, keep it
                    let new_status = match (existing_status.as_str(), status.as_str()) {
                        ("failed", "passed") | ("passed", "failed") => "failed".to_string(),
                        ("non_existing", val) | (val, "non_existing") => val.to_string(),
                        (same, _) if same == status.as_str() => same.to_string(),
                        _ => status, // Default to new status for other cases
                    };
                    test_status_map.insert(test_name, new_status);
                }
                None => {
                    test_status_map.insert(test_name, status);
                }
            }
        }
    }
    
    // Create a lookup map for test types
    let mut test_type_map = HashMap::new();
    for (test_type, test_name) in all_tests {
        test_type_map.insert(test_name.clone(), test_type.to_string());
    }
    
    // Convert to final TestStatus objects
    let mut final_results: Vec<TestStatus> = test_status_map
        .into_iter()
        .map(|(test_name, status)| {
            let test_type = test_type_map.get(&test_name)
                .unwrap_or(&"unknown".to_string())
                .clone();
            TestStatus {
                test_name,
                status,
                r#type: test_type,
            }
        })
        .collect();
    
    // Sort by test name for consistent output
    final_results.sort_by(|a, b| a.test_name.cmp(&b.test_name));
    
    println!("Merged results contain {} unique tests", final_results.len());
    final_results
}

async fn analyze_log_with_openai(
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
    
    // Chunk the log content for processing
    let chunk_size = 50000; // 50KB chunks for more reliable processing
    let chunks = chunk_log_content(&log_content, chunk_size);
    println!("Split log into {} chunks for processing", chunks.len());
    
    // Process each chunk and collect results
    let mut all_chunk_results = Vec::new();
    for (i, chunk) in chunks.iter().enumerate() {
        println!("Processing chunk {}/{} (size: {} bytes)", i + 1, chunks.len(), chunk.len());
        
        // Retry mechanism for chunk processing
        let mut retry_count = 0;
        let max_retries = 3;
        let chunk_result = loop {
            match process_log_chunk(chunk, openai_token, all_tests, i + 1).await {
                Ok(result) => break result,
                Err(e) => {
                    retry_count += 1;
                    if retry_count > max_retries {
                        return Err(format!("Failed to process chunk {} after {} retries: {}", i + 1, max_retries, e));
                    }
                    println!("Error: {}", e);
                    println!("Chunk {} failed (attempt {}), retrying in 2 seconds...", i + 1, retry_count);
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                }
            }
        };
        
        all_chunk_results.push(chunk_result);
    }
    
    // Merge results from all chunks
    let merged_results = merge_chunk_results(all_chunk_results, all_tests);
    println!("Merged results from {} chunks", chunks.len());
    
    // Convert merged results to JSON
    let final_content = serde_json::to_string_pretty(&merged_results)
        .map_err(|e| format!("Failed to serialize merged results: {}", e))?;
    
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

pub fn read_analysis_file(file_path: String) -> Result<String, String> {
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

#[derive(Serialize, Deserialize)]
pub struct TestLists {
    pub fail_to_pass: Vec<String>,
    pub pass_to_pass: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct SearchResult {
    pub line_number: usize,
    pub line_content: String,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct LogSearchResults {
    pub base_results: Vec<SearchResult>,
    pub before_results: Vec<SearchResult>,
    pub after_results: Vec<SearchResult>,
}

pub fn get_test_lists(file_paths: Vec<String>) -> Result<TestLists, String> {
    println!("Getting test lists from file paths: {:?}", file_paths);
    
    // Find main.json file
    let main_json_path = file_paths.iter()
        .find(|path| path.to_lowercase().contains("main.json") || path.to_lowercase().contains("main/"))
        .ok_or("main.json file not found in provided paths".to_string())?;
    
    println!("Found main.json at: {}", main_json_path);
    
    let main_json_content = fs::read_to_string(main_json_path)
        .map_err(|e| format!("Failed to read main.json: {}", e))?;
    
    let main_json: serde_json::Value = serde_json::from_str(&main_json_content)
        .map_err(|e| format!("Failed to parse main.json: {}", e))?;
    
    // Extract fail_to_pass and pass_to_pass arrays
    let empty_vec: Vec<serde_json::Value> = vec![];
    let fail_to_pass: Vec<String> = main_json.get("fail_to_pass")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty_vec)
        .iter()
        .filter_map(|v| v.as_str())
        .map(|s| s.to_string())
        .collect();
    
    let pass_to_pass: Vec<String> = main_json.get("pass_to_pass")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty_vec)
        .iter()
        .filter_map(|v| v.as_str())
        .map(|s| s.to_string())
        .collect();
    
    println!("Found {} fail_to_pass tests and {} pass_to_pass tests", 
             fail_to_pass.len(), pass_to_pass.len());
    
    Ok(TestLists {
        fail_to_pass,
        pass_to_pass,
    })
}

pub fn search_logs(file_paths: Vec<String>, test_name: String) -> Result<LogSearchResults, String> {
    println!("Searching logs for test: {}", test_name);
    
    // Find log files
    let base_log = file_paths.iter().find(|path| path.to_lowercase().contains("base.log"));
    let before_log = file_paths.iter().find(|path| path.to_lowercase().contains("before.log"));
    let after_log = file_paths.iter().find(|path| path.to_lowercase().contains("after.log"));
    
    let base_results = if let Some(path) = base_log {
        search_in_log_file(path, &test_name)?
    } else {
        Vec::new()
    };
    
    let before_results = if let Some(path) = before_log {
        search_in_log_file(path, &test_name)?
    } else {
        Vec::new()
    };
    
    let after_results = if let Some(path) = after_log {
        search_in_log_file(path, &test_name)?
    } else {
        Vec::new()
    };
    
    println!("Search results: base={}, before={}, after={}", 
             base_results.len(), before_results.len(), after_results.len());
    
    Ok(LogSearchResults {
        base_results,
        before_results,
        after_results,
    })
}

pub async fn analyze_logs(file_paths: Vec<String>) -> Result<serde_json::Value, String> {
    println!("Starting log analysis with file paths: {:?}", file_paths);
    
    // Find and parse main.json
    let main_json_path = file_paths.iter()
        .find(|path| path.to_lowercase().contains("main.json") || path.to_lowercase().contains("main/"))
        .ok_or("main.json file not found in provided paths".to_string())?;
    
    println!("Found main.json at: {}", main_json_path);
    
    let main_json_content = fs::read_to_string(main_json_path)
        .map_err(|e| format!("Failed to read main.json: {}", e))?;
    
    let main_json: serde_json::Value = serde_json::from_str(&main_json_content)
        .map_err(|e| format!("Failed to parse main.json: {}", e))?;
    
    // Extract fail_to_pass and pass_to_pass arrays
    let empty_vec: Vec<serde_json::Value> = vec![];
    let fail_to_pass: Vec<String> = main_json.get("fail_to_pass")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty_vec)
        .iter()
        .filter_map(|v| v.as_str())
        .map(|s| s.to_string())
        .collect();
    
    let pass_to_pass: Vec<String> = main_json.get("pass_to_pass")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty_vec)
        .iter()
        .filter_map(|v| v.as_str())
        .map(|s| s.to_string())
        .collect();
    
    println!("Found {} fail_to_pass tests and {} pass_to_pass tests", 
             fail_to_pass.len(), pass_to_pass.len());
    
    // Find log files
    let base_log = file_paths.iter().find(|path| path.to_lowercase().contains("base.log"));
    let before_log = file_paths.iter().find(|path| path.to_lowercase().contains("before.log"));
    let after_log = file_paths.iter().find(|path| path.to_lowercase().contains("after.log"));
    
    if base_log.is_none() || before_log.is_none() || after_log.is_none() {
        return Err("Missing required log files (base.log, before.log, after.log)".to_string());
    }
    
    // Parse log files using the Rust test parser logic
    let base_parsed = parse_rust_log_file(base_log.unwrap())?;
    let before_parsed = parse_rust_log_file(before_log.unwrap())?;
    let after_parsed = parse_rust_log_file(after_log.unwrap())?;
    
    // Generate analysis result similar to swebench-log-analyzer-rust
    let analysis_result = generate_analysis_result(
        &base_parsed,
        &before_parsed, 
        &after_parsed,
        &pass_to_pass,
        &fail_to_pass,
        base_log.unwrap(),
        before_log.unwrap(),
        after_log.unwrap()
    );
    
    Ok(analysis_result)
}

fn search_in_log_file(file_path: &str, test_name: &str) -> Result<Vec<SearchResult>, String> {
    println!("Searching in log file: {} for test: {}", file_path, test_name);
    
    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read log file {}: {}", file_path, e))?;
    
    let lines: Vec<&str> = content.lines().collect();
    let mut results = Vec::new();
    
    // Prepare search terms
    let search_terms = get_search_terms(test_name);
    println!("Search terms for '{}': {:?}", test_name, search_terms);
    
    // Search for lines containing any of the search terms
    for (line_number, line) in lines.iter().enumerate() {
        let mut found_match = false;
        
        // Check if line contains any of our search terms
        for search_term in &search_terms {
            if line.contains(search_term) {
                found_match = true;
                break;
            }
        }
        
        if found_match {
            let context_before: Vec<String> = lines.iter()
                .skip(line_number.saturating_sub(5))
                .take(5.min(line_number))
                .map(|s| s.to_string())
                .collect();
            
            let context_after: Vec<String> = lines.iter()
                .skip(line_number + 1)
                .take(5)
                .map(|s| s.to_string())
                .collect();
            
            results.push(SearchResult {
                line_number: line_number + 1, // 1-based line numbers
                line_content: line.to_string(),
                context_before,
                context_after,
            });
        }
    }
    
    println!("Found {} matches in {}", results.len(), file_path);
    Ok(results)
}

fn get_search_terms(test_name: &str) -> Vec<String> {
    let mut search_terms = vec![test_name.to_string()];
    
    // Split on " - " and take the last part if it exists
    if let Some(last_part) = test_name.split(" - ").last() {
        if last_part != test_name {
            // Only add if it's different from the original test name
            search_terms.push(last_part.to_string());
        }
    }
    
    // Remove duplicates while preserving order
    search_terms.dedup();
    
    search_terms
}

#[derive(Debug)]
struct ParsedLog {
    passed: std::collections::HashSet<String>,
    failed: std::collections::HashSet<String>,
    ignored: std::collections::HashSet<String>,
    all: std::collections::HashSet<String>,
}

// ---------------- Single-line (ANSI) aware parsing ----------------
fn strip_ansi_color_codes(s: &str) -> String {
    ANSI_RE.replace_all(s, "").into_owned()
}

fn parse_rust_log_single_line(text: &str) -> ParsedLog {
    let mut passed = std::collections::HashSet::new();
    let mut failed = std::collections::HashSet::new();
    let mut ignored = std::collections::HashSet::new();

    let clean = strip_ansi_color_codes(text);

    // fast path: straightforward "test name ... STATUS"
    for cap in ENH_TEST_RE_1.captures_iter(&clean) {
        let name = cap.get(1).unwrap().as_str().to_string();
        let mut status = cap.get(2).unwrap().as_str().to_lowercase();
        if status == "failed" || status == "error" {
            status = "failed".to_string();
        }
        match status.as_str() {
            "ok" => { passed.insert(name); }
            "failed" => { failed.insert(name); }
            "ignored" => { ignored.insert(name); }
            _ => {}
        }
    }

    // harder cases: "test name ... <debug> STATUS" before next test
    let start_re = Regex::new(r"(?i)test\s+([^\s.]+(?:::[^\s.]+)*)\s*\.{2,}").unwrap();
    let next_test_re = Regex::new(r"(?i)test\s+[^\s.]+(?:::[^\s.]+)*\s*\.{2,}").unwrap();
    for cap in start_re.captures_iter(&clean) {
        let name = cap.get(1).unwrap().as_str().to_string();
        if passed.contains(&name) || failed.contains(&name) || ignored.contains(&name) {
            continue;
        }
        let search_pos = cap.get(0).unwrap().end();
        let end_pos = if let Some(ncap) = next_test_re.find_at(&clean, search_pos) {
            ncap.start()
        } else {
            std::cmp::min(search_pos + 1000, clean.len())
        };
        let window = &clean[search_pos..end_pos];

        // prefer a status near the end of window and not obviously part of diagnostics
        // Find all status matches and pick the most appropriate one
        let mut status_matches = Vec::new();
        for m in STATUS_IN_TEXT_RE.find_iter(window) {
            let status = m.as_str().to_lowercase();
            let match_start = m.start();
            
            // Get context around the match
            let context_start = match_start.saturating_sub(50);
            let context_end = std::cmp::min(match_start + 50, window.len());
            let context = &window[context_start..context_end].to_lowercase();
            
            // Enhanced filtering to avoid false positives
            if status == "error" && (
                context.contains("error:") || 
                context.contains("panic") ||
                context.contains("custom") ||
                context.contains("called `result::unwrap()") ||
                context.contains("thread") ||
                context.contains("kind:")
            ) {
                continue;
            }
            
            status_matches.push((status, match_start));
        }
        
        // Use the last (most recent) valid status match
        if let Some((status, _)) = status_matches.last() {
            match status.as_str() {
                "ok" => { passed.insert(name); }
                "failed" | "error" => { failed.insert(name); }
                "ignored" => { ignored.insert(name); }
                _ => {}
            }
        }
    }

    let mut all = std::collections::HashSet::new();
    all.extend(passed.iter().cloned());
    all.extend(failed.iter().cloned());
    all.extend(ignored.iter().cloned());

    ParsedLog { passed, failed, ignored, all }
}

fn looks_single_line_like(text: &str) -> bool {
    let line_count = text.lines().count();
    let has_ansi = ANSI_RE.is_match(text);
    let simple_pat = Regex::new(r"(?i)test\s+[^\s.]+(?:::[^\s.]+)*\s*\.{2,}\s*(ok|FAILED|ignored|error)").unwrap();
    let test_count = simple_pat.find_iter(text).count();
    (line_count <= 3 && test_count > 5) || has_ansi
}

fn parse_rust_log_file(file_path: &str) -> Result<ParsedLog, String> {
    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read log file {}: {}", file_path, e))?;

    // Switch to ANSI/single-line parser when appropriate
    if looks_single_line_like(&content) {
        return Ok(parse_rust_log_single_line(&content));
    }

    let mut passed = std::collections::HashSet::new();
    let mut failed = std::collections::HashSet::new();
    let mut ignored = std::collections::HashSet::new();
    let mut freq = std::collections::HashMap::new();
    
    let lines: Vec<&str> = content.lines().collect();
    
    // First pass: handle normal test lines with immediate results
    for line in &lines {
        if let Some(captures) = TEST_LINE_RE.captures(line) {
            let test_name = captures.get(1).unwrap().as_str().to_string();
            let status = captures.get(2).unwrap().as_str().to_lowercase();
            
            *freq.entry(test_name.clone()).or_insert(0) += 1;
            
            match status.as_str() {
                "ok" => { passed.insert(test_name); }
                "failed" | "error" => { failed.insert(test_name); }
                "ignored" => { ignored.insert(test_name); }
                _ => {}
            }
        }
    }
    
    // Second pass: handle cases where test result is on a separate line
    let mut pending_tests = std::collections::HashMap::new();
    
    for (i, line) in lines.iter().enumerate() {
        if let Some(captures) = TEST_START_RE.captures(line) {
            let test_name = captures.get(1).unwrap().as_str().to_string();
            let remainder = captures.get(2).unwrap().as_str();
            
            // Skip if we already found this test with a clear status
            if passed.contains(&test_name) || failed.contains(&test_name) || ignored.contains(&test_name) {
                continue;
            }
            
            // If remainder doesn't contain a clear status, this test might have result later
            if !STATUS_RE.is_match(remainder) {
                pending_tests.insert(test_name, i);
            }
        }

        // Also consider corrupted test lines mixed with debug output
        if let Some(cap) = CORRUPTED_TEST_LINE_RE.captures(line) {
            let tn = cap.get(1).unwrap().as_str().to_string();
            if !passed.contains(&tn) && !failed.contains(&tn) && !ignored.contains(&tn) {
                pending_tests.insert(tn, i);
            }
        }
    }
    
    // For pending tests, search more aggressively for their results
    for (test_name, start_line) in pending_tests {
        // Look in subsequent lines for the result, potentially many lines later
        let initial_limit = 200usize;
        let extended_limit = 10_000usize; // for verbose logs
        let mut found = false;

        // heuristic: try normal window first
        for j in start_line + 1..min(start_line + initial_limit, lines.len()) {
            let line = lines[j];

            // Check for standalone status words
            let stripped = line.trim();
            if stripped.eq_ignore_ascii_case("ok")
                || stripped.eq_ignore_ascii_case("FAILED")
                || stripped.eq_ignore_ascii_case("ignored")
                || stripped.eq_ignore_ascii_case("error")
            {
                let status = stripped.to_lowercase();
                *freq.entry(test_name.clone()).or_insert(0) += 1;

                match status.as_str() {
                    "ok" => { passed.insert(test_name.clone()); }
                    "failed" | "error" => { failed.insert(test_name.clone()); }
                    "ignored" => { ignored.insert(test_name.clone()); }
                    _ => {}
                }
                found = true;
                break;
            }

            // Check for status words at the end of lines (after debug output)
            if let Some(captures) = STATUS_AT_END_RE.captures(line) {
                let status = captures.get(1).unwrap().as_str().to_lowercase();
                
                // Enhanced filtering to avoid false positives from diagnostic messages
                let line_lower = line.to_lowercase();
                if status == "error" && (
                    line_lower.contains("error:") || 
                    line_lower.contains("panic") ||
                    line_lower.contains("custom") ||
                    line_lower.contains("called `result::unwrap()") ||
                    line_lower.contains("thread") ||
                    line_lower.contains("kind:")
                ) {
                    continue;
                }
                
                // Also skip if the status word appears in the middle of a diagnostic message
                if let Some(pos) = line_lower.find(&status) {
                    let before_status = &line_lower[..pos];
                    let after_status = &line_lower[pos + status.len()..];
                    
                    // Skip if it's clearly part of a diagnostic message
                    if before_status.contains("error:") || 
                       before_status.contains("panic") ||
                       after_status.contains("value:") ||
                       after_status.contains("kind:") {
                        continue;
                    }
                }
                
                *freq.entry(test_name.clone()).or_insert(0) += 1;

                match status.as_str() {
                    "ok" => { passed.insert(test_name.clone()); }
                    "failed" | "error" => { failed.insert(test_name.clone()); }
                    "ignored" => { ignored.insert(test_name.clone()); }
                    _ => {}
                }
                found = true;
                break;
            }

            // Stop looking if we hit another test line (but allow some leeway)
            if ANOTHER_TEST_RE.is_match(line) && j > start_line + 5 {
                break;
            }
        }

        // Extended scan window for extremely verbose logs
        if !found {
            for j in min(start_line + initial_limit, lines.len())..min(start_line + extended_limit, lines.len()) {
                let line = lines[j];
                let stripped = line.trim();
                if stripped.eq_ignore_ascii_case("ok")
                    || stripped.eq_ignore_ascii_case("FAILED")
                    || stripped.eq_ignore_ascii_case("ignored")
                    || stripped.eq_ignore_ascii_case("error")
                {
                    let status = stripped.to_lowercase();
                    *freq.entry(test_name.clone()).or_insert(0) += 1;
                    match status.as_str() {
                        "ok" => { passed.insert(test_name.clone()); }
                        "failed" | "error" => { failed.insert(test_name.clone()); }
                        "ignored" => { ignored.insert(test_name.clone()); }
                        _ => {}
                    }
                    break;
                }

                if let Some(captures) = STATUS_AT_END_RE.captures(line) {
                    let status = captures.get(1).unwrap().as_str().to_lowercase();
                    
                    // Enhanced filtering to avoid false positives from diagnostic messages
                    let line_lower = line.to_lowercase();
                    if status == "error" && (
                        line_lower.contains("error:") || 
                        line_lower.contains("panic") ||
                        line_lower.contains("custom") ||
                        line_lower.contains("called `result::unwrap()") ||
                        line_lower.contains("thread") ||
                        line_lower.contains("kind:")
                    ) {
                        continue;
                    }
                    
                    // Also skip if the status word appears in the middle of a diagnostic message
                    if let Some(pos) = line_lower.find(&status) {
                        let before_status = &line_lower[..pos];
                        let after_status = &line_lower[pos + status.len()..];
                        
                        // Skip if it's clearly part of a diagnostic message
                        if before_status.contains("error:") || 
                           before_status.contains("panic") ||
                           after_status.contains("value:") ||
                           after_status.contains("kind:") {
                            continue;
                        }
                    }
                    
                    *freq.entry(test_name.clone()).or_insert(0) += 1;
                    match status.as_str() {
                        "ok" => { passed.insert(test_name.clone()); }
                        "failed" | "error" => { failed.insert(test_name.clone()); }
                        "ignored" => { ignored.insert(test_name.clone()); }
                        _ => {}
                    }
                    break;
                }

                if ANOTHER_TEST_RE.is_match(line) && j > start_line + 50 { break; }
            }
        }
    }
    
    // Third pass: handle split status words like "o\nk"
    for (i, line) in lines.iter().enumerate() {
        // Look for lines that end with just "o" and check if next line starts with "k"
        if line.trim() == "o" && i + 1 < lines.len() && lines[i + 1].trim() == "k" {
            // Look backwards to find the corresponding test
            for j in (0..i).rev().take(10) {
                if let Some(captures) = TEST_WITH_O_RE.captures(lines[j]) {
                    let test_name = captures.get(1).unwrap().as_str().to_string();
                    if !passed.contains(&test_name) && !failed.contains(&test_name) && !ignored.contains(&test_name) {
                        *freq.entry(test_name.clone()).or_insert(0) += 1;
                        passed.insert(test_name);
                    }
                    break;
                }
            }
        }
        
        // Also handle the case where test line itself ends with "... o" (split across lines)
        if let Some(captures) = TEST_WITH_O_RE.captures(line) {
            let test_name = captures.get(1).unwrap().as_str().to_string();
            if i + 1 < lines.len() && lines[i + 1].trim() == "k" {
                if !passed.contains(&test_name) && !failed.contains(&test_name) && !ignored.contains(&test_name) {
                    *freq.entry(test_name.clone()).or_insert(0) += 1;
                    passed.insert(test_name);
                }
            }
        }
    }
    
    // Fourth pass: scan for any missed test patterns with complex formatting
    let mut test_starts = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if let Some(captures) = TEST_STARTS_RE.captures(line) {
            let test_name = captures.get(1).unwrap().as_str().to_string();
            test_starts.push((i, test_name));
        }
    }
    
    // For each test start, look for the corresponding result within a reasonable range
    for (line_idx, test_name) in test_starts {
        if passed.contains(&test_name) || failed.contains(&test_name) || ignored.contains(&test_name) {
            continue;
        }
        
        // Search forward through lines for the result
        let mut search_text = String::new();
        for j in line_idx..std::cmp::min(line_idx + 100, lines.len()) {
            search_text.push_str(lines[j]);
            search_text.push('\n');
            
            // Stop if we hit another test (but give some leeway for interleaved output)
            if j > line_idx + 5 && TEST_STARTS_RE.is_match(lines[j]) {
                break;
            }
        }
        
        // Look for status in this accumulated text, but be more selective
        // Find all status matches and pick the most likely one
        let mut status_matches = Vec::new();
        for cap in STATUS_IN_TEXT_RE.captures_iter(&search_text) {
            let status = cap.get(1).unwrap().as_str().to_lowercase();
            let match_start = cap.get(0).unwrap().start();
            
            // Get some context around the match
            let context_start = match_start.saturating_sub(50);
            let context_end = std::cmp::min(match_start + 50, search_text.len());
            let context = &search_text[context_start..context_end].to_lowercase();
            
            // Enhanced filtering to avoid false positives
            if status == "error" && (
                context.contains("error:") || 
                context.contains("panic") ||
                context.contains("custom") ||
                context.contains("called `result::unwrap()") ||
                context.contains("thread") ||
                context.contains("kind:")
            ) {
                continue;
            }
            
            status_matches.push((status, match_start));
        }
        
        // Use the last (most recent) valid status match
        if let Some((status, _)) = status_matches.last() {
            *freq.entry(test_name.clone()).or_insert(0) += 1;
            
            match status.as_str() {
                "ok" => { passed.insert(test_name); }
                "failed" | "error" => { failed.insert(test_name); }
                "ignored" => { ignored.insert(test_name); }
                _ => {}
            }
        }
    }
    
    // Also read the "failures:" block to catch names not emitted on one-line form
    let mut collecting = false;
    for line in &lines {
        let trimmed = line.trim();
        if trimmed == "failures:" {
            collecting = true;
            continue;
        }
        if collecting {
            if trimmed.starts_with("error:") || trimmed.starts_with("test result:") {
                collecting = false;
                continue;
            }
            if let Some(captures) = FAILURES_BLOCK_RE.captures(line) {
                let test_name = captures.get(1).unwrap().as_str().to_string();
                if !test_name.starts_with("----") {
                    failed.insert(test_name);
                }
                continue;
            }
            if trimmed.is_empty() || trimmed.starts_with("----") {
                continue;
            }
            collecting = false;
        }
    }
    
    let mut all = std::collections::HashSet::new();
    all.extend(passed.iter().cloned());
    all.extend(failed.iter().cloned());
    all.extend(ignored.iter().cloned());
    
    Ok(ParsedLog {
        passed,
        failed,
        ignored,
        all,
    })
}

// ---------------- Duplicate detection (C5) parity----------------
fn detect_file_boundary(line: &str) -> Option<String> {
    if let Some(c) = FILE_BOUNDARY_RE_1.captures(line) {
        return Some(c.get(1).unwrap().as_str().to_string());
    }
    if let Some(c) = FILE_BOUNDARY_RE_2.captures(line) {
        return Some(c.get(1).unwrap().as_str().to_string());
    }
    if let Some(c) = FILE_BOUNDARY_RE_3.captures(line) {
        return Some(c.get(1).unwrap().as_str().to_string());
    }
    None
}

fn extract_test_info_enhanced(line: &str) -> Option<(String, String)> {
    if let Some(c) = ENH_TEST_RE_1.captures(line) {
        return Some((
            c.get(1).unwrap().as_str().trim().to_string(),
            c.get(2).unwrap().as_str().trim().to_string(),
        ));
    }
    if let Some(c) = ENH_TEST_RE_2.captures(line) {
        return Some((
            c.get(1).unwrap().as_str().trim().to_string(),
            c.get(2).unwrap().as_str().trim().to_string(),
        ));
    }
    None
}

#[derive(Clone)]
struct Occur {
    test_name: String,
    status: String,
    line_no: usize,
    context_before: Vec<String>,
    context_after: Vec<String>,
}

fn is_true_duplicate(occ: &[Occur]) -> bool {
    if occ.len() <= 1 { return false; }
    let mut lines: Vec<usize> = occ.iter().map(|o| o.line_no).collect();
    lines.sort_unstable();
    let mut min_dist = usize::MAX;
    for i in 1..lines.len() {
        min_dist = min(min_dist, lines[i] - lines[i-1]);
    }
    if min_dist < 10 { return true; }
    let mut has_fail = false;
    let mut has_ok = false;
    for o in occ {
        let s = o.status.to_lowercase();
        if s == "failed" || s == "error" { has_fail = true; }
        if s == "ok" { has_ok = true; }
    }
    if has_fail && has_ok { return true; }
    let contexts: Vec<String> = occ.iter().map(|o| {
        let mut c = String::new();
        c.push_str(&o.context_before.join(" "));
        c.push_str(&o.context_after.join(" "));
        c.trim().to_string()
    }).collect();
    if !contexts.is_empty() && contexts.iter().all(|c| !c.is_empty() && *c == contexts[0]) {
        return true;
    }
    false
}

fn detect_same_file_duplicates(raw_content: &str) -> Vec<String> {
    if raw_content.is_empty() { return vec![]; }
    let lines: Vec<&str> = raw_content.split('\n').collect();
    let mut current_file = "unknown".to_string();
    use std::collections::HashMap;
    let mut per_file: HashMap<String, Vec<Occur>> = HashMap::new();

    for (i, line) in lines.iter().enumerate() {
        if let Some(f) = detect_file_boundary(line) {
            current_file = f;
            continue;
        }
        if let Some((name, status)) = extract_test_info_enhanced(line) {
            let before = if i >= 2 { lines[i-2..i].iter().map(|s| s.to_string()).collect() } else { vec![] };
            let after = if i+1 < lines.len() { lines[i+1..min(lines.len(), i+3)].iter().map(|s| s.to_string()).collect() } else { vec![] };
            per_file.entry(current_file.clone()).or_default().push(Occur{ test_name: name, status, line_no: i, context_before: before, context_after: after });
        }
    }

    let mut out = vec![];
    for (file, occs) in per_file {
        use std::collections::HashMap;
        let mut by_name: HashMap<String, Vec<Occur>> = HashMap::new();
        for o in occs { by_name.entry(o.test_name.clone()).or_default().push(o); }
        for (name, list) in by_name {
            if list.len() > 1 && is_true_duplicate(&list) {
                let places: Vec<String> = list.iter().map(|o| format!("line {}", o.line_no)).collect();
                out.push(format!("{} (appears {} times in {}: {})", name, places.len(), file, places.join(", ")));
            }
        }
    }
    out
}

fn status_lookup(names: &[String], parsed: &ParsedLog) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    for name in names {
        if parsed.failed.contains(name) {
            out.insert(name.clone(), "failed".to_string());
        } else if parsed.passed.contains(name) {
            out.insert(name.clone(), "passed".to_string());
        } else if parsed.ignored.contains(name) {
            out.insert(name.clone(), "ignored".to_string());
        } else {
            out.insert(name.clone(), "missing".to_string());
        }
    }
    out
}

fn generate_analysis_result(
    base_parsed: &ParsedLog,
    before_parsed: &ParsedLog,
    after_parsed: &ParsedLog,
    pass_to_pass: &[String],
    fail_to_pass: &[String],
    base_path: &str,
    before_path: &str,
    after_path: &str,
) -> serde_json::Value {
    let universe: Vec<String> = pass_to_pass.iter().chain(fail_to_pass.iter()).cloned().collect();
    
    let base_s = status_lookup(&universe, base_parsed);
    let before_s = status_lookup(&universe, before_parsed);
    let after_s = status_lookup(&universe, after_parsed);
    
    // ---------------- Rule checks parity ----------------
    let c1_hits: Vec<String> = pass_to_pass.iter()
        .filter(|t| base_s.get(*t) == Some(&"failed".to_string()))
        .cloned()
        .collect();
    let c1 = !c1_hits.is_empty();
    
    // C2: failed in after (not: "not passed")
    let c2_hits: Vec<String> = universe.iter()
        .filter(|t| after_s.get(*t) == Some(&"failed".to_string()))
        .cloned()
        .collect();
    let c2 = !c2_hits.is_empty();
    
    let c3_hits: Vec<String> = fail_to_pass.iter()
        .filter(|t| before_s.get(*t) == Some(&"passed".to_string()))
        .cloned()
        .collect();
    let c3 = !c3_hits.is_empty();
    
    // C4: Report *violations* of the valid P2P pattern:
    //  base: missing AND (before: failed ORreqaetr missing) AND after: passed
    // We mark problem when test violates the above (meets first part(s) but fails after, or passes in before).
    let mut c4_hits: Vec<String> = vec![];
    for t in pass_to_pass {
        let b = base_s.get(t).map(String::as_str).unwrap_or("missing");
        let be = before_s.get(t).map(String::as_str).unwrap_or("missing");
        let a = after_s.get(t).map(String::as_str).unwrap_or("missing");
        if b == "missing" {
            let before_ok = be == "failed" || be == "missing";
            let after_ok = a == "passed";
            if before_ok && !after_ok {
                c4_hits.push(format!("{t} (missing in base, {be} in before, but {a} in after)"));
            } else if !before_ok {
                c4_hits.push(format!("{t} (missing in base but {be} in before - violates C4 pattern)"));
            }
        }
    }
    let c4 = !c4_hits.is_empty();
    
    // C5: true duplicates per log using enhanced detection
    let mut dup_map = serde_json::Map::new();
    let base_txt = fs::read_to_string(base_path).unwrap_or_default();
    let before_txt = fs::read_to_string(before_path).unwrap_or_default();
    let after_txt = fs::read_to_string(after_path).unwrap_or_default();
    let base_dups = detect_same_file_duplicates(&base_txt);
    let before_dups = detect_same_file_duplicates(&before_txt);
    let after_dups = detect_same_file_duplicates(&after_txt);
    if !base_dups.is_empty() {
        dup_map.insert("base".to_string(), serde_json::Value::Array(base_dups.into_iter().take(50).map(serde_json::Value::String).collect()));
    }
    if !before_dups.is_empty() {
        dup_map.insert("before".to_string(), serde_json::Value::Array(before_dups.into_iter().take(50).map(serde_json::Value::String).collect()));
    }
    if !after_dups.is_empty() {
        dup_map.insert("after".to_string(), serde_json::Value::Array(after_dups.into_iter().take(50).map(serde_json::Value::String).collect()));
    }
    let c5 = !dup_map.is_empty();
    
    // P2P rejection logic
    let p2p_ignored: Vec<String> = pass_to_pass.iter()
        .filter(|t| base_s.get(*t) == Some(&"passed".to_string()) && after_s.get(*t) == Some(&"passed".to_string()))
        .cloned()
        .collect();
    
    let p2p_considered: Vec<String> = pass_to_pass.iter()
        .filter(|t| !(base_s.get(*t) == Some(&"passed".to_string()) && after_s.get(*t) == Some(&"passed".to_string())))
        .cloned()
        .collect();
    
    let p2p_rejected: Vec<String> = p2p_considered.iter()
        .filter(|t| base_s.get(*t) == Some(&"missing".to_string()) && before_s.get(*t) != Some(&"passed".to_string()))
        .cloned()
        .collect();
    
    let p2p_ok: Vec<String> = p2p_considered.iter()
        .filter(|t| base_s.get(*t) == Some(&"missing".to_string()) && before_s.get(*t) == Some(&"passed".to_string()))
        .cloned()
        .collect();
    
    let f2p_ignored: Vec<String> = fail_to_pass.iter()
        .filter(|t| after_s.get(*t) == Some(&"passed".to_string()))
        .cloned()
        .collect();
    
    let f2p_considered: Vec<String> = fail_to_pass.iter()
        .filter(|t| after_s.get(*t) != Some(&"passed".to_string()))
        .cloned()
        .collect();
    
    let f2p_rejected: Vec<String> = fail_to_pass.iter()
        .filter(|t| after_s.get(*t) == Some(&"failed".to_string()))
        .cloned()
        .collect();
    
    let f2p_ok: Vec<String> = fail_to_pass.iter()
        .filter(|t| after_s.get(*t) == Some(&"missing".to_string()))
        .cloned()
        .collect();
    
    let rejection_satisfied = !p2p_rejected.is_empty();
    
    // Generate p2p_analysis
    let mut p2p_analysis = serde_json::Map::new();
    for test_name in pass_to_pass {
        let mut test_data = serde_json::Map::new();
        test_data.insert("base".to_string(), serde_json::Value::String(base_s.get(test_name).unwrap_or(&"missing".to_string()).clone()));
        test_data.insert("before".to_string(), serde_json::Value::String(before_s.get(test_name).unwrap_or(&"missing".to_string()).clone()));
        test_data.insert("after".to_string(), serde_json::Value::String(after_s.get(test_name).unwrap_or(&"missing".to_string()).clone()));
        p2p_analysis.insert(test_name.clone(), serde_json::Value::Object(test_data));
    }
    
    // Generate f2p_analysis
    let mut f2p_analysis = serde_json::Map::new();
    for test_name in fail_to_pass {
        let mut test_data = serde_json::Map::new();
        test_data.insert("base".to_string(), serde_json::Value::String(base_s.get(test_name).unwrap_or(&"missing".to_string()).clone()));
        test_data.insert("before".to_string(), serde_json::Value::String(before_s.get(test_name).unwrap_or(&"missing".to_string()).clone()));
        test_data.insert("after".to_string(), serde_json::Value::String(after_s.get(test_name).unwrap_or(&"missing".to_string()).clone()));
        f2p_analysis.insert(test_name.clone(), serde_json::Value::Object(test_data));
    }
    
    serde_json::json!({
        "inputs": {
            "base_log": base_path,
            "before_log": before_path,
            "after_log": after_path,
        },
        "counts": {
            "P2P": pass_to_pass.len(),
            "F2P": fail_to_pass.len()
        },
        "rule_checks": {
            "c1_failed_in_base_present_in_P2P": {
                "has_problem": c1,
                "examples": c1_hits
            },
            "c2_failed_in_after_present_in_F2P_or_P2P": {
                "has_problem": c2,
                "examples": c2_hits
            },
            "c3_F2P_success_in_before": {
                "has_problem": c3,
                "examples": c3_hits
            },
            "c4_P2P_missing_in_base_and_not_passing_in_before": {
                "has_problem": c4,
                "examples": c4_hits
            },
            "c5_duplicates_in_same_log_for_F2P_or_P2P": {
                "has_problem": c5,
                "duplicate_examples_per_log": serde_json::Value::Object(dup_map)
            },
        },
        "rejection_reason": {
            "satisfied": rejection_satisfied,
            "p2p_ignored_because_passed_in_base_and_after": p2p_ignored,
            "p2p_considered": p2p_considered,
            "p2p_rejected": p2p_rejected,
            "p2p_considered_but_ok": p2p_ok,
            "f2p_ignored_because_passed_in_after": f2p_ignored,
            "f2p_considered": f2p_considered,
            "f2p_rejected": f2p_rejected,
            "f2p_considered_but_ok": f2p_ok,
        },
        "p2p_analysis": p2p_analysis,
        "f2p_analysis": f2p_analysis,
        "debug_log_counts": [
            {
                "label": "base",
                "passed": base_parsed.passed.len(),
                "failed": base_parsed.failed.len(),
                "ignored": base_parsed.ignored.len(),
                "all": base_parsed.all.len(),
            },
            {
                "label": "before",
                "passed": before_parsed.passed.len(),
                "failed": before_parsed.failed.len(),
                "ignored": before_parsed.ignored.len(),
                "all": before_parsed.all.len(),
            },
            {
                "label": "after",
                "passed": after_parsed.passed.len(),
                "failed": after_parsed.failed.len(),
                "ignored": after_parsed.ignored.len(),
                "all": after_parsed.all.len(),
            },
        ],
    })
}
