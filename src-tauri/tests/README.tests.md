# E2E Test Configuration

## Overview

The E2E test configuration system uses a layered approach to handle both committed configuration and local overrides safely.

## Configuration Files

### ✅ **Safe to Commit** (Public Configuration)
- `test_config.rs` - Main configuration module with defaults
- `test_config.example.rs` - Example configuration template  
- `.env.test.example` - Example environment variable template
- `README.tests.md` - This documentation

### ❌ **Not Committed** (Local/Sensitive Configuration)
- `test_config.local.rs` - Local configuration overrides
- `.env.test` - Local environment variables
- `test_artifacts/*.secret` - Any sensitive test data
- `test_data/credentials.json` - Authentication credentials

## Configuration Priority (highest to lowest)

1. **Environment Variables** (e.g., `E2E_TIMEOUT_SECONDS=600`)
2. **Local .env.test file** (for development convenience)
3. **Default values** (in `test_config.rs`)

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `E2E_TIMEOUT_SECONDS` | Test timeout in seconds | `300` |
| `E2E_RETRY_ATTEMPTS` | Number of retry attempts | `1` |
| `E2E_PARALLEL` | Enable parallel execution | `false` |
| `E2E_SAVE_LOGS` | Save detailed logs | `true` |
| `E2E_LOG_DIR` | Log directory path | `test_logs` |

## Setup for Development

1. **Copy the example environment file:**
   ```bash
   cp src-tauri/tests/.env.test.example src-tauri/tests/.env.test
   ```

2. **Customize your local settings:**
   ```bash
   # Edit .env.test with your preferences
   E2E_TIMEOUT_SECONDS=600
   E2E_PARALLEL=true
   E2E_SAVE_LOGS=true
   ```

3. **Run tests:**
   ```bash
   cd src-tauri
   cargo run --bin e2e_runner
   ```

## Security Best Practices

- ✅ **DO commit:** Default configurations, test structure, documentation
- ❌ **DON'T commit:** API keys, authentication tokens, personal settings
- ✅ **DO use:** Environment variables for sensitive data
- ✅ **DO use:** The application's settings system for persistent auth
- ❌ **DON'T hardcode:** Credentials in any configuration file

## Authentication Handling

The tests will attempt to authenticate using:
1. **Saved credentials** from the application's settings system
2. **Environment variables** (`GOOGLE_CLIENT_SECRET`, etc.)
3. **Interactive prompts** if no credentials are found

No credentials are stored in the test configuration files themselves.

## Examples

### Run all tests with custom timeout:
```bash
E2E_TIMEOUT_SECONDS=600 cargo run --bin e2e_runner
```

### Run specific test categories:
```bash
cargo run --bin e2e_runner -- --no-violations
```

### Run with parallel execution:
```bash
E2E_PARALLEL=true cargo run --bin e2e_runner
```

### Run specific test IDs:
```bash
cargo run --bin e2e_runner -- 1 2 3
```
