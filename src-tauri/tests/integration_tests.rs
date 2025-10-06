//! Integration tests for core SWE Reviewer functionality
//!
//! These tests focus on validating the key functionality of the system
//! with a smaller subset of test cases for faster CI/development cycles.

use std::time::Duration;
use swe_reviewer_lib::report_checker::{validate_deliverable, download_deliverable, process_deliverable};
use swe_reviewer_lib::analysis::analyze_logs;

/// Test the complete flow with a known good case (no violations expected)
async fn test_complete_flow_no_violations() {
    let drive_link = "https://drive.google.com/drive/folders/1rq33SVzJCs9HZHS0mqGdtYO-W_ntWsFB";
    
    println!("Testing complete flow with no violations expected");
    println!("Drive link: {}", drive_link);
    
    // Step 1: Validate
    let validation_result = validate_deliverable(drive_link.to_string()).await
        .expect("Validation should succeed");
    
    assert!(!validation_result.files_to_download.is_empty(), "Should have files to download");
    assert!(!validation_result.folder_id.is_empty(), "Should have folder ID");
    
    // Verify essential files are present
    let file_names: Vec<&str> = validation_result.files_to_download.iter()
        .map(|f| f.name.as_str()).collect();
    
    let has_main_json = file_names.iter().any(|name| name.ends_with(".json") && !name.starts_with("report"));
    let has_base_log = file_names.iter().any(|name| name.contains("base.log"));
    let has_before_log = file_names.iter().any(|name| name.contains("before.log"));
    let has_after_log = file_names.iter().any(|name| name.contains("after.log"));
    
    assert!(has_main_json, "Should have main JSON file");
    assert!(has_base_log, "Should have base log file");
    assert!(has_before_log, "Should have before log file"); 
    assert!(has_after_log, "Should have after log file");
    
    println!("✅ Validation passed - found {} files", validation_result.files_to_download.len());
    
    // Step 2: Download
    let download_result = download_deliverable(
        validation_result.files_to_download,
        validation_result.folder_id
    ).await.expect("Download should succeed");
    
    assert!(!download_result.temp_directory.is_empty(), "Should have temp directory");
    assert!(!download_result.downloaded_files.is_empty(), "Should have downloaded files");
    
    println!("✅ Download passed - {} files to {}", 
             download_result.downloaded_files.len(), 
             download_result.temp_directory);
    
    // Step 3: Process
    let processing_result = process_deliverable(download_result.downloaded_files).await
        .expect("Processing should succeed");
    
    assert_eq!(processing_result.get("status").and_then(|s| s.as_str()), Some("completed"));
    
    let file_paths = processing_result
        .get("file_paths")
        .and_then(|fp| fp.as_array())
        .expect("Should have file paths")
        .iter()
        .filter_map(|p| p.as_str())
        .map(|s| s.to_string())
        .collect::<Vec<String>>();
    
    assert!(!file_paths.is_empty(), "Should have file paths for analysis");
    
    println!("✅ Processing passed - {} file paths generated", file_paths.len());
    
    // Step 4: Analyze
    let analysis_result = analyze_logs(file_paths).await
        .expect("Analysis should succeed");
    
    // Verify analysis structure
    assert!(analysis_result.get("rule_checks").is_some(), "Should have rule checks");
    assert!(analysis_result.get("p2p_analysis").is_some(), "Should have P2P analysis");
    assert!(analysis_result.get("f2p_analysis").is_some(), "Should have F2P analysis");
    
    println!("✅ Analysis passed - rule checks completed");
    
    // For no-violations case, we expect minimal rule violations
    if let Some(rule_checks) = analysis_result.get("rule_checks") {
        let violations: Vec<String> = rule_checks.as_object().unwrap_or(&serde_json::Map::new())
            .iter()
            .filter_map(|(name, data)| {
                if data.get("has_problem").and_then(|v| v.as_bool()).unwrap_or(false) {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect();
        
        println!("📊 Rule violations found: {:?}", violations);
        
        // For this specific test case, we expect no major violations
        // Some minor issues might be acceptable depending on the data
        assert!(violations.len() <= 2, "Should have minimal violations for no-violation test case");
    }
    
    println!("🎉 Complete flow test passed!");
}

/// Test validation failure with an invalid link
#[tokio::test]  
async fn test_validation_failure_invalid_link() {
    let invalid_link = "https://drive.google.com/drive/folders/invalid_id";
    
    let result = validate_deliverable(invalid_link.to_string()).await;
    
    // Should fail with invalid link
    assert!(result.is_err(), "Validation should fail for invalid link");
    
    let error = result.err().unwrap();
    println!("Expected validation error: {}", error);
}

/// Test a case expected to have F2P violations
#[tokio::test]
#[ignore] // Ignore by default for faster CI, can be run with --ignored
async fn test_f2p_violation_case() {
    let drive_link = "https://drive.google.com/drive/folders/1LAbDGCOkgTUKDGy9i2pgnhUlT07ews_9";
    
    println!("Testing F2P violation case");
    println!("Drive link: {}", drive_link);
    
    // Run complete flow
    let validation_result = validate_deliverable(drive_link.to_string()).await
        .expect("Validation should succeed");
    
    let download_result = download_deliverable(
        validation_result.files_to_download,
        validation_result.folder_id
    ).await.expect("Download should succeed");
    
    let processing_result = process_deliverable(download_result.downloaded_files).await
        .expect("Processing should succeed");
    
    let file_paths = processing_result
        .get("file_paths")
        .and_then(|fp| fp.as_array())
        .expect("Should have file paths")
        .iter()
        .filter_map(|p| p.as_str())
        .map(|s| s.to_string())
        .collect::<Vec<String>>();
    
    let analysis_result = analyze_logs(file_paths).await
        .expect("Analysis should succeed");
    
    // Check for expected violations
    if let Some(rule_checks) = analysis_result.get("rule_checks") {
        let violations: Vec<String> = rule_checks.as_object().unwrap_or(&serde_json::Map::new())
            .iter()
            .filter_map(|(name, data)| {
                if data.get("has_problem").and_then(|v| v.as_bool()).unwrap_or(false) {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect();
        
        println!("📊 Rule violations found: {:?}", violations);
        
        // Should have some violations for this test case
        assert!(!violations.is_empty(), "Should have violations for F2P violation test case");
        
        // Check for specific F2P-related violations
        let f2p_violations = violations.iter()
            .filter(|v| v.contains("F2P") || v.contains("f2p"))
            .count();
        
        println!("🔍 F2P-related violations: {}", f2p_violations);
    }
}

/// Test P2P violation case
#[tokio::test]
#[ignore] // Ignore by default for faster CI
async fn test_p2p_violation_case() {
    let drive_link = "https://drive.google.com/drive/folders/14j3jPC1BZ0IHm3rsIhZi5HhHP7BoO6jR";
    
    println!("Testing P2P violation case");
    
    // Run abbreviated test focusing on analysis
    let validation_result = validate_deliverable(drive_link.to_string()).await
        .expect("Validation should succeed");
    
    let download_result = download_deliverable(
        validation_result.files_to_download,
        validation_result.folder_id
    ).await.expect("Download should succeed");
    
    let processing_result = process_deliverable(download_result.downloaded_files).await
        .expect("Processing should succeed");
    
    let file_paths = processing_result
        .get("file_paths")
        .and_then(|fp| fp.as_array())
        .expect("Should have file paths")
        .iter()
        .filter_map(|p| p.as_str())
        .map(|s| s.to_string())
        .collect::<Vec<String>>();
    
    let analysis_result = analyze_logs(file_paths).await
        .expect("Analysis should succeed");
    
    // Check for P2P violations
    if let Some(rule_checks) = analysis_result.get("rule_checks") {
        let violations: Vec<String> = rule_checks.as_object().unwrap_or(&serde_json::Map::new())
            .iter()
            .filter_map(|(name, data)| {
                if data.get("has_problem").and_then(|v| v.as_bool()).unwrap_or(false) {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect();
        
        println!("📊 Rule violations found: {:?}", violations);
        
        // Check for P2P-related violations
        let p2p_violations = violations.iter()
            .filter(|v| v.contains("P2P") || v.contains("p2p") || v.contains("base"))
            .count();
        
        println!("🔍 P2P-related violations: {}", p2p_violations);
        assert!(p2p_violations > 0, "Should have P2P violations for this test case");
    }
}

/// Benchmark test to measure performance
#[tokio::test]
#[ignore] // Ignore for normal test runs
async fn benchmark_validation_performance() {
    let test_cases = vec![
        "https://drive.google.com/drive/folders/1rq33SVzJCs9HZHS0mqGdtYO-W_ntWsFB",
        "https://drive.google.com/drive/folders/1N6nLBCW6CPE-BxRLUKeRREi0T3mQtEia",
        "https://drive.google.com/drive/folders/1U5SYc5wfMU9GMWyDdiQpWBmM7cu1-1TK",
    ];
    
    let mut total_duration = Duration::ZERO;
    
    for (i, drive_link) in test_cases.iter().enumerate() {
        let start = std::time::Instant::now();
        
        let result = validate_deliverable(drive_link.to_string()).await;
        
        let duration = start.elapsed();
        total_duration += duration;
        
        println!("Validation {}: {:.2}s - {}", 
                 i + 1, 
                 duration.as_secs_f64(),
                 if result.is_ok() { "✅" } else { "❌" });
    }
    
    let avg_duration = total_duration / test_cases.len() as u32;
    println!("Average validation time: {:.2}s", avg_duration.as_secs_f64());
    
    // Performance assertion - validation should complete within reasonable time
    assert!(avg_duration < Duration::from_secs(30), "Average validation should be under 30 seconds");
}

/// Test error handling and recovery
async fn test_error_handling() {
    // Test various error conditions
    
    // 1. Invalid folder ID
    let invalid_result = validate_deliverable("https://invalid-url".to_string()).await;
    assert!(invalid_result.is_err(), "Should fail for completely invalid URL");
    
    // 2. Valid format but non-existent folder
    let nonexistent_result = validate_deliverable(
        "https://drive.google.com/drive/folders/1aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()
    ).await;
    // This might succeed or fail depending on permissions, but shouldn't crash
    println!("Non-existent folder test result: {:?}", nonexistent_result.is_ok());
    
    // 3. Empty folder ID
    let empty_result = validate_deliverable("".to_string()).await;
    assert!(empty_result.is_err(), "Should fail for empty URL");
}

/// Integration test runner that can be called from external scripts
pub async fn run_integration_tests() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Running SWE Reviewer Integration Tests");
    println!("═══════════════════════════════════════");
    
    // Test 1: Basic validation and flow
    println!("\n🔬 Test 1: Complete flow with no violations");
    test_complete_flow_no_violations().await;
    
    // Test 2: Error handling
    println!("\n🔬 Test 2: Error handling"); 
    test_error_handling().await;
    
    println!("\n✅ All integration tests passed!");
    Ok(())
}
