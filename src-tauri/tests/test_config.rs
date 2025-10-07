/// Configuration for E2E tests
/// This module provides utilities for managing test execution

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Test execution configuration
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub timeout_seconds: u64,
    pub retry_attempts: u32,
    pub parallel_execution: bool,
    pub save_logs: bool,
    pub log_directory: String,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: std::env::var("E2E_TIMEOUT_SECONDS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(300), // 5 minutes default
            retry_attempts: std::env::var("E2E_RETRY_ATTEMPTS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1),
            parallel_execution: std::env::var("E2E_PARALLEL")
                .map(|s| s.to_lowercase() == "true")
                .unwrap_or(false), // Avoid rate limiting by default
            save_logs: std::env::var("E2E_SAVE_LOGS")
                .map(|s| s.to_lowercase() != "false")
                .unwrap_or(true),
            log_directory: std::env::var("E2E_LOG_DIR")
                .unwrap_or_else(|_| "test_logs".to_string()),
        }
    }
}

impl TestConfig {
    /// Load configuration from environment variables and local overrides
    pub fn load() -> Self {
        let mut config = Self::default();
        
        // Try to load local configuration override if it exists
        if let Ok(local_config) = Self::load_local_config() {
            config = local_config;
        }
        
        config
    }
    
    /// Attempt to load configuration from a local file (not committed to git)
    fn load_local_config() -> Result<Self, Box<dyn std::error::Error>> {
        use std::fs;
        
        // Try to load from test_config.local.rs if it exists
        let local_config_path = "src-tauri/tests/test_config.local.rs";
        if fs::metadata(local_config_path).is_ok() {
            println!("📁 Found local test configuration at {}", local_config_path);
            // For now, just return default - in the future this could parse the local file
        }
        
        // Try to load from .env.test
        let env_file_path = "src-tauri/tests/.env.test";
        if fs::metadata(env_file_path).is_ok() {
            println!("🔧 Loading test environment from {}", env_file_path);
            let env_content = fs::read_to_string(env_file_path)?;
            for line in env_content.lines() {
                if let Some((key, value)) = line.split_once('=') {
                    std::env::set_var(key.trim(), value.trim());
                }
            }
        }
        
        Ok(Self::default())
    }
}

/// Test categories for organizing test execution
#[derive(Debug, Clone, PartialEq)]
pub enum TestCategory {
    NoViolations,
    F2PViolations,
    P2PViolations,
    SrcDiffViolations,
    MultipleViolations,
}

/// Test result for JSON serialization
#[derive(Debug, Serialize, Deserialize)]
pub struct SerializableTestResult {
    pub test_id: usize,
    pub passed: bool,
    pub violations_found: Vec<String>,
    pub error: Option<String>,
    pub duration_seconds: f64,
    pub timestamp: String,
}

/// Get test category mapping
pub fn get_test_categories() -> HashMap<usize, TestCategory> {
    let mut categories = HashMap::new();
    
    // No violations expected
    categories.insert(3, TestCategory::NoViolations);
    categories.insert(4, TestCategory::NoViolations);
    categories.insert(5, TestCategory::NoViolations);
    categories.insert(6, TestCategory::NoViolations);
    categories.insert(7, TestCategory::NoViolations);
    categories.insert(8, TestCategory::NoViolations);
    categories.insert(10, TestCategory::NoViolations);
    categories.insert(12, TestCategory::NoViolations);
    categories.insert(15, TestCategory::NoViolations);
    
    // F2P violations
    categories.insert(1, TestCategory::F2PViolations);
    categories.insert(2, TestCategory::F2PViolations);
    categories.insert(11, TestCategory::F2PViolations);
    
    // Src diff violations
    categories.insert(9, TestCategory::SrcDiffViolations);
    
    // P2P violations
    categories.insert(14, TestCategory::P2PViolations);
    
    // Multiple violations
    categories.insert(13, TestCategory::MultipleViolations);
    
    categories
}

/// Get test IDs by category
pub fn get_test_ids_by_category(category: TestCategory) -> Vec<usize> {
    get_test_categories()
        .into_iter()
        .filter_map(|(id, cat)| if cat == category { Some(id) } else { None })
        .collect()
}

/// Priority order for test execution (easier tests first)
pub fn get_test_execution_order() -> Vec<usize> {
    vec![
        // Start with no violations (should be fastest and most reliable)
        3, 4, 5, 6, 7, 8, 10, 12, 15,
        // Then simple F2P violations
        1, 2, 11,
        // Then src diff violations
        9,
        // Then P2P violations
        14,
        // Finally multiple violations (most complex)
        13,
    ]
}

/// Environment setup utilities
pub mod setup {
    use std::env;
    
