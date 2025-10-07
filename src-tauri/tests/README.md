# SWE Reviewer E2E Test Suite

This directory contains comprehensive End-to-End (E2E) tests for the SWE Reviewer system. The tests validate the complete functionality from Google Drive link validation through log analysis and violation detection.

## 🎯 Test Overview

The test suite validates 15 specific Google Drive links with known expected behaviors:

### No Violations Expected (9 cases)
- **Test 3**: https://drive.google.com/drive/folders/1rq33SVzJCs9HZHS0mqGdtYO-W_ntWsFB
- **Test 4**: https://drive.google.com/drive/folders/1N6nLBCW6CPE-BxRLUKeRREi0T3mQtEia
- **Test 5**: https://drive.google.com/drive/folders/1U5SYc5wfMU9GMWyDdiQpWBmM7cu1-1TK
- **Test 6**: https://drive.google.com/drive/folders/1AFP1OzZmpA-S56I4AS37YqBaNhE8cA_E
- **Test 7**: https://drive.google.com/drive/folders/1MA_5ZhRFiOBd24z2OruKC05pBQr5ZeGB
- **Test 8**: https://drive.google.com/drive/folders/1NpabUZ6Uv4ZY5Stjesi7EWgAHNfslUr_
- **Test 10**: https://drive.google.com/drive/folders/1tWW536Zwx2dIEYfovvkP92rnz_S3F4Wt
- **Test 12**: https://drive.google.com/drive/folders/1hlZZpb-hh6VU461cKTZnIaM1gr353m3h
- **Test 15**: https://drive.google.com/drive/folders/1Wc6SHwQUs_gndnDrVsDFv5-4SZjN14jN

### F2P (Fail-to-Pass) Violations (3 cases)
- **Test 1**: P2P or F2P missing in after - https://drive.google.com/drive/folders/1LAbDGCOkgTUKDGy9i2pgnhUlT07ews_9
- **Test 2**: P2P or F2P missing in after - https://drive.google.com/drive/folders/1rpBzsSwp4fow2xuw6q6qYk-v_a5Uv1EZ
- **Test 11**: F2P tests passing in before - https://drive.google.com/drive/folders/1kFzsfORq7uTTbbdeTXQN7oqBeJAt3Tzg

### Source Diff Violations (1 case)
- **Test 9**: F2P tests in src diff - https://drive.google.com/drive/folders/1dDjkXNPWg81VBcEGoBz2N3wv0JPjVupo

### P2P (Pass-to-Pass) Violations (2 cases)
- **Test 13**: P2P failed in base and P2P missing in base/before - https://drive.google.com/drive/folders/14j3jPC1BZ0IHm3rsIhZi5HhHP7BoO6jR
- **Test 14**: P2P missing in all logs - https://drive.google.com/drive/folders/1meg12kGotjuGLIRQJW2siN8j2jB2uyiA

## 🚀 Quick Start

### Prerequisites

1. **Google Drive Authentication**: You need Google OAuth credentials. Either:
   - Set `GOOGLE_CLIENT_SECRET` environment variable
   - Have authenticated through the SWE Reviewer app (credentials saved in settings)

2. **Optional - OpenAI API Key**: For AI-enhanced analysis features:
   ```bash
   export OPENAI_API_KEY="your-api-key-here"
   ```

### Running Tests

Use the comprehensive test runner:

```bash
# Quick integration tests (recommended for development)
./test_runner.sh quick

# Run all tests (takes 10-15 minutes)
./test_runner.sh full

# Run a single test case
./test_runner.sh single 3

# Run only tests expecting no violations
./test_runner.sh no-violations

# Run only tests expecting violations
./test_runner.sh violations

# Check compilation without running tests
./test_runner.sh check

# Show all available options
./test_runner.sh help
```

### Alternative: Using Cargo Directly

```bash
# Quick integration test
cargo test --test integration_tests test_complete_flow_no_violations

# All integration tests
cargo test --test integration_tests

# Run violation detection tests (slower)
cargo test --test integration_tests --ignored

# Run the comprehensive E2E suite
cargo run --bin e2e_runner
```

## 📁 Test Files

### Core Test Files
- **`tests/e2e_tests.rs`**: Main E2E test suite with all 15 test cases
- **`tests/integration_tests.rs`**: Fast integration tests for development
- **`tests/test_config.rs`**: Test configuration and utilities

### Test Runners
- **`test_runner.sh`**: Comprehensive test runner script (recommended)
- **`run_e2e_tests.sh`**: Simple E2E test runner
- **`src/bin/e2e_runner.rs`**: Binary entry point for E2E tests

## 🔍 What Each Test Validates

### Complete Flow Testing
Each test validates the entire SWE Reviewer pipeline:

1. **Validation** (`validate_deliverable`)
   - Validates Google Drive folder structure
   - Checks for required files (main.json, log files)
   - Verifies folder permissions and accessibility

2. **Download** (`download_deliverable`) 
   - Downloads all required files to temporary directory
   - Handles authentication token refresh
   - Manages file path organization

3. **Processing** (`process_deliverable`)
   - Organizes downloaded files
   - Creates proper directory structure
   - Prepares files for analysis

4. **Analysis** (`analyze_logs`)
   - Parses test logs (base, before, after, agent)
   - Performs rule-based violation detection
   - Generates comprehensive analysis results

### Violation Detection Rules

The tests validate detection of these rule violations:

