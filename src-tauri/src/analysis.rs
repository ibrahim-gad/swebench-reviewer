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

    // Pattern for mixed format: "test name ... status additional_content"
    static ref TEST_MIXED_FORMAT_RE: Regex = Regex::new(r"(?i)\btest\s+(.+?)\s+\.\.\.\s+(ok|FAILED|ignored|error)\s+(.+)")
        .expect("Failed to compile TEST_MIXED_FORMAT_RE regex");

    static ref TEST_START_RE: Regex = Regex::new(r"(?i)\btest\s+(.+?)\s+\.\.\.\s*(.*?)$")
        .expect("Failed to compile TEST_START_RE regex");

    static ref STATUS_RE: Regex = Regex::new(r"(?i)\b(ok|failed|ignored|error)\b")
        .expect("Failed to compile STATUS_RE regex");

    static ref STATUS_AT_END_RE: Regex = Regex::new(r"(?i)\b(ok|failed|ignored|error)\s*$")
        .expect("Failed to compile STATUS_AT_END_RE regex");

    // New pattern to match status at the beginning of lines mixed with logging output
    static ref STATUS_AT_START_RE: Regex = Regex::new(r"(?i)^(ok|FAILED|ignored|error)")
        .expect("Failed to compile STATUS_AT_START_RE regex");

    static ref ANOTHER_TEST_RE: Regex = Regex::new(r"(?i)\btest\s+[^\s]+\s+\.\.\.\s*")
        .expect("Failed to compile ANOTHER_TEST_RE regex");

    static ref TEST_WITH_O_RE: Regex = Regex::new(r"(?i)\btest\s+([^\s]+(?:::\w+)*)\s+\.\.\.\s*o\s*$")
        .expect("Failed to compile TEST_WITH_O_RE regex");

    static ref TEST_STARTS_RE: Regex = Regex::new(r"(?i)\btest\s+([^\s]+(?:::\w+)*)\s+\.\.\.\s*")
        .expect("Failed to compile TEST_STARTS_RE regex");

    static ref STATUS_IN_TEXT_RE: Regex = Regex::new(r"(?i)\b(ok|failed|ignored|error)\b")
        .expect("Failed to compile STATUS_IN_TEXT_RE regex");

    // Additional patterns
    static ref CORRUPTED_TEST_LINE_RE: Regex = Regex::new(r"(?i)(?:line)?test\s+([^\s]+(?:::\w+)*)\s+\.\.\.\s*")
        .expect("Failed to compile CORRUPTED_TEST_LINE_RE regex");

    // File boundary hints
    static ref FILE_BOUNDARY_RE_1: Regex = Regex::new(r"(?i)Running\s+([^\s]+(?:/[^\s]+)*\.(?:rs|fixed))\s*\(").unwrap();
    static ref FILE_BOUNDARY_RE_2: Regex = Regex::new(r"(?i)===\s*Running\s+(.+\.(?:rs|fixed))").unwrap();
    static ref FILE_BOUNDARY_RE_3: Regex = Regex::new(r"(?i)test\s+result:\s+ok\.\s+\d+\s+passed.*for\s+(.+\.(?:rs|fixed))").unwrap();

    // Enhanced extraction patterns
    static ref ENH_TEST_RE_1: Regex = Regex::new(r"(?i)\btest\s+([^\s]+(?:::[^\s]+)*)\s*\.{2,}\s*(ok|FAILED|ignored|error)").unwrap();
    static ref ENH_TEST_RE_2: Regex = Regex::new(r"(?i)test\s+([^\s]+)\s+\.\.\.\s+(ok|FAILED|ignored|error)").unwrap();
    
    // UI test format patterns - handles paths as test names with direct status
    static ref UI_TEST_PATH_RE: Regex = Regex::new(r"(?i)^([^\s]+(?:/[^\s]+)*\.(?:rs|fixed|toml|txt|md)(?:\s+\(revision\s+[^)]+\))?)\s+\.\.\.\s+(ok|FAILED|ignored|error)\s*$").unwrap();
    static ref UI_TEST_PATH_SIMPLE_RE: Regex = Regex::new(r"(?i)^([^\s]+(?:/[^\s]+)*\.(?:rs|fixed|toml|txt|md)(?:\s+\(revision\s+[^)]+\))?)\s+\.\.\.\s+(ok|FAILED|ignored|error)\s*$").unwrap();
    
    // Nextest format patterns - handles "PASS [duration] test_name" and "FAIL [duration] test_name"
    static ref NEXTEST_PASS_RE: Regex = Regex::new(r"(?i)\s*PASS\s+\[[^\]]+\]\s+(.+?)\s*$").unwrap();
    static ref NEXTEST_FAIL_RE: Regex = Regex::new(r"(?i)\s*FAIL\s+\[[^\]]+\]\s+(.+?)\s*$").unwrap();
    static ref NEXTEST_SKIP_RE: Regex = Regex::new(r"(?i)\s*(SKIP|IGNORED)\s+\[[^\]]+\]\s+(.+?)\s*$").unwrap();
    
    // START pattern for nextest - captures test names from START lines
    static ref NEXTEST_START_RE: Regex = Regex::new(r"(?i)^\s*START\s+(.+)$").unwrap();

    // ANSI escape detection
    static ref ANSI_RE: Regex = Regex::new(r"\x1B(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~])").unwrap();

    static ref FAILURES_BLOCK_RE: Regex = Regex::new(r"^\s{4}(.+?)\s*$")
        .expect("Failed to compile FAILURES_BLOCK_RE regex");

    // Additional patterns for single-line parsing to avoid repeated compilation
    static ref SINGLE_LINE_START_RE: Regex = Regex::new(r"(?i)test\s+([^\s]+(?:::[^\s]+)*)\s*\.{2,}").unwrap();
    static ref SINGLE_LINE_NEXT_TEST_RE: Regex = Regex::new(r"(?i)test\s+[^\s]+(?:::[^\s]+)*\s*\.{2,}").unwrap();
    static ref SINGLE_LINE_STATUS_AT_START_RE: Regex = Regex::new(r"(?i)^(ok|FAILED|ignored|error)").unwrap();
    static ref SIMPLE_PATTERN_RE: Regex = Regex::new(r"(?i)test\s+[^\s]+(?:::[^\s]+)*\s*\.{2,}\s*(ok|FAILED|ignored|error)").unwrap();
    
    // Pattern for tests that have diagnostic info after the "..." but before status
    static ref TEST_WITH_DIAGNOSTICS_RE: Regex = Regex::new(r"(?i)\btest\s+(.+?)\s+\.\.\.\s*(?:error:|$)").unwrap();
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
    pub agent_results: Vec<SearchResult>,
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
    let agent_log = file_paths.iter().find(|path| path.to_lowercase().contains("post_agent_patch.log") || path.to_lowercase().contains("agent.log"));
    
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

    let agent_results = if let Some(path) = agent_log {
        search_in_log_file(path, &test_name)?
    } else {
        Vec::new()
    };
    
    println!("Search results: base={}, before={}, after={}, agent={}", 
             base_results.len(), before_results.len(), after_results.len(), agent_results.len());
    
    Ok(LogSearchResults {
        base_results,
        before_results,
        after_results,
        agent_results,
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
    let agent_log = file_paths.iter().find(|path| path.to_lowercase().contains("post_agent_patch.log") || path.to_lowercase().contains("agent.log"));
    
    if base_log.is_none() || before_log.is_none() || after_log.is_none() {
        return Err("Missing required log files (base.log, before.log, after.log)".to_string());
    }
    
    // Parse log files using the Rust test parser logic
    let base_parsed = parse_rust_log_file(base_log.unwrap())?;
    let before_parsed = parse_rust_log_file(before_log.unwrap())?;
    let after_parsed = parse_rust_log_file(after_log.unwrap())?;
    
    // Parse agent log if available
    let agent_parsed = if let Some(agent_path) = agent_log {
        Some(parse_rust_log_file(agent_path)?)
    } else {
        None
    };
    
    // Find and parse report.json if available
    let report_json_path = file_paths.iter().find(|path| path.to_lowercase().contains("results/report.json") || path.to_lowercase().ends_with("report.json"));
    let report_data = if let Some(report_path) = report_json_path {
        println!("Found report.json at: {}", report_path);
        match fs::read_to_string(report_path) {
            Ok(content) => {
                match serde_json::from_str::<serde_json::Value>(&content) {
                    Ok(json) => Some(json),
                    Err(e) => {
                        println!("Failed to parse report.json: {}", e);
                        None
                    }
                }
            },
            Err(e) => {
                println!("Failed to read report.json: {}", e);
                None
            }
        }
    } else {
        println!("No report.json found in file paths");
        None
    };
    
    // Generate analysis result similar to swebench-log-analyzer-rust
    let analysis_result = generate_analysis_result(
        &base_parsed,
        &before_parsed, 
        &after_parsed,
        agent_parsed.as_ref(),
        &pass_to_pass,
        &fail_to_pass,
        base_log.unwrap(),
        before_log.unwrap(),
        after_log.unwrap(),
        agent_log,
        report_data.as_ref(),
        &file_paths
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

    // Handle mixed format: "test name ... status additional_content"
    for cap in TEST_MIXED_FORMAT_RE.captures_iter(&clean) {
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

    // UI test format: "path/to/test.rs ... ok" (without "test" keyword)
    for line in clean.lines() {
        if let Some(cap) = UI_TEST_PATH_RE.captures(line) {
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
    }

    // UI test format: "path/to/test.toml ... ok" (including .toml files)
    for line in clean.lines() {
        if let Some(cap) = UI_TEST_PATH_SIMPLE_RE.captures(line) {
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
    }

    // harder cases: "test name ... <debug> STATUS" before next test
    for cap in SINGLE_LINE_START_RE.captures_iter(&clean) {
        let name = cap.get(1).unwrap().as_str().to_string();
        if passed.contains(&name) || failed.contains(&name) || ignored.contains(&name) {
            continue;
        }
        let search_pos = cap.get(0).unwrap().end();
        let end_pos = if let Some(ncap) = SINGLE_LINE_NEXT_TEST_RE.find_at(&clean, search_pos) {
            ncap.start()
        } else {
            std::cmp::min(search_pos + 1000, clean.len())
        };
        let window = &clean[search_pos..end_pos];

        // Find all status matches including beginning-of-line patterns and pick the most appropriate one
        let mut status_matches = Vec::new();
        
        // Look for status at end of lines within window
        for m in STATUS_IN_TEXT_RE.find_iter(window) {
            let status = m.as_str().to_lowercase();
            let match_start = m.start();
            
            // Get context around the match (safely handle UTF-8 boundaries)
            let context_start = match_start.saturating_sub(50);
            let context_end = std::cmp::min(match_start + 50, window.len());
            
            // Optimized single-pass character boundary detection
            let mut safe_start = None;
            let mut safe_end = None;
            for (i, _) in window.char_indices() {
                if safe_start.is_none() && i >= context_start {
                    safe_start = Some(i);
                }
                if safe_end.is_none() && i >= context_end {
                    safe_end = Some(i);
                }
                if safe_start.is_some() && safe_end.is_some() {
                    break;
                }
            }
            let safe_start = safe_start.unwrap_or(context_start);
            let safe_end = safe_end.unwrap_or(context_end);
            let context = &window[safe_start..safe_end].to_lowercase();
            
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
        
        // Also look for status at the beginning of lines mixed with logging
        for line in window.lines() {
            if let Some(cap) = SINGLE_LINE_STATUS_AT_START_RE.captures(line) {
                let status = cap.get(1).unwrap().as_str().to_lowercase();
                let line_lower = line.to_lowercase();
                
                // Special handling for status mixed with logging output
                if (status == "failed" || status == "error") && 
                   (line_lower.contains("logging at") || 
                    line_lower.contains("debug:") || 
                    line_lower.contains("trace:") || 
                    line_lower.contains("info:") || 
                    line_lower.contains("warn:")) {
                    
                    // Check for panic evidence for this test in the window
                    let panic_for_this_test = window.to_lowercase().contains(&format!("thread '{}'", name)) && 
                                            window.to_lowercase().contains("panicked at");
                    
                    if panic_for_this_test {
                        status_matches.push((status, 0)); // Use 0 as position indicator for start-of-line matches
                    }
                } else {
                    status_matches.push((status, 0));
                }
            }
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

// Helper function to check if an error status is part of diagnostic messages
fn is_diagnostic_error(status: &str, line: &str) -> bool {
    if status != "error" {
        return false;
    }
    
    let line_lower = line.to_lowercase();
    line_lower.contains("error:") || 
    line_lower.contains("panic") ||
    line_lower.contains("custom") ||
    line_lower.contains("called `result::unwrap()") ||
    line_lower.contains("thread") ||
    line_lower.contains("kind:")
}

// Helper function to check if status appears in the middle of diagnostic messages
fn is_status_in_diagnostic_context(status: &str, line: &str) -> bool {
    let line_lower = line.to_lowercase();
    if let Some(pos) = line_lower.find(status) {
        let before_status = &line_lower[..pos];
        let after_status = &line_lower[pos + status.len()..];
        
        before_status.contains("error:") || 
        before_status.contains("panic") ||
        after_status.contains("value:") ||
        after_status.contains("kind:")
    } else {
        false
    }
}

// Helper function to check for panic evidence for a specific test
fn has_panic_evidence(test_name: &str, lines: &[&str], search_start: usize, search_end: usize) -> bool {
    let search_range = &lines[search_start..search_end];
    search_range.iter().any(|search_line| {
        let search_lower = search_line.to_lowercase();
        search_lower.contains(&format!("thread '{}'", test_name)) && 
        search_lower.contains("panicked at")
    })
}

// Helper function to process status and update test collections
fn process_test_status(
    status: &str,
    test_name: &str,
    passed: &mut std::collections::HashSet<String>,
    failed: &mut std::collections::HashSet<String>,
    ignored: &mut std::collections::HashSet<String>,
    freq: &mut std::collections::HashMap<String, i32>
) {
    *freq.entry(test_name.to_string()).or_insert(0) += 1;
    
    match status {
        "ok" => { passed.insert(test_name.to_string()); }
        "failed" | "error" => { failed.insert(test_name.to_string()); }
        "ignored" => { ignored.insert(test_name.to_string()); }
        _ => {}
    }
}

fn looks_single_line_like(text: &str) -> bool {
    let line_count = text.lines().count();
    let has_ansi = ANSI_RE.is_match(text);
    let test_count = SIMPLE_PATTERN_RE.find_iter(text).count();
    
    // Count UI test patterns line-by-line since they use line anchors
    let mut ui_test_count = 0;
    for line in text.lines() {
        if UI_TEST_PATH_RE.is_match(line) || UI_TEST_PATH_SIMPLE_RE.is_match(line) {
            ui_test_count += 1;
        }
    }
    
    // Check if it looks like a UI test format (many path-based test results)
    let has_ui_tests = ui_test_count > 10;
    
    (line_count <= 3 && test_count > 5) || has_ansi || has_ui_tests
}

fn looks_nextest_format(text: &str) -> bool {
    // Check for nextest-specific patterns
    let nextest_indicators = [
        "Nextest run ID",
        "nextest run",
        "Starting tests across",
        "PASS [",
        "FAIL [",
        "START             ", // Added START pattern from your example
    ];
    
    let has_indicators = nextest_indicators.iter().any(|indicator| 
        text.to_lowercase().contains(&indicator.to_lowercase())
    );
    
    // Count nextest-style result lines
    let nextest_lines = NEXTEST_PASS_RE.find_iter(text).count() + 
                       NEXTEST_FAIL_RE.find_iter(text).count() + 
                       NEXTEST_SKIP_RE.find_iter(text).count();
    
    // Also check for the mixed format pattern with traditional + nextest
    let has_mixed_format = text.contains("PASS [") && text.contains("test ") && text.contains("... ok");
    
    // Check for cargo nextest run command line
    let has_nextest_command = text.contains("cargo nextest run");
    
    has_indicators || nextest_lines > 5 || has_mixed_format || has_nextest_command
}

fn parse_nextest_log(text: &str) -> ParsedLog {
    let mut passed = std::collections::HashSet::new();
    let mut failed = std::collections::HashSet::new();
    let mut ignored = std::collections::HashSet::new();

    let lines: Vec<&str> = text.lines().collect();

    // Parse nextest format using separate regex patterns for better accuracy
    for (i, line) in lines.iter().enumerate() {
        // Parse PASS lines
        if let Some(captures) = NEXTEST_PASS_RE.captures(line) {
            let full_match = captures.get(1).unwrap().as_str().trim();
            // Extract just the test name part (after the crate name)
            let test_name = extract_test_name_from_nextest_line(full_match);
            println!("NEXTEST PASS: '{}' -> '{}'", full_match, test_name);
            passed.insert(test_name);
            continue;
        }
        
        // Parse FAIL lines
        if let Some(captures) = NEXTEST_FAIL_RE.captures(line) {
            let full_match = captures.get(1).unwrap().as_str().trim();
            // Extract just the test name part (after the crate name)
            let test_name = extract_test_name_from_nextest_line(full_match);
            println!("NEXTEST FAIL: '{}' -> '{}'", full_match, test_name);
            failed.insert(test_name);
            continue;
        }
        
        // Parse SKIP/IGNORED lines - note: using capture group 2 for SKIP/IGNORED
        if let Some(captures) = NEXTEST_SKIP_RE.captures(line) {
            // For SKIP/IGNORED pattern, the test name is in group 2
            if let Some(test_name_match) = captures.get(2) {
                let full_match = test_name_match.as_str().trim();
                let test_name = extract_test_name_from_nextest_line(full_match);
                ignored.insert(test_name);
            }
            continue;
        }
        
        // Also handle traditional Rust test patterns for mixed format logs
        if let Some(captures) = TEST_LINE_RE.captures(line) {
            let test_name = captures.get(1).unwrap().as_str().to_string();
            let status = captures.get(2).unwrap().as_str().to_lowercase();
            
            match status.as_str() {
                "ok" => { passed.insert(test_name); }
                "failed" | "error" => { failed.insert(test_name); }
                "ignored" => { ignored.insert(test_name); }
                _ => {}
            }
            continue;
        }
        
        // Handle mixed format: "test name ... status additional_content"
        if let Some(captures) = TEST_MIXED_FORMAT_RE.captures(line) {
            let test_name = captures.get(1).unwrap().as_str().to_string();
            let status = captures.get(2).unwrap().as_str().to_lowercase();
            
            match status.as_str() {
                "ok" => { passed.insert(test_name); }
                "failed" | "error" => { failed.insert(test_name); }
                "ignored" => { ignored.insert(test_name); }
                _ => {}
            }
            continue;
        }
        
        // Handle enhanced test patterns as well
        if let Some(captures) = ENH_TEST_RE_1.captures(line) {
            let test_name = captures.get(1).unwrap().as_str().to_string();
            let mut status = captures.get(2).unwrap().as_str().to_lowercase();
            if status == "failed" || status == "error" {
                status = "failed".to_string();
            }
            match status.as_str() {
                "ok" => { passed.insert(test_name); }
                "failed" => { failed.insert(test_name); }
                "ignored" => { ignored.insert(test_name); }
                _ => {}
            }
            continue;
        }
        
        // Handle the diagnostic pattern: test starts with error/diagnostic but ends with status
        if let Some(captures) = TEST_START_RE.captures(line) {
            let test_name = captures.get(1).unwrap().as_str().to_string();
            let remainder = captures.get(2).unwrap().as_str().trim();
            
            // Skip if we already processed this test
            if passed.contains(&test_name) || failed.contains(&test_name) || ignored.contains(&test_name) {
                continue;
            }
            
            // Look for diagnostic pattern: test starts with diagnostic info or is empty after "..."
            if remainder.starts_with("error:") || remainder.is_empty() {
                // Search forward for the final status
                for j in (i + 1)..std::cmp::min(i + 50, lines.len()) {
                    let search_line = lines[j].trim();
                    
                    // Stop if we hit another test
                    if TEST_START_RE.is_match(lines[j]) {
                        break;
                    }
                    
                    // Look for standalone status words
                    if search_line.eq_ignore_ascii_case("ok") {
                        passed.insert(test_name.clone());
                        break;
                    } else if search_line.eq_ignore_ascii_case("failed") || 
                             search_line.eq_ignore_ascii_case("error") {
                        failed.insert(test_name.clone());
                        break;
                    } else if search_line.eq_ignore_ascii_case("ignored") {
                        ignored.insert(test_name.clone());
                        break;
                    }
                }
            }
        }
    }

    let mut all = std::collections::HashSet::new();
    all.extend(passed.iter().cloned());
    all.extend(failed.iter().cloned());
    all.extend(ignored.iter().cloned());

    ParsedLog { passed, failed, ignored, all }
}

fn parse_rust_log_file(file_path: &str) -> Result<ParsedLog, String> {
    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read log file {}: {}", file_path, e))?;

    // Check for nextest format first
    if looks_nextest_format(&content) {
        return Ok(parse_nextest_log(&content));
    }

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
        // Handle standard format: "test name ... status"
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
            continue;
        }
        
        // Handle mixed format: "test name ... status additional_content"
        if let Some(captures) = TEST_MIXED_FORMAT_RE.captures(line) {
            let test_name = captures.get(1).unwrap().as_str().to_string();
            let status = captures.get(2).unwrap().as_str().to_lowercase();
            
            *freq.entry(test_name.clone()).or_insert(0) += 1;
            
            match status.as_str() {
                "ok" => { passed.insert(test_name); }
                "failed" | "error" => { failed.insert(test_name); }
                "ignored" => { ignored.insert(test_name); }
                _ => {}
            }
            continue;
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

            // Check for status words at the end of lines (after debug output) OR at the beginning mixed with logging
            let mut status_match = None;
            if let Some(captures) = STATUS_AT_END_RE.captures(line) {
                status_match = Some(captures);
            } else if let Some(captures) = STATUS_AT_START_RE.captures(line) {
                status_match = Some(captures);
            }

            if let Some(captures) = status_match {
                let status = captures.get(1).unwrap().as_str().to_lowercase();
                
                // Enhanced filtering to avoid false positives from diagnostic messages
                if is_diagnostic_error(&status, line) {
                    continue;
                }
                
                // Also skip if the status word appears in the middle of a diagnostic message
                if is_status_in_diagnostic_context(&status, line) {
                    continue;
                }

                // Special handling for status mixed with logging output
                // Skip if the status appears mixed with logging output UNLESS there's evidence of a panic for this test
                let line_lower = line.to_lowercase();
                if (status == "failed" || status == "error") && 
                   (line_lower.contains("logging at") || 
                    line_lower.contains("debug:") || 
                    line_lower.contains("trace:") || 
                    line_lower.contains("info:") || 
                    line_lower.contains("warn:")) {
                    
                    // Check if there's a panic message for this specific test in a broader range
                    let search_start = start_line.saturating_sub(100);
                    let search_end = std::cmp::min(j + 1, lines.len());
                    
                    if !has_panic_evidence(&test_name, &lines, search_start, search_end) {
                        // This status is mixed with logging output and no panic evidence, skip it
                        continue;
                    }
                }
                
                process_test_status(&status, &test_name, &mut passed, &mut failed, &mut ignored, &mut freq);
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
                    process_test_status(&status, &test_name, &mut passed, &mut failed, &mut ignored, &mut freq);
                    break;
                }

                // Check for status words at the end of lines (after debug output) OR at the beginning mixed with logging
                let mut status_match = None;
                if let Some(captures) = STATUS_AT_END_RE.captures(line) {
                    status_match = Some(captures);
                } else if let Some(captures) = STATUS_AT_START_RE.captures(line) {
                    status_match = Some(captures);
                }

                if let Some(captures) = status_match {
                    let status = captures.get(1).unwrap().as_str().to_lowercase();
                    
                    // Enhanced filtering to avoid false positives from diagnostic messages
                    if is_diagnostic_error(&status, line) {
                        continue;
                    }
                    
                    // Also skip if the status word appears in the middle of a diagnostic message
                    if is_status_in_diagnostic_context(&status, line) {
                        continue;
                    }

                    // Special handling for status mixed with logging output
                    // Skip if the status appears mixed with logging output UNLESS there's evidence of a panic for this test
                    let line_lower = line.to_lowercase();
                    if (status == "failed" || status == "error") && 
                       (line_lower.contains("logging at") || 
                        line_lower.contains("debug:") || 
                        line_lower.contains("trace:") || 
                        line_lower.contains("info:") || 
                        line_lower.contains("warn:")) {
                        
                        // Check if there's a panic message for this specific test in a broader range
                        let search_start = start_line.saturating_sub(100);
                        let search_end = std::cmp::min(j + 1, lines.len());
                        
                        if !has_panic_evidence(&test_name, &lines, search_start, search_end) {
                            // This status is mixed with logging output and no panic evidence, skip it
                            continue;
                        }
                    }
                    
                    process_test_status(&status, &test_name, &mut passed, &mut failed, &mut ignored, &mut freq);
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
            
            // Get some context around the match (safely handle UTF-8 boundaries)
            let context_start = match_start.saturating_sub(50);
            let context_end = std::cmp::min(match_start + 50, search_text.len());
            
            // Optimized single-pass character boundary detection
            let mut safe_start = None;
            let mut safe_end = None;
            for (i, _) in search_text.char_indices() {
                if safe_start.is_none() && i >= context_start {
                    safe_start = Some(i);
                }
                if safe_end.is_none() && i >= context_end {
                    safe_end = Some(i);
                }
                if safe_start.is_some() && safe_end.is_some() {
                    break;
                }
            }
            let safe_start = safe_start.unwrap_or(context_start);
            let safe_end = safe_end.unwrap_or(context_end);
            let context = &search_text[safe_start..safe_end].to_lowercase();
            
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
            process_test_status(&status, &test_name, &mut passed, &mut failed, &mut ignored, &mut freq);
        }
    }
    
    // Fifth pass: handle tests with diagnostic output followed by status on separate line
    // This handles patterns like:
    // test name ... error: some diagnostic
    // more diagnostic lines
    // ok
    for (i, line) in lines.iter().enumerate() {
        if let Some(captures) = TEST_START_RE.captures(line) {
            let test_name = captures.get(1).unwrap().as_str().to_string();
            let remainder = captures.get(2).unwrap().as_str().trim();
            
            // Skip if we already processed this test
            if passed.contains(&test_name) || failed.contains(&test_name) || ignored.contains(&test_name) {
                continue;
            }
            
            // Look for diagnostic pattern: test starts with diagnostic info but no immediate status
            if remainder.starts_with("error:") || remainder.is_empty() {
                // Search forward for the final status (usually "ok", "failed", etc.)
                let mut found_status = false;
                for j in (i + 1)..std::cmp::min(i + 50, lines.len()) {
                    let search_line = lines[j].trim();
                    
                    // Stop if we hit another test
                    if TEST_START_RE.is_match(lines[j]) {
                        break;
                    }
                    
                    // Look for standalone status words
                    if search_line.eq_ignore_ascii_case("ok") {
                        passed.insert(test_name.clone());
                        found_status = true;
                        break;
                    } else if search_line.eq_ignore_ascii_case("failed") || 
                             search_line.eq_ignore_ascii_case("error") {
                        failed.insert(test_name.clone());
                        found_status = true;
                        break;
                    } else if search_line.eq_ignore_ascii_case("ignored") {
                        ignored.insert(test_name.clone());
                        found_status = true;
                        break;
                    }
                }
                
                if found_status {
                    *freq.entry(test_name).or_insert(0) += 1;
                }
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
    
    // Check for UI test format patterns
    if let Some(c) = UI_TEST_PATH_RE.captures(line) {
        return Some((
            c.get(1).unwrap().as_str().trim().to_string(),
            c.get(2).unwrap().as_str().trim().to_string(),
        ));
    }
    if let Some(c) = UI_TEST_PATH_SIMPLE_RE.captures(line) {
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
        min_dist = std::cmp::min(min_dist, lines[i] - lines[i-1]);
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
    
    println!("=== STATUS LOOKUP DEBUG ===");
    println!("Looking up status for {} test names", names.len());
    println!("Parsed log has {} passed, {} failed, {} ignored tests", 
             parsed.passed.len(), parsed.failed.len(), parsed.ignored.len());
    
    // Debug: show some examples of parsed test names
    if !parsed.passed.is_empty() {
        println!("Sample passed tests: {:?}", parsed.passed.iter().take(3).collect::<Vec<_>>());
    }
    if !parsed.failed.is_empty() {
        println!("Sample failed tests: {:?}", parsed.failed.iter().take(3).collect::<Vec<_>>());
    }
    
    for name in names {
        let status = if parsed.failed.contains(name) {
            "failed".to_string()
        } else if parsed.passed.contains(name) {
            "passed".to_string()
        } else if parsed.ignored.contains(name) {
            "ignored".to_string()
        } else {
            // Debug: Check for partial matches to understand the mismatch
            let partial_matches: Vec<&String> = parsed.passed.iter()
                .chain(parsed.failed.iter())
                .chain(parsed.ignored.iter())
                .filter(|test| test.contains(name) || name.contains(*test))
                .collect();
            
            if !partial_matches.is_empty() {
                println!("MISMATCH: '{}' not found exactly, but found partial matches: {:?}", name, partial_matches);
            } else {
                println!("MISSING: '{}' not found at all", name);
            }
            
            "missing".to_string()
        };
        
        out.insert(name.clone(), status);
    }
    
    println!("=== END STATUS LOOKUP ===");
    out
}

fn report_status_lookup(names: &[String], report_data: &serde_json::Value) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    let mut report_failed_tests = std::collections::HashSet::new();
    let mut report_passed_tests = std::collections::HashSet::new();
    
    // Parse report.json to extract test results using the same logic as C6 check
    // Try different possible structures for report.json
    if let Some(results_array) = report_data.get("results").and_then(|r| r.as_array()) {
        for result in results_array {
            if let (Some(test_name), Some(status)) = (result.get("test_name").and_then(|t| t.as_str()), result.get("status").and_then(|s| s.as_str())) {
                match status.to_lowercase().as_str() {
                    "failed" | "fail" => { report_failed_tests.insert(test_name.to_string()); }
                    "passed" | "pass" | "success" => { report_passed_tests.insert(test_name.to_string()); }
                    _ => {}
                }
            }
        }
    } else if let Some(test_results) = report_data.get("test_results").and_then(|r| r.as_array()) {
        for result in test_results {
            if let (Some(test_name), Some(status)) = (result.get("test_name").and_then(|t| t.as_str()), result.get("status").and_then(|s| s.as_str())) {
                match status.to_lowercase().as_str() {
                    "failed" | "fail" => { report_failed_tests.insert(test_name.to_string()); }
                    "passed" | "pass" | "success" => { report_passed_tests.insert(test_name.to_string()); }
                    _ => {}
                }
            }
        }
    } else if let Some(tests_obj) = report_data.get("tests").and_then(|t| t.as_object()) {
        // Format: {"tests": {"test_name": {"status": "failed"}}}
        for (test_name, test_data) in tests_obj {
            if let Some(status) = test_data.get("status").and_then(|s| s.as_str()) {
                match status.to_lowercase().as_str() {
                    "failed" | "fail" => { report_failed_tests.insert(test_name.clone()); }
                    "passed" | "pass" | "success" => { report_passed_tests.insert(test_name.clone()); }
                    _ => {}
                }
            }
        }
    } else if let Some(obj) = report_data.as_object() {
        // Check for SWE-bench format first
        let mut found_swe_format = false;
        for (_key, value) in obj {
            if let Some(tests_status) = value.get("tests_status").and_then(|t| t.as_object()) {
                found_swe_format = true;
                
                // Parse all test categories
                for (_category, category_data) in tests_status {
                    if let Some(category_obj) = category_data.as_object() {
                        // Extract failed tests from "failure" arrays
                        if let Some(failure_array) = category_obj.get("failure").and_then(|f| f.as_array()) {
                            for test_item in failure_array {
                                if let Some(test_name) = test_item.as_str() {
                                    report_failed_tests.insert(test_name.to_string());
                                }
                            }
                        }
                        // Extract passed tests from "success" arrays
                        if let Some(success_array) = category_obj.get("success").and_then(|f| f.as_array()) {
                            for test_item in success_array {
                                if let Some(test_name) = test_item.as_str() {
                                    report_passed_tests.insert(test_name.to_string());
                                }
                            }
                        }
                    }
                }
                break; // Found SWE-bench format, no need to check other keys
            }
        }
        
        // If not SWE-bench format, try direct mapping format: {"test_name": "status"}
        if !found_swe_format {
            for (test_name, status_val) in obj {
                if let Some(status) = status_val.as_str() {
                    match status.to_lowercase().as_str() {
                        "failed" | "fail" => { report_failed_tests.insert(test_name.clone()); }
                        "passed" | "pass" | "success" => { report_passed_tests.insert(test_name.clone()); }
                        _ => {}
                    }
                }
            }
        }
    }
    
    // Map test names to their status
    for name in names {
        if report_failed_tests.contains(name) {
            out.insert(name.clone(), "failed".to_string());
        } else if report_passed_tests.contains(name) {
            out.insert(name.clone(), "passed".to_string());
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
    agent_parsed: Option<&ParsedLog>,
    pass_to_pass: &[String],
    fail_to_pass: &[String],
    base_path: &str,
    before_path: &str,
    after_path: &str,
    agent_path: Option<&String>,
    report_data: Option<&serde_json::Value>,
    file_paths: &[String],
) -> serde_json::Value {
    let universe: Vec<String> = pass_to_pass.iter().chain(fail_to_pass.iter()).cloned().collect();
    
    let base_s = status_lookup(&universe, base_parsed);
    let before_s = status_lookup(&universe, before_parsed);
    let after_s = status_lookup(&universe, after_parsed);
    let agent_s = if let Some(agent_parsed) = agent_parsed {
        status_lookup(&universe, agent_parsed)
    } else {
        std::collections::HashMap::new()
    };
    
    // Parse report.json status if available
    let report_s = if let Some(report_data) = report_data {
        report_status_lookup(&universe, report_data)
    } else {
        std::collections::HashMap::new()
    };
    
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
    
    // C4: P2P tests that are missing in base and not passing in before
    // Logic:
    // - If P2P passed in base → Skip (don't check)
    // - If P2P is missing in base → Check before:
    //   - If passing in before → No violation
    //   - If missing or failed in before → Violation
    let mut c4_hits: Vec<String> = vec![];
    for t in pass_to_pass {
        let b = base_s.get(t).map(String::as_str).unwrap_or("missing");
        let be = before_s.get(t).map(String::as_str).unwrap_or("missing");
        
        // If P2P passed in base, skip this test (no need to check before)
        if b == "passed" {
            continue;
        }
        
        // If P2P is missing in base, check it in before
        if b == "missing" {
            // If P2P is NOT passing in before (missing or failed), it's a violation
            if be != "passed" {
                c4_hits.push(format!("{t} (missing in base, {be} in before)"));
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
    
    // C6: Test marked as failing in report.json but passing in post_agent_log
    // This checks for inconsistencies between report.json and agent log results
    let mut c6_hits: Vec<String> = vec![];
    // C7: F2P tests found in golden source diff files but not in test diff files
    let mut c7_hits: Vec<String> = vec![];
    let c7 = {
        println!("Performing C7 check: looking for F2P tests in golden source diff files (but not in test diffs)");
        
        // Find diff/patch files from patches folder
        let diff_files: Vec<&String> = file_paths.iter()
            .filter(|path| {
                let path_lower = path.to_lowercase();
                path_lower.contains("patches/") && (path_lower.ends_with(".diff") || path_lower.ends_with(".patch"))
            })
            .collect();
        
        println!("Found {} diff/patch files", diff_files.len());
        
        if !diff_files.is_empty() {
            // Separate golden source diffs from test diffs
            let (golden_source_diffs, test_diffs): (Vec<&String>, Vec<&String>) = diff_files.iter()
                .partition(|path| {
                    let filename = path.split('/').last().unwrap_or("").to_lowercase();
                    // Golden source diffs typically contain "gold", "golden", "src", "source"
                    // Test diffs typically contain "test"
                    (filename.contains("gold") || filename.contains("src") || filename.contains("source")) &&
                    !filename.contains("test")
                });
            
            println!("Found {} golden source diff files and {} test diff files", 
                     golden_source_diffs.len(), test_diffs.len());
            
            // Read all test diff contents to check if tests appear there
            let mut test_diff_contents = String::new();
            for test_diff in &test_diffs {
                if let Ok(content) = fs::read_to_string(test_diff) {
                    test_diff_contents.push_str(&content);
                    test_diff_contents.push('\n');
                    println!("Read test diff file: {}", test_diff);
                }
            }
            
            // Check golden source diffs for F2P tests
            for golden_diff in &golden_source_diffs {
                println!("Checking golden source diff file: {}", golden_diff);
                
                if let Ok(diff_content) = fs::read_to_string(golden_diff) {
                    println!("Read golden source diff successfully, {} bytes", diff_content.len());
                    
                    // Check if any F2P test names appear in this golden source diff
                    for f2p_test in fail_to_pass {
                        // Extract the actual test name from module path (e.g., "tests::test_example" -> "test_example")
                        let test_name_to_search = if f2p_test.contains("::") {
                            f2p_test.split("::").last().unwrap_or(f2p_test)
                        } else {
                            f2p_test
                        };
                        
                        if diff_content.contains(test_name_to_search) {
                            // Check if this test also appears in test diffs as an actual test function
                            let found_exact_test_in_test_diffs = if !test_diff_contents.is_empty() {
                                // Normalize line endings to handle CRLF, LF, etc.
                                let normalized_test_diff = test_diff_contents.replace("\r\n", "\n").replace("\r", "\n");
                                
                                // Look for exact test function patterns in test diffs
                                // Use regex-like matching to handle whitespace and line endings flexibly
                                let found_direct_fn = normalized_test_diff.contains(&format!("fn {}(", test_name_to_search)) ||
                                                     normalized_test_diff.contains(&format!("fn {} (", test_name_to_search));
                                
                                // Look for #[test] attribute followed by the function (with flexible whitespace/newlines)
                                let found_test_attribute = {
                                    let lines: Vec<&str> = normalized_test_diff.lines().collect();
                                    let mut found = false;
                                    for i in 0..lines.len().saturating_sub(1) {
                                        if lines[i].trim() == "#[test]" {
                                            // Check next few lines for the function
                                            for j in (i + 1)..std::cmp::min(i + 4, lines.len()) {
                                                let line = lines[j].trim();
                                                if line.starts_with(&format!("fn {}(", test_name_to_search)) ||
                                                   line.starts_with(&format!("fn {} (", test_name_to_search)) {
                                                    found = true;
                                                    break;
                                                }
                                            }
                                            if found { break; }
                                        }
                                    }
                                    found
                                };
                                
                                found_direct_fn || found_test_attribute
                            } else {
                                false
                            };
                            
                            if found_exact_test_in_test_diffs {
                                println!("F2P test '{}' found in both golden source and test diffs as actual test function - not a violation", f2p_test);
                            } else {
                                let violation = format!("{} (found as '{}' in {} but not as actual test function in test diffs)", 
                                                      f2p_test, test_name_to_search, 
                                                      golden_diff.split('/').last().unwrap_or(golden_diff));
                                c7_hits.push(violation);
                                println!("C7 violation: F2P test '{}' found as '{}' in golden source diff '{}' but not as actual test function in test diffs", 
                                         f2p_test, test_name_to_search, golden_diff);
                            }
                        }
                    }
                } else {
                    println!("Failed to read golden source diff file: {}", golden_diff);
                }
            }
        } else {
            println!("No diff/patch files found in patches folder");
        }
        
        let has_violations = !c7_hits.is_empty();
        println!("C7 check completed: {} violations found", c7_hits.len());
        has_violations
    };

    let c6 = if let (Some(_agent_parsed), Some(report_data)) = (agent_parsed, report_data) {
        println!("Performing C6 check: comparing report.json with agent log results");
        
        // Parse report.json to extract test results
        // Common formats: results array, test_results array, direct test mapping, or SWE-bench format
        let mut report_failed_tests = std::collections::HashSet::new();
        
        // Try different possible structures for report.json
        if let Some(results_array) = report_data.get("results").and_then(|r| r.as_array()) {
            for result in results_array {
                if let (Some(test_name), Some(status)) = (result.get("test_name").and_then(|t| t.as_str()), result.get("status").and_then(|s| s.as_str())) {
                    if status.to_lowercase() == "failed" || status.to_lowercase() == "fail" {
                        report_failed_tests.insert(test_name.to_string());
                    }
                }
            }
        } else if let Some(test_results) = report_data.get("test_results").and_then(|r| r.as_array()) {
            for result in test_results {
                if let (Some(test_name), Some(status)) = (result.get("test_name").and_then(|t| t.as_str()), result.get("status").and_then(|s| s.as_str())) {
                    if status.to_lowercase() == "failed" || status.to_lowercase() == "fail" {
                        report_failed_tests.insert(test_name.to_string());
                    }
                }
            }
        } else if let Some(tests_obj) = report_data.get("tests").and_then(|t| t.as_object()) {
            // Format: {"tests": {"test_name": {"status": "failed"}}}
            for (test_name, test_data) in tests_obj {
                if let Some(status) = test_data.get("status").and_then(|s| s.as_str()) {
                    if status.to_lowercase() == "failed" || status.to_lowercase() == "fail" {
                        report_failed_tests.insert(test_name.clone());
                    }
                }
            }
        } else if let Some(obj) = report_data.as_object() {
            // Check for SWE-bench format first
            let mut found_swe_format = false;
            for (key, value) in obj {
                if let Some(tests_status) = value.get("tests_status").and_then(|t| t.as_object()) {
                    println!("Found SWE-bench format report.json for key: {}", key);
                    found_swe_format = true;
                    
                    // Parse all test categories that indicate failure
                    for (category, category_data) in tests_status {
                        if let Some(category_obj) = category_data.as_object() {
                            // Extract failed tests from "failure" arrays in all categories
                            if let Some(failure_array) = category_obj.get("failure").and_then(|f| f.as_array()) {
                                for test_item in failure_array {
                                    if let Some(test_name) = test_item.as_str() {
                                        report_failed_tests.insert(test_name.to_string());
                                        println!("Found failed test in category {}: {}", category, test_name);
                                    }
                                }
                            }
                        }
                    }
                    break; // Found SWE-bench format, no need to check other keys
                }
            }
            
            // If not SWE-bench format, try direct mapping format: {"test_name": "status"}
            if !found_swe_format {
                for (test_name, status_val) in obj {
                    if let Some(status) = status_val.as_str() {
                        if status.to_lowercase() == "failed" || status.to_lowercase() == "fail" {
                            report_failed_tests.insert(test_name.clone());
                        }
                    }
                }
            }
        }
        
        println!("Found {} failed tests in report.json", report_failed_tests.len());
        
        // Check F2P and P2P tests for inconsistencies in both directions
        for test_name in &universe {
            let report_status = if report_failed_tests.contains(test_name) {
                "failed"
            } else if report_s.get(test_name) == Some(&"passed".to_string()) {
                "passed"
            } else {
                "missing" // Skip tests that are missing in report.json
            };
            
            let agent_status = agent_s.get(test_name).map(String::as_str).unwrap_or("missing");
            
            // Check for status mismatches (excluding missing cases)
            if report_status != "missing" && agent_status != "missing" && report_status != agent_status {
                match (report_status, agent_status) {
                    ("failed", "passed") => {
                        c6_hits.push(format!("{} (marked as failed in report.json but passing in agent log)", test_name));
                    },
                    ("passed", "failed") => {
                        c6_hits.push(format!("{} (marked as passed in report.json but failing in agent log)", test_name));
                    },
                    _ => {} // Other combinations like "passed" vs "ignored" could be added if needed
                }
            }
        }
        
        println!("C6 check found {} inconsistencies", c6_hits.len());
        !c6_hits.is_empty()
    } else {
        println!("C6 check skipped: missing agent log or report.json");
        false
    };
    
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
        test_data.insert("agent".to_string(), serde_json::Value::String(agent_s.get(test_name).unwrap_or(&"missing".to_string()).clone()));
        test_data.insert("report".to_string(), serde_json::Value::String(report_s.get(test_name).unwrap_or(&"missing".to_string()).clone()));
        p2p_analysis.insert(test_name.clone(), serde_json::Value::Object(test_data));
    }
    
    // Generate f2p_analysis
    let mut f2p_analysis = serde_json::Map::new();
    for test_name in fail_to_pass {
        let mut test_data = serde_json::Map::new();
        test_data.insert("base".to_string(), serde_json::Value::String(base_s.get(test_name).unwrap_or(&"missing".to_string()).clone()));
        test_data.insert("before".to_string(), serde_json::Value::String(before_s.get(test_name).unwrap_or(&"missing".to_string()).clone()));
        test_data.insert("after".to_string(), serde_json::Value::String(after_s.get(test_name).unwrap_or(&"missing".to_string()).clone()));
        test_data.insert("agent".to_string(), serde_json::Value::String(agent_s.get(test_name).unwrap_or(&"missing".to_string()).clone()));
        test_data.insert("report".to_string(), serde_json::Value::String(report_s.get(test_name).unwrap_or(&"missing".to_string()).clone()));
        f2p_analysis.insert(test_name.clone(), serde_json::Value::Object(test_data));
    }
    
    // Generate debug_log_counts
    let mut debug_log_counts = vec![
        serde_json::json!({
            "label": "base",
            "passed": base_parsed.passed.len(),
            "failed": base_parsed.failed.len(),
            "ignored": base_parsed.ignored.len(),
            "all": base_parsed.all.len(),
        }),
        serde_json::json!({
            "label": "before",
            "passed": before_parsed.passed.len(),
            "failed": before_parsed.failed.len(),
            "ignored": before_parsed.ignored.len(),
            "all": before_parsed.all.len(),
        }),
        serde_json::json!({
            "label": "after",
            "passed": after_parsed.passed.len(),
            "failed": after_parsed.failed.len(),
            "ignored": after_parsed.ignored.len(),
            "all": after_parsed.all.len(),
        }),
    ];
    if let Some(agent_parsed) = agent_parsed {
        debug_log_counts.push(serde_json::json!({
            "label": "agent",
            "passed": agent_parsed.passed.len(),
            "failed": agent_parsed.failed.len(),
            "ignored": agent_parsed.ignored.len(),
            "all": agent_parsed.all.len(),
        }));
    }
    
    serde_json::json!({
        "inputs": {
            "base_log": base_path,
            "before_log": before_path,
            "after_log": after_path,
            "agent_log": agent_path.map(|p| p.as_str()).unwrap_or(""),
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
            "c6_test_marked_failed_in_report_but_passing_in_agent": {
                "has_problem": c6,
                "examples": c6_hits
            },
            "c7_f2p_tests_in_golden_source_diff": {
                "has_problem": c7,
                "examples": c7_hits
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
        "debug_log_counts": serde_json::Value::Array(debug_log_counts)
    })
}

// Function to extract clean test name from nextest line
// This tries to intelligently parse different nextest formats without hardcoding specific crates
fn extract_test_name_from_nextest_line(full_line: &str) -> String {
    let trimmed = full_line.trim();
    
    println!("EXTRACT DEBUG: input='{}'", trimmed);
    
    // Simple approach: Just return the full test name as captured by regex
    // The nextest format is: "PASS [time] full_test_name"
    // We should preserve the full test name exactly as it appears
    
    // Special handling for known patterns in main.json:
    // 1. "miden-testing kernel_tests::..." -> keep as is
    // 2. "miden-testing::miden-integration-tests ..." -> keep as is  
    // 3. "miden-lib ..." -> keep as is
    // 4. "miden-objects ..." -> keep as is
    // 5. "miden-tx ..." -> keep as is (NEW - this was missing!)
    
    // For miden crates, the format in main.json matches exactly what's in the log
    if trimmed.starts_with("miden-") {
        let result = trimmed.to_string();
        println!("EXTRACT DEBUG: Miden crate, keeping as-is='{}'", result);
        return result;
    }
    
    // Check for double crate format: "miden-testing::miden-integration-tests scripts::faucet::test"
    if trimmed.contains("::miden-integration-tests ") {
        let result = trimmed.to_string();
        println!("EXTRACT DEBUG: Double crate format, keeping as-is='{}'", result);
        return result;
    }
    
    // Check for crate::lib format: "grillon::lib assert::json_path..." -> just the test part
    if trimmed.contains("::lib ") {
        if let Some(lib_pos) = trimmed.find("::lib ") {
            let result = trimmed[lib_pos + 6..].trim().to_string(); // 6 = len("::lib ")
            println!("EXTRACT DEBUG: crate::lib format, extracting test part='{}'", result);
            return result;
        }
    }
    
    // For other formats, check if there's a space and we should remove the crate prefix
    if let Some(space_pos) = trimmed.find(' ') {
        let crate_part = &trimmed[..space_pos];
        let test_part = &trimmed[space_pos + 1..];
        
        // If the crate part doesn't contain "::" and the test part does, remove the crate prefix
        if !crate_part.contains("::") && test_part.contains("::") {
            let result = test_part.trim().to_string();
            println!("EXTRACT DEBUG: Generic crate format, removing prefix='{}'", result);
            return result;
        }
    }
    
    // If no patterns match, return the original
    let result = trimmed.to_string();
    println!("EXTRACT DEBUG: no pattern matched, keeping original='{}'", result);
    result
}



// Test function to verify nextest parsing
#[tauri::command]
pub fn test_nextest_parsing() -> Result<String, String> {
    let test_content = r#"PASS [   0.021s] grillon assertion::impls::json_path::tests::is_eq::impl_is_eq_object_with_array_and_object
PASS [   0.155s] grillon::lib assert::json_path::json_path_does_not_match
PASS [   0.028s] miden-lib account::interface::test::test_basic_wallet_default_notes
PASS [   0.034s] miden-testing kernel_tests::tx::test_account_delta::storage_delta_for_map_slots
PASS [   0.045s] miden-testing::miden-integration-tests scripts::faucet::faucet_contract_mint_fungible_asset_fails_exceeds_max_supply
PASS [   2.877s] miden-tx auth::tx_authenticator::test::serialize_auth_key"#;
    
    println!("=== TESTING NEXTEST PARSING ===");
    
    // Test the nextest format detection
    let is_nextest = looks_nextest_format(test_content);
    println!("Detected as nextest format: {}", is_nextest);
    
    // Parse the content
    let parsed = if is_nextest {
        parse_nextest_log(test_content)
    } else {
        parse_rust_log_single_line(test_content)
    };
    
    println!("Parsed results:");
    println!("  Passed: {} tests", parsed.passed.len());
    for test in &parsed.passed {
        println!("    - {}", test);
    }
    
    // Test specific extraction for the problematic case
    println!("\n=== TESTING SPECIFIC EXTRACTION ===");
    let test_line = "miden-tx auth::tx_authenticator::test::serialize_auth_key";
    let extracted = extract_test_name_from_nextest_line(test_line);
    println!("Input: '{}'", test_line);
    println!("Extracted: '{}'", extracted);
    
    Ok(format!("Parsed {} passed tests", parsed.passed.len()))
}
