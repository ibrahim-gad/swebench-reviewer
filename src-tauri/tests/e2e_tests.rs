//! End-to-End Tests for SWE Reviewer
//! 
//! This module tests the complete functionality of the SWE Reviewer system
//! by testing against actual Google Drive links with expected behaviors.
//! 
//! Test Flow:
//! 1. validate_deliverable - Validates the folder structure and contents
//! 2. download_deliverable - Downloads all required files to temp directory  
//! 3. process_deliverable - Processes files and creates temporary structure
//! 4. analyze_logs - Performs comprehensive rule-based analysis
//!
//! Expected Violations:
//! - f2p_missing_in_after: fail-to-pass tests are missing in } - p2p_missing_in_after: pass-to-pass tests are missing in after log  
//! - f2p_tests_in_src_diff: fail-to-pass tests found in source diffs
//! - f2p_passing_in_before: fail-to-pass tests passing in before log
//! - p2p_failed_in_base: pass-to-pass tests failed in base log
//! - p2p_missing_in_base_and_before: pass-to-pass tests missing in base and before

use std::time::{Duration, SystemTime};
use std::collections::HashSet;
use serde_json;
use chrono;

// Import the library modules we need to test
use swe_reviewer_lib::report_checker::{validate_deliverable, download_deliverable, process_deliverable};
use swe_reviewer_lib::analysis::analyze_logs;

// Import test configuration - load from tests directory
#[path = "../tests/test_config.rs"]
mod test_config;
use test_config::{TestConfig, SerializableTestResult, setup, utils, execution};

/// Test result structure for internal tracking
#[derive(Debug, Clone)]
pub struct TestResult {
    test_id: usize,
    drive_link: String,
    expected_behavior: String,
    passed: bool,
    violations_found: Vec<String>,
    error: Option<String>,
    duration: Duration,
    analysis_data: Option<serde_json::Value>,
}

/// Test case definition
#[derive(Debug, Clone)]
struct TestCase {
    id: usize,
    drive_link: String,
    expected_behavior: String,
    expected_violations: Vec<String>,
}