    /// Check if required environment variables are set
    pub fn check_environment() -> Result<(), String> {
        println!("🔧 Checking environment setup...");
        
        // Check for authentication - we need Google credentials
        let auth_methods = vec![
            ("GOOGLE_CLIENT_SECRET", "Google OAuth client secret"),
            ("SWE_REVIEWER_GOOGLE_CLIENT_SECRET", "Alternative Google client secret path"),
        ];
        
        let mut auth_found = false;
        for (env_var, description) in auth_methods {
            if env::var(env_var).is_ok() {
                println!("  ✅ Found {}", description);
                auth_found = true;
                break;
            }
        }
        
        if !auth_found {
            println!("  ⚠️ No Google OAuth credentials found in environment");
            println!("  💡 Tests will attempt to use saved credentials from settings");
        }
        
        // Check for OpenAI API key (optional for analysis)
        if env::var("OPENAI_API_KEY").is_ok() {
            println!("  ✅ OpenAI API key found - AI analysis enabled");
        } else {
            println!("  ⚠️ OPENAI_API_KEY not set - analysis features may be limited");
        }
        
        println!("  ✅ Environment check completed");
        Ok(())
    }
    
    /// Create test output directory
    pub fn create_output_dir(dir: &str) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(dir)?;
        println!("📁 Created output directory: {}", dir);
        Ok(())
    }
    
    /// Setup test directories
    pub fn setup_test_directories() -> Result<(), std::io::Error> {
        create_output_dir("test_logs")?;
        create_output_dir("test_reports")?;
        create_output_dir("test_artifacts")?;
        Ok(())
    }
}

/// Test utilities
pub mod utils {
    use std::time::{SystemTime, UNIX_EPOCH};
    use super::SerializableTestResult;
    
    /// Generate a unique test run ID
    pub fn generate_test_run_id() -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        format!("test_run_{}", timestamp)
    }
    
    /// Format test duration
    pub fn format_duration(start: SystemTime) -> String {
        let elapsed = start.elapsed().unwrap_or_default();
        format!("{:.2}s", elapsed.as_secs_f64())
    }
    
    /// Convert test result data to SerializableTestResult
    pub fn create_serializable_result(test_id: usize, passed: bool, violations: Vec<String>, error: Option<String>, duration: f64) -> SerializableTestResult {
        SerializableTestResult {
            test_id,
            passed,
            violations_found: violations,
            error,
            duration_seconds: duration,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
    
    /// Save test results to JSON file
    pub fn save_test_results_json(results: &[SerializableTestResult], filename: &str) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(results)?;
        std::fs::write(filename, json)?;
        println!("💾 Saved test results to: {}", filename);
        Ok(())
    }
    
    /// Generate HTML report
    pub fn generate_html_report(results: &[SerializableTestResult], test_run_id: &str) -> String {
        let mut html = String::new();
        
        html.push_str("<!DOCTYPE html><html><head>");
        html.push_str("<title>SWE Reviewer E2E Test Report</title>");
        html.push_str("<style>");
        html.push_str("body { font-family: Arial, sans-serif; margin: 20px; }");
        html.push_str(".passed { color: green; } .failed { color: red; }");
        html.push_str("table { border-collapse: collapse; width: 100%; }");
        html.push_str("th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }");
        html.push_str("th { background-color: #f2f2f2; }");
        html.push_str("</style></head><body>");
        
        html.push_str(&format!("<h1>SWE Reviewer E2E Test Report</h1>"));
        html.push_str(&format!("<p>Test Run ID: {}</p>", test_run_id));
        html.push_str(&format!("<p>Generated: {}</p>", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));
        
        let passed_count = results.iter().filter(|r| r.passed).count();
        let total_count = results.len();
        let success_rate = if total_count > 0 { (passed_count as f64 / total_count as f64) * 100.0 } else { 0.0 };
        
        html.push_str(&format!("<h2>Summary</h2>"));
        html.push_str(&format!("<p>Total Tests: {}</p>", total_count));
        html.push_str(&format!("<p>Passed: <span class=\"passed\">{}</span></p>", passed_count));
        html.push_str(&format!("<p>Failed: <span class=\"failed\">{}</span></p>", total_count - passed_count));
        html.push_str(&format!("<p>Success Rate: {:.1}%</p>", success_rate));
        
        html.push_str("<h2>Test Details</h2>");
        html.push_str("<table>");
        html.push_str("<tr><th>Test ID</th><th>Status</th><th>Duration</th><th>Violations Found</th><th>Error</th></tr>");
        
        for result in results {
            let status_class = if result.passed { "passed" } else { "failed" };
            let status_text = if result.passed { "PASS" } else { "FAIL" };
            let violations = if result.violations_found.is_empty() { 
                "None".to_string() 
            } else { 
                result.violations_found.join(", ") 
            };
            let error = result.error.as_deref().unwrap_or("");
            
            html.push_str(&format!(
                "<tr><td>{}</td><td class=\"{}\">{}</td><td>{:.2}s</td><td>{}</td><td>{}</td></tr>",
                result.test_id, status_class, status_text, result.duration_seconds, violations, error
            ));
        }
        
        html.push_str("</table>");
        html.push_str("</body></html>");
        
        html
    }
    
    /// Save HTML report
    pub fn save_html_report(results: &[SerializableTestResult], test_run_id: &str, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
        let html = generate_html_report(results, test_run_id);
        std::fs::write(filename, html)?;
        println!("📊 Saved HTML report to: {}", filename);
        Ok(())
    }
}