- **C1**: P2P tests failed in base log
- **C2**: Tests failed in after log
- **C3**: F2P tests passing in before log  
- **C4**: P2P tests missing in base and not passing in before
- **C5**: Duplicate test entries in same log
- **C6**: Test status inconsistency between report.json and agent log
- **C7**: F2P tests present in source code diffs

## 📊 Test Output

### Console Output
Tests provide detailed progress information:
```
🧪 Executing Test #3: no violations
   🔗 Drive Link: https://drive.google.com/drive/folders/1rq33SVzJCs9HZHS0mqGdtYO-W_ntWsFB
   ⏳ Step 1: Validating deliverable...
   ✅ Validation successful - found 6 files to download
   ⏳ Step 2: Downloading files...
   ✅ Downloaded 6 files to /tmp/.tmpXXX
   ⏳ Step 3: Processing deliverable...
   ✅ Processing completed - status: completed
   ⏳ Step 4: Analyzing logs...
   ✅ Analysis completed successfully
   🔍 Step 5: Extracting violations...
   📊 Found violations: []
   🎯 Expected violations: []
   ✅ PASS Test #3 completed in 12.34s
```

### Generated Reports
Tests generate multiple output formats:

- **`test_reports/e2e_results_<timestamp>.json`**: Machine-readable test results
- **`test_reports/e2e_report_<timestamp>.html`**: Human-readable HTML report
- **`test_logs/`**: Individual test execution logs
- **`test_artifacts/`**: Downloaded test files (for debugging)

### HTML Report Features
The generated HTML report includes:
- Test execution summary with pass/fail counts
- Individual test details with timing information
- Violation detection results
- Error messages and debugging information
- Responsive design for viewing on different devices

## ⚡ Performance Considerations

### Test Execution Times
- **Quick integration test**: ~30 seconds
- **Single E2E test case**: ~45-90 seconds  
- **Full test suite (15 cases)**: ~10-15 minutes
- **Validation-only tests**: ~5-10 seconds per case

### Optimization Options
- Use `--release` flag for faster execution
- Run `no-violations` tests first (most reliable)
- Use `single <id>` for focused testing
- Set custom timeouts with `--timeout <seconds>`

### Rate Limiting
- 2-second delay between test cases
- Automatic retry on temporary failures
- Token refresh handling for long test runs

## 🛠️ Development & Debugging

### Adding New Test Cases
1. Add entry to `get_test_cases()` in `e2e_tests.rs`
2. Update category mapping in `test_config.rs`
3. Add expected violations to the test case definition
4. Update documentation and help text

### Debugging Failed Tests
1. Check console output for specific error messages
2. Review generated artifacts in `test_artifacts/`
3. Use `--verbose` flag for detailed logging
4. Run individual test cases with `single <id>`
5. Check authentication status if download failures occur

### Running in CI/CD
```bash
# Fast check for PRs
./test_runner.sh check && ./test_runner.sh quick

# Comprehensive testing for releases
./test_runner.sh full --release --timeout 900
```

## 🔧 Environment Configuration

### Required Environment Variables
```bash
# Google OAuth (choose one method)
export GOOGLE_CLIENT_SECRET="your-client-secret"
# OR use app-saved credentials (no env var needed)

# Optional: OpenAI API for enhanced analysis
export OPENAI_API_KEY="your-openai-key"

# Optional: Rust logging
export RUST_LOG="info"  # or "debug" for verbose output
export RUST_BACKTRACE="1"  # for detailed error traces
```

### Directory Structure
```
src-tauri/
├── tests/
│   ├── e2e_tests.rs           # Main E2E test suite
│   ├── integration_tests.rs   # Fast integration tests
│   └── test_config.rs         # Test configuration
├── src/bin/
│   └── e2e_runner.rs         # Binary test runner
├── test_runner.sh            # Main test script
├── run_e2e_tests.sh         # Simple E2E runner
├── test_logs/               # Generated test logs
├── test_reports/            # Generated reports
└── test_artifacts/          # Downloaded test files
```

## 📈 Test Metrics & Success Criteria

### Success Criteria
- **No-violation tests**: Must pass with zero rule violations
- **Violation tests**: Must detect expected violations
- **Performance**: Average test under 90 seconds
- **Reliability**: 95%+ success rate on clean runs

### Key Metrics Tracked
- Test execution time per case
- Total test suite duration  
- Violation detection accuracy
- Authentication success rate
- Download success rate
- Analysis completion rate

## 🤝 Contributing

When adding or modifying tests:

1. **Follow naming conventions**: `test_<category>_<description>`
2. **Update documentation**: Add new cases to this README
3. **Test locally**: Verify with `./test_runner.sh quick`
4. **Add expected violations**: Update test case definitions
5. **Consider performance**: Avoid unnecessarily slow operations

### Common Test Patterns
```rust
#[tokio::test]
async fn test_new_violation_case() {
    let test_case = TestCase {
        id: 16,
        drive_link: "https://drive.google.com/...".to_string(),
        expected_behavior: "new violation type".to_string(),
        expected_violations: vec!["new_violation".to_string()],
    };
    
    let result = execute_test_case(&test_case, &TestConfig::default()).await;
    assert!(result.passed, "Test should pass: {:?}", result.error);
    assert!(result.violations_found.contains(&"new_violation".to_string()));
}
```

This comprehensive test suite ensures the SWE Reviewer system works correctly across all supported scenarios and provides confidence in the system's reliability and accuracy.