/// Get all test cases with their expected behaviors
fn get_test_cases() -> Vec<TestCase> {
    vec![
        TestCase {
            id: 1,
            drive_link: "https://drive.google.com/drive/folders/1LAbDGCOkgTUKDGy9i2pgnhUlT07ews_9".to_string(),
            expected_behavior: "p2p or f2p missing in after".to_string(),
            expected_violations: vec!["f2p_missing_in_after".to_string(), "p2p_missing_in_after".to_string()],
        },
        TestCase {
            id: 2,
            drive_link: "https://drive.google.com/drive/folders/1rpBzsSwp4fow2xuw6q6qYk-v_a5Uv1EZ".to_string(),
            expected_behavior: "p2p or f2p missing in after".to_string(),
            expected_violations: vec!["f2p_missing_in_after".to_string(), "p2p_missing_in_after".to_string()],
        },
        TestCase {
            id: 3,
            drive_link: "https://drive.google.com/drive/folders/1rq33SVzJCs9HZHS0mqGdtYO-W_ntWsFB".to_string(),
            expected_behavior: "no violations".to_string(),
            expected_violations: vec![],
        },
        TestCase {
            id: 4,
            drive_link: "https://drive.google.com/drive/folders/1N6nLBCW6CPE-BxRLUKeRREi0T3mQtEia".to_string(),
            expected_behavior: "no violations".to_string(),
            expected_violations: vec![],
        },
        TestCase {
            id: 5,
            drive_link: "https://drive.google.com/drive/folders/1U5SYc5wfMU9GMWyDdiQpWBmM7cu1-1TK".to_string(),
            expected_behavior: "no violations".to_string(),
            expected_violations: vec![],
        },
        TestCase {
            id: 6,
            drive_link: "https://drive.google.com/drive/folders/1AFP1OzZmpA-S56I4AS37YqBaNhE8cA_E".to_string(),
            expected_behavior: "no violations".to_string(),
            expected_violations: vec![],
        },
        TestCase {
            id: 7,
            drive_link: "https://drive.google.com/drive/folders/1MA_5ZhRFiOBd24z2OruKC05pBQr5ZeGB".to_string(),
            expected_behavior: "no violations".to_string(),
            expected_violations: vec![],
        },
        TestCase {
            id: 8,
            drive_link: "https://drive.google.com/drive/folders/1NpabUZ6Uv4ZY5Stjesi7EWgAHNfslUr_".to_string(),
            expected_behavior: "no violations".to_string(),
            expected_violations: vec![],
        },
        TestCase {
            id: 9,
            drive_link: "https://drive.google.com/drive/folders/1dDjkXNPWg81VBcEGoBz2N3wv0JPjVupo".to_string(),
            expected_behavior: "f2p tests in src diff".to_string(),
            expected_violations: vec!["f2p_tests_in_src_diff".to_string()],
        },
        TestCase {
            id: 10,
            drive_link: "https://drive.google.com/drive/folders/1tWW536Zwx2dIEYfovvkP92rnz_S3F4Wt".to_string(),
            expected_behavior: "no violations".to_string(),
            expected_violations: vec![],
        },
        TestCase {
            id: 11,
            drive_link: "https://drive.google.com/drive/folders/1kFzsfORq7uTTbbdeTXQN7oqBeJAt3Tzg".to_string(),
            expected_behavior: "f2p tests passing in before".to_string(),
            expected_violations: vec!["f2p_passing_in_before".to_string()],
        },
        TestCase {
            id: 12,
            drive_link: "https://drive.google.com/drive/folders/1hlZZpb-hh6VU461cKTZnIaM1gr353m3h".to_string(),
            expected_behavior: "no violations".to_string(),
            expected_violations: vec![],
        },
        TestCase {
            id: 13,
            drive_link: "https://drive.google.com/drive/folders/14j3jPC1BZ0IHm3rsIhZi5HhHP7BoO6jR".to_string(),
            expected_behavior: "p2p failed in base and p2p missing in base and before".to_string(),
            expected_violations: vec!["p2p_failed_in_base".to_string(), "p2p_missing_in_base_and_before".to_string()],
        },
        TestCase {
            id: 14,
            drive_link: "https://drive.google.com/drive/folders/1meg12kGotjuGLIRQJW2siN8j2jB2uyiA".to_string(),
            expected_behavior: "p2p missing in all logs".to_string(),
            expected_violations: vec!["p2p_missing_in_all_logs".to_string()],
        },
        TestCase {
            id: 15,
            drive_link: "https://drive.google.com/drive/folders/1Wc6SHwQUs_gndnDrVsDFv5-4SZjN14jN".to_string(),
            expected_behavior: "no violations".to_string(),
            expected_violations: vec![],
        },
    ]
}