/// Test execution strategies
pub mod execution {
    use super::{TestCategory, get_test_ids_by_category, get_test_execution_order};
    
    /// Execution strategy for tests
    #[derive(Debug, Clone)]
    pub enum ExecutionStrategy {
        All,
        ByCategory(TestCategory),
        ByIds(Vec<usize>),
        Sequential,
        FailFast,
    }
    
    impl ExecutionStrategy {
        /// Get test IDs for this strategy
        pub fn get_test_ids(&self) -> Vec<usize> {
            match self {
                ExecutionStrategy::All => get_test_execution_order(),
                ExecutionStrategy::ByCategory(category) => get_test_ids_by_category(category.clone()),
                ExecutionStrategy::ByIds(ids) => ids.clone(),
                ExecutionStrategy::Sequential => get_test_execution_order(),
                ExecutionStrategy::FailFast => get_test_execution_order(),
            }
        }
        
        /// Should stop on first failure
        pub fn should_fail_fast(&self) -> bool {
            matches!(self, ExecutionStrategy::FailFast)
        }
    }
    
    /// Parse execution strategy from command line args
    pub fn parse_strategy_from_args(args: &[String]) -> ExecutionStrategy {
        if args.is_empty() {
            return ExecutionStrategy::All;
        }
        
        match args[0].as_str() {
            "--no-violations" => ExecutionStrategy::ByCategory(TestCategory::NoViolations),
            "--f2p-violations" => ExecutionStrategy::ByCategory(TestCategory::F2PViolations),
            "--p2p-violations" => ExecutionStrategy::ByCategory(TestCategory::P2PViolations),
            "--src-diff" => ExecutionStrategy::ByCategory(TestCategory::SrcDiffViolations),
            "--multiple" => ExecutionStrategy::ByCategory(TestCategory::MultipleViolations),
            "--fail-fast" => ExecutionStrategy::FailFast,
            "--sequential" => ExecutionStrategy::Sequential,
            _ => {
                // Try to parse as test IDs
                let test_ids: Vec<usize> = args
                    .iter()
                    .filter_map(|arg| arg.parse().ok())
                    .collect();
                
                if test_ids.is_empty() {
                    ExecutionStrategy::All
                } else {
                    ExecutionStrategy::ByIds(test_ids)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_category_mapping() {
        let categories = get_test_categories();
        
        // Check that we have mappings for all test IDs 1-15
        for id in 1..=15 {
            assert!(categories.contains_key(&id), "Missing category for test ID {}", id);
        }
        
        // Check specific categorizations
        assert_eq!(categories.get(&3), Some(&TestCategory::NoViolations));
        assert_eq!(categories.get(&1), Some(&TestCategory::F2PViolations));
        assert_eq!(categories.get(&9), Some(&TestCategory::SrcDiffViolations));
        assert_eq!(categories.get(&13), Some(&TestCategory::MultipleViolations));
    }
    
    #[test]
    fn test_execution_order() {
        let order = get_test_execution_order();
        
        // Should contain all test IDs
        assert_eq!(order.len(), 15);
        
        // Should start with no-violation tests
        assert!(order[0..9].iter().all(|&id| {
            get_test_categories().get(&id) == Some(&TestCategory::NoViolations)
        }));
    }
    
    #[test]
    fn test_get_ids_by_category() {
        let no_violations = get_test_ids_by_category(TestCategory::NoViolations);
        assert!(!no_violations.is_empty());
        assert!(no_violations.contains(&3));
        
        let f2p_violations = get_test_ids_by_category(TestCategory::F2PViolations);
        assert!(f2p_violations.contains(&1));
    }
    
    #[test]
    fn test_execution_strategy() {
        use execution::ExecutionStrategy;
        
        let all_strategy = ExecutionStrategy::All;
        let all_ids = all_strategy.get_test_ids();
        assert_eq!(all_ids.len(), 15);
        
        let category_strategy = ExecutionStrategy::ByCategory(TestCategory::NoViolations);
        let category_ids = category_strategy.get_test_ids();
        assert!(category_ids.len() > 0);
        assert!(category_ids.iter().all(|&id| id <= 15));
        
        let fail_fast = ExecutionStrategy::FailFast;
        assert!(fail_fast.should_fail_fast());
        
        let normal = ExecutionStrategy::All;
        assert!(!normal.should_fail_fast());
    }
}