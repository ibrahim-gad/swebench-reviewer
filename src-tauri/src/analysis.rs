use serde::{Deserialize, Serialize};
use std::fs;
use crate::settings::load_setting;

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
    let chunk_size = 250; // 10KB chunks for more reliable processing
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