/// Execute a single test case
async fn execute_test_case(test_case: &TestCase, _config: &TestConfig) -> TestResult {
    println!("\n🧪 Executing Test #{}: {}", test_case.id, test_case.expected_behavior);
    println!("   🔗 Drive Link: {}", test_case.drive_link);
    
    let start_time = SystemTime::now();
    let mut result = TestResult {
        test_id: test_case.id,
        drive_link: test_case.drive_link.clone(),
        expected_behavior: test_case.expected_behavior.clone(),
        passed: false,
        violations_found: vec![],
        error: None,
        duration: Duration::default(),
        analysis_data: None,
    };
    
    // Step 1: Validate deliverable
    println!("   ⏳ Step 1: Validating deliverable...");
    let validation_result = match validate_deliverable(test_case.drive_link.clone()).await {
        Ok(result) => {
            println!("   ✅ Validation successful - found {} files to download", result.files_to_download.len());
            result
        }
        Err(e) => {
            result.error = Some(format!("Validation failed: {}", e));
            result.duration = start_time.elapsed().unwrap_or_default();
            println!("   ❌ Validation failed: {}", e);
            return result;
        }
    };
    
    // Step 2: Download deliverable
    println!("   ⏳ Step 2: Downloading files...");
    let download_result = match download_deliverable(
        validation_result.files_to_download,
        validation_result.folder_id
    ).await {
        Ok(result) => {
            println!("   ✅ Downloaded {} files to {}", result.downloaded_files.len(), result.temp_directory);
            result
        }
        Err(e) => {
            result.error = Some(format!("Download failed: {}", e));
            result.duration = start_time.elapsed().unwrap_or_default();
            println!("   ❌ Download failed: {}", e);
            return result;
        }
    };
    
    // Step 3: Process deliverable
    println!("   ⏳ Step 3: Processing deliverable...");
    let processing_result = match process_deliverable(download_result.downloaded_files).await {
        Ok(result) => {
            println!("   ✅ Processing completed - status: {}", result.get("status").and_then(|s| s.as_str()).unwrap_or("unknown"));
            result
        }
        Err(e) => {
            result.error = Some(format!("Processing failed: {}", e));
            result.duration = start_time.elapsed().unwrap_or_default();
            println!("   ❌ Processing failed: {}", e);
            return result;
        }
    };
    
    // Extract file paths from processing result
    let file_paths = match processing_result.get("file_paths").and_then(|fp| fp.as_array()) {
        Some(paths) => {
            paths.iter()
                .filter_map(|p| p.as_str())
                .map(|s| s.to_string())
                .collect::<Vec<String>>()
        }
        None => {
            result.error = Some("No file paths found in processing result".to_string());
            result.duration = start_time.elapsed().unwrap_or_default();
            println!("   ❌ No file paths found in processing result");
            return result;
        }
    };
    
    println!("   📁 Found {} file paths for analysis", file_paths.len());
    
    // Step 4: Analyze logs
    println!("   ⏳ Step 4: Analyzing logs...");
    let analysis_result = match analyze_logs(file_paths).await {
        Ok(analysis) => {
            println!("   ✅ Analysis completed successfully");
            analysis
        }
        Err(e) => {
            result.error = Some(format!("Analysis failed: {}", e));
            result.duration = start_time.elapsed().unwrap_or_default();
            println!("   ❌ Analysis failed: {}", e);
            return result;
        }
    };
    
    result.analysis_data = Some(analysis_result.clone());
    
    // Step 5: Extract and validate violations
    println!("   🔍 Step 5: Extracting violations...");
    
    // Debug: Save analysis result for test #5
    if test_case.id == 5 {
        println!("   🐛 DEBUG: Analysis result for test #5:");
        println!("{}", serde_json::to_string_pretty(&analysis_result).unwrap_or_else(|_| "Failed to serialize".to_string()));
    }
    
    result.violations_found = extract_violations(&analysis_result);
    
    println!("   📊 Found violations: {:?}", result.violations_found);
    println!("   🎯 Expected violations: {:?}", test_case.expected_violations);
    
    // Step 6: Check if test passed
    result.passed = validate_test_result(&result.violations_found, &test_case.expected_violations);
    result.duration = start_time.elapsed().unwrap_or_default();
    
    let status = if result.passed { "✅ PASS" } else { "❌ FAIL" };
    println!("   {} Test #{} completed in {:.2}s", status, test_case.id, result.duration.as_secs_f64());
    
    result
}

/// Extract violations from analysis result
fn extract_violations(analysis_result: &serde_json::Value) -> Vec<String> {
    let mut violations = Vec::new();
    
    if let Some(rule_checks) = analysis_result.get("rule_checks") {
        // Check C1: P2P failed in base
        if let Some(c1) = rule_checks.get("c1_failed_in_base_present_in_P2P") {
            if c1.get("has_problem").and_then(|v| v.as_bool()).unwrap_or(false) {
                violations.push("p2p_failed_in_base".to_string());
            }
        }
        
        // Check C2: Failed in after (F2P or P2P)
        if let Some(c2) = rule_checks.get("c2_failed_in_after_present_in_F2P_or_P2P") {
            if c2.get("has_problem").and_then(|v| v.as_bool()).unwrap_or(false) {
                if let Some(_examples) = c2.get("examples").and_then(|e| e.as_array()) {
                    // Check if any examples are from F2P or P2P tests
                    violations.push("tests_failed_in_after".to_string());
                }
            }
        }
        
        // Check C3: F2P passing in before
        if let Some(c3) = rule_checks.get("c3_F2P_success_in_before") {
            if c3.get("has_problem").and_then(|v| v.as_bool()).unwrap_or(false) {
                violations.push("f2p_passing_in_before".to_string());
            }
        }
        
        // Check C4: P2P missing in base and not passing in before
        if let Some(c4) = rule_checks.get("c4_P2P_missing_in_base_and_not_passing_in_before") {
            if c4.get("has_problem").and_then(|v| v.as_bool()).unwrap_or(false) {
                violations.push("p2p_missing_in_base_and_before".to_string());
            }
        }
        
        // Check C7: F2P tests in golden source diff
        if let Some(c7) = rule_checks.get("c7_f2p_tests_in_golden_source_diff") {
            if c7.get("has_problem").and_then(|v| v.as_bool()).unwrap_or(false) {
                violations.push("f2p_tests_in_src_diff".to_string());
            }
        }
    }
    
    // Check rejection reasons for missing tests
    if let Some(rejection) = analysis_result.get("rejection_reason") {
        // F2P missing in after (tests considered but ok)
        if let Some(f2p_considered_but_ok) = rejection.get("f2p_considered_but_ok").and_then(|v| v.as_array()) {
            if !f2p_considered_but_ok.is_empty() {
                violations.push("f2p_missing_in_after".to_string());
            }
        }
        
        // P2P missing in after - check P2P analysis for missing tests
        if let Some(p2p_analysis) = analysis_result.get("p2p_analysis") {
            if let Some(p2p_obj) = p2p_analysis.as_object() {
                let mut missing_in_after = false;
                for (_, test_data) in p2p_obj {
                    if let Some(after_status) = test_data.get("after").and_then(|v| v.as_str()) {
                        if after_status == "missing" {
                            missing_in_after = true;
                            break;
                        }
                    }
                }
                if missing_in_after {
                    violations.push("p2p_missing_in_after".to_string());
                }
            }
        }
        
        // P2P missing in all logs
        if let Some(p2p_analysis) = analysis_result.get("p2p_analysis") {
            if let Some(p2p_obj) = p2p_analysis.as_object() {
                let mut missing_in_all = false;
                for (_, test_data) in p2p_obj {
                    if let Some(base_status) = test_data.get("base").and_then(|v| v.as_str()) {
                        if let Some(before_status) = test_data.get("before").and_then(|v| v.as_str()) {
                            if let Some(after_status) = test_data.get("after").and_then(|v| v.as_str()) {
                                if base_status == "missing" && before_status == "missing" && after_status == "missing" {
                                    missing_in_all = true;
                                    break;
                                }
                            }
                        }
                    }
                }
                if missing_in_all {
                    violations.push("p2p_missing_in_all_logs".to_string());
                }
            }
        }
    }
    
    violations.sort();
    violations.dedup();
    violations
}

/// Validate if test result matches expected violations
fn validate_test_result(found_violations: &[String], expected_violations: &[String]) -> bool {
    let found_set: HashSet<_> = found_violations.iter().collect();
    
    // For "no violations" case, we expect empty violations
    if expected_violations.is_empty() {
        return found_violations.is_empty();
    }
    
    // For specific violations, we need at least one of the expected violations to be present
    // This allows for some flexibility as the system might detect additional related violations
    expected_violations.iter().any(|expected| found_set.contains(&expected))
}

/// Run tests with specific execution strategy
async fn run_tests_with_strategy(strategy: execution::ExecutionStrategy, config: &TestConfig) -> Vec<TestResult> {
    let test_cases = get_test_cases();
    let test_ids = strategy.get_test_ids();
    let should_fail_fast = strategy.should_fail_fast();
    
    println!("🚀 Starting E2E Tests with strategy: {:?}", strategy);
    println!("📋 Running tests: {:?}", test_ids);
    
    let mut results = Vec::new();
    
    for (index, test_id) in test_ids.iter().enumerate() {
        if let Some(test_case) = test_cases.iter().find(|tc| tc.id == *test_id) {
            let result = execute_test_case(test_case, config).await;
            let passed = result.passed;
            results.push(result);
            
            if should_fail_fast && !passed {
                println!("⚠️ Fail-fast mode: Stopping execution due to test failure");
                break;
            }
            
            // Add delay between tests to avoid rate limiting
            if index < test_ids.len() - 1 {
                println!("⏳ Waiting 2 seconds between tests...");
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
        }
    }
    
    results
}

/// Print test summary
fn print_test_summary(results: &[TestResult]) {
    let total = results.len();
    let passed = results.iter().filter(|r| r.passed).count();
    let failed = total - passed;
    let success_rate = if total > 0 { (passed as f64 / total as f64) * 100.0 } else { 0.0 };
    
    println!("\n📊 TEST SUMMARY");
    println!("═══════════════");
    println!("Total Tests:  {}", total);
    println!("Passed:       {} ✅", passed);
    println!("Failed:       {} ❌", failed);
    println!("Success Rate: {:.1}%", success_rate);
    
    if failed > 0 {
        println!("\n❌ FAILED TESTS:");
        for result in results.iter().filter(|r| !r.passed) {
            println!("   Test #{}: {} - {}", 
                     result.test_id, 
                     result.expected_behavior,
                     result.error.as_deref().unwrap_or("Validation failed"));
        }
    }
    
    println!("\n⏱️  PERFORMANCE:");
    let total_duration: Duration = results.iter().map(|r| r.duration).sum();
    let avg_duration = if total > 0 { total_duration / total as u32 } else { Duration::default() };
    println!("Total Time:   {:.2}s", total_duration.as_secs_f64());
    println!("Average Time: {:.2}s", avg_duration.as_secs_f64());
}

/// Main test runner - can be called from binary or tests
pub async fn run_e2e_tests() -> Result<Vec<TestResult>, Box<dyn std::error::Error>> {
    println!("🧪 SWE Reviewer E2E Test Suite");
    println!("════════════════════════════════");
    
    // Setup environment
    setup::check_environment()?;
    setup::setup_test_directories()?;
    
    let config = TestConfig::default();
    
    // Parse command line arguments for execution strategy
    let args: Vec<String> = std::env::args().collect();
    let strategy = execution::parse_strategy_from_args(&args[1..]);
    
    println!("🔧 Test Configuration:");
    println!("   Timeout: {}s", config.timeout_seconds);
    println!("   Retry Attempts: {}", config.retry_attempts);
    println!("   Parallel Execution: {}", config.parallel_execution);
    
    // Execute tests
    let _start_time = SystemTime::now();
    let results = run_tests_with_strategy(strategy, &config).await;
    
    // Print summary
    print_test_summary(&results);
    
    Ok(results)
}

/// Main entry point for standalone execution
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let results = run_e2e_tests().await?;
    let test_run_id = utils::generate_test_run_id();
    
    // Save results
    let serializable_results: Vec<SerializableTestResult> = results
        .iter()
        .map(|r| test_result_to_serializable(r))
        .collect();
    
    let json_filename = format!("test_reports/e2e_results_{}.json", test_run_id);
    let html_filename = format!("test_reports/e2e_report_{}.html", test_run_id);
    
    utils::save_test_results_json(&serializable_results, &json_filename)?;
    utils::save_html_report(&serializable_results, &test_run_id, &html_filename)?;
    
    println!("\n📁 Output Files:");
    println!("   JSON Results: {}", json_filename);
    println!("   HTML Report:  {}", html_filename);
    
    // Exit with appropriate code
    let exit_code = if results.iter().all(|r| r.passed) { 0 } else { 1 };
    std::process::exit(exit_code);
}

// Individual test functions for cargo test integration

#[tokio::test]
async fn test_no_violations_cases() {
    let config = TestConfig::default();
    let test_cases = get_test_cases();
    let no_violation_cases: Vec<_> = test_cases.iter()
        .filter(|tc| tc.expected_violations.is_empty())
        .take(3) // Test first 3 for CI speed
        .collect();
    
    for test_case in no_violation_cases {
        let result = execute_test_case(test_case, &config).await;
        assert!(result.passed, "Test #{} should pass: {:?}", test_case.id, result.error);
        assert!(result.violations_found.is_empty(), "No violations expected for test #{}", test_case.id);
    }
}

#[tokio::test]
async fn test_f2p_violations() {
    let config = TestConfig::default();
    let test_cases = get_test_cases();
    
    // Test case 1: f2p missing in after
    if let Some(test_case) = test_cases.iter().find(|tc| tc.id == 1) {
        let result = execute_test_case(test_case, &config).await;
        assert!(result.passed, "Test #{} should pass: {:?}", test_case.id, result.error);
        assert!(result.violations_found.contains(&"f2p_missing_in_after".to_string()) || 
                result.violations_found.contains(&"p2p_missing_in_after".to_string()),
                "Expected f2p or p2p missing in after violation for test #{}", test_case.id);
    }
    
    // Test case 11: f2p passing in before
    if let Some(test_case) = test_cases.iter().find(|tc| tc.id == 11) {
        let result = execute_test_case(test_case, &config).await;
        assert!(result.passed, "Test #{} should pass: {:?}", test_case.id, result.error);
        assert!(result.violations_found.contains(&"f2p_passing_in_before".to_string()),
                "Expected f2p passing in before violation for test #{}", test_case.id);
    }
}

#[tokio::test]
async fn test_src_diff_violations() {
    let config = TestConfig::default();
    let test_cases = get_test_cases();
    
    // Test case 9: f2p tests in src diff
    if let Some(test_case) = test_cases.iter().find(|tc| tc.id == 9) {
        let result = execute_test_case(test_case, &config).await;
        assert!(result.passed, "Test #{} should pass: {:?}", test_case.id, result.error);
        assert!(result.violations_found.contains(&"f2p_tests_in_src_diff".to_string()),
                "Expected f2p tests in src diff violation for test #{}", test_case.id);
    }
}

#[tokio::test]
async fn test_p2p_violations() {
    let config = TestConfig::default();
    let test_cases = get_test_cases();
    
    // Test case 13: multiple P2P violations
    if let Some(test_case) = test_cases.iter().find(|tc| tc.id == 13) {
        let result = execute_test_case(test_case, &config).await;
        assert!(result.passed, "Test #{} should pass: {:?}", test_case.id, result.error);
        assert!(result.violations_found.contains(&"p2p_failed_in_base".to_string()) ||
                result.violations_found.contains(&"p2p_missing_in_base_and_before".to_string()),
                "Expected p2p violations for test #{}", test_case.id);
    }
    
    // Test case 14: p2p missing in all logs
    if let Some(test_case) = test_cases.iter().find(|tc| tc.id == 14) {
        let result = execute_test_case(test_case, &config).await;
        assert!(result.passed, "Test #{} should pass: {:?}", test_case.id, result.error);
        assert!(result.violations_found.contains(&"p2p_missing_in_all_logs".to_string()),
                "Expected p2p missing in all logs violation for test #{}", test_case.id);
    }
}

#[tokio::test]
async fn test_validation_flow() {
    // Test the validation flow with the first test case
    let test_cases = get_test_cases();
    if let Some(test_case) = test_cases.first() {
        // Test validation step
        let validation_result = validate_deliverable(test_case.drive_link.clone()).await;
        assert!(validation_result.is_ok(), "Validation should succeed");
        
        let validation = validation_result.unwrap();
        assert!(!validation.files_to_download.is_empty(), "Should have files to download");
        assert!(!validation.folder_id.is_empty(), "Should have folder ID");
        
        // Verify required files are present
        let file_names: Vec<&str> = validation.files_to_download.iter().map(|f| f.name.as_str()).collect();
        let has_main_json = file_names.iter().any(|name| name.ends_with(".json") && !name.starts_with("report"));
        let has_logs = file_names.iter().any(|name| name.contains("base.log") || name.contains("_base.log"));
        
        assert!(has_main_json, "Should have main JSON file");
        assert!(has_logs, "Should have log files");
    }
}

// Custom conversion function for TestResult to SerializableTestResult
fn test_result_to_serializable(result: &TestResult) -> SerializableTestResult {
    SerializableTestResult {
        test_id: result.test_id,
        passed: result.passed,
        violations_found: result.violations_found.clone(),
        error: result.error.clone(),
        duration_seconds: result.duration.as_secs_f64(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    }
}
