#!/usr/bin/env bash

# SWE Reviewer Test Runner
# Provides multiple ways to run and test the SWE Reviewer system

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo -e "${BLUE}🧪 SWE Reviewer Test Suite${NC}"
echo -e "${BLUE}═══════════════════════════${NC}"

# Function to show help
show_help() {
    echo "Usage: $0 <command> [options]"
    echo ""
    echo "Commands:"
    echo "  quick          Run quick integration tests (recommended for development)"
    echo "  full           Run full E2E test suite with all Google Drive links"
    echo "  single <id>    Run a single test case by ID (1-15)"
    echo "  validate       Run validation-only tests (no downloads)"
    echo "  no-violations  Run only tests expecting no violations"
    echo "  violations     Run only tests expecting violations"
    echo "  benchmark      Run performance benchmarks" 
    echo "  unit           Run unit tests"
    echo "  check          Just check that the code compiles"
    echo "  help           Show this help message"
    echo ""
    echo "Options:"
    echo "  --verbose      Show detailed output"
    echo "  --release      Use release build (faster execution)"
    echo "  --timeout <s>  Set timeout in seconds (default: 300)"
    echo ""
    echo "Examples:"
    echo "  $0 quick              # Fast integration tests"
    echo "  $0 full --release     # Complete test suite with optimizations"
    echo "  $0 single 1           # Test just Google Drive link #1"
    echo "  $0 validate --verbose # Validation tests with detailed output"
    echo "  $0 benchmark          # Performance testing"
    echo ""
    echo "Test Cases (for 'single' and 'full' commands):"
    echo "  1:  F2P missing in after - https://drive.google.com/drive/folders/1LAbDGCOkgTUKDGy9i2pgnhUlT07ews_9"
    echo "  2:  F2P missing in after - https://drive.google.com/drive/folders/1rpBzsSwp4fow2xuw6q6qYk-v_a5Uv1EZ"
    echo "  3:  No violations - https://drive.google.com/drive/folders/1rq33SVzJCs9HZHS0mqGdtYO-W_ntWsFB"
    echo "  4:  No violations - https://drive.google.com/drive/folders/1N6nLBCW6CPE-BxRLUKeRREi0T3mQtEia"
    echo "  5:  No violations - https://drive.google.com/drive/folders/1U5SYc5wfMU9GMWyDdiQpWBmM7cu1-1TK"
    echo "  6:  No violations - https://drive.google.com/drive/folders/1AFP1OzZmpA-S56I4AS37YqBaNhE8cA_E"
    echo "  7:  No violations - https://drive.google.com/drive/folders/1MA_5ZhRFiOBd24z2OruKC05pBQr5ZeGB"
    echo "  8:  No violations - https://drive.google.com/drive/folders/1NpabUZ6Uv4ZY5Stjesi7EWgAHNfslUr_"
    echo "  9:  F2P in src diff - https://drive.google.com/drive/folders/1dDjkXNPWg81VBcEGoBz2N3wv0JPjVupo"
    echo "  10: No violations - https://drive.google.com/drive/folders/1tWW536Zwx2dIEYfovvkP92rnz_S3F4Wt"
    echo "  11: F2P passing in before - https://drive.google.com/drive/folders/1kFzsfORq7uTTbbdeTXQN7oqBeJAt3Tzg"
    echo "  12: No violations - https://drive.google.com/drive/folders/1hlZZpb-hh6VU461cKTZnIaM1gr353m3h"
    echo "  13: P2P violations - https://drive.google.com/drive/folders/14j3jPC1BZ0IHm3rsIhZi5HhHP7BoO6jR"
    echo "  14: P2P missing all - https://drive.google.com/drive/folders/1meg12kGotjuGLIRQJW2siN8j2jB2uyiA"
    echo "  15: No violations - https://drive.google.com/drive/folders/1Wc6SHwQUs_gndnDrVsDFv5-4SZjN14jN"
}

# Function to check environment
check_environment() {
    echo -e "${YELLOW}🔧 Checking environment...${NC}"
    
    # Check if we're in the right directory
    if [[ ! -f "Cargo.toml" ]]; then
        echo -e "${RED}❌ Error: Cannot find Cargo.toml. Please run from src-tauri directory.${NC}"
        exit 1
    fi
    
    # Check for Google credentials
    if [[ -z "${GOOGLE_CLIENT_SECRET:-}" ]] && [[ -z "${SWE_REVIEWER_GOOGLE_CLIENT_SECRET:-}" ]]; then
        echo -e "${YELLOW}⚠️ Warning: No Google OAuth credentials found in environment${NC}"
        echo -e "${YELLOW}   Some tests may fail if credentials are not saved in app settings${NC}"
    else
        echo -e "${GREEN}✅ Google OAuth credentials found${NC}"
    fi
    
    # Check for OpenAI API key (optional)
    if [[ -n "${OPENAI_API_KEY:-}" ]]; then
        echo -e "${GREEN}✅ OpenAI API key found${NC}"
    else
        echo -e "${YELLOW}⚠️ OPENAI_API_KEY not set - AI analysis may be limited${NC}"
    fi
    
    # Create output directories
    mkdir -p test_logs test_reports test_artifacts
    echo -e "${GREEN}✅ Environment ready${NC}"
}

# Parse command line arguments
COMMAND=${1:-help}
VERBOSE=false
RELEASE_FLAG=""
TIMEOUT=300

# Parse options
shift 2>/dev/null || true
while [[ $# -gt 0 ]]; do
    case $1 in
        --verbose)
            VERBOSE=true
            shift
            ;;
        --release)
            RELEASE_FLAG="--release"
            shift
            ;;
        --timeout)
            TIMEOUT="$2"
            shift 2
            ;;
        *)
            echo -e "${YELLOW}Unknown option: $1${NC}"
            shift
            ;;
    esac
done

# Set verbosity
if [[ "$VERBOSE" == "true" ]]; then
    export RUST_LOG="debug"
    export RUST_BACKTRACE="1"
else
    export RUST_LOG="info"
fi

# Main command processing
case $COMMAND in
    help|--help|-h)
        show_help
        exit 0
        ;;
        
    check)
        echo -e "${YELLOW}🔨 Checking compilation...${NC}"
        check_environment
        if cargo check $RELEASE_FLAG; then
            echo -e "${GREEN}✅ Code compiles successfully${NC}"
        else
            echo -e "${RED}❌ Compilation errors found${NC}"
            exit 1
        fi
        ;;
        
    unit)
        echo -e "${YELLOW}🧪 Running unit tests...${NC}"
        check_environment
        echo "cargo test --lib $RELEASE_FLAG --timeout $TIMEOUT"
        if cargo test --lib $RELEASE_FLAG --timeout $TIMEOUT; then
            echo -e "${GREEN}✅ Unit tests passed${NC}"
        else
            echo -e "${RED}❌ Unit tests failed${NC}"
            exit 1
        fi
        ;;
        
    quick)
        echo -e "${YELLOW}🚀 Running quick integration tests...${NC}"
        check_environment
        if cargo test --test integration_tests $RELEASE_FLAG test_complete_flow_no_violations --timeout $TIMEOUT; then
            echo -e "${GREEN}✅ Quick tests passed${NC}"
        else
            echo -e "${RED}❌ Quick tests failed${NC}"
            exit 1
        fi
        ;;
        
    validate)
        echo -e "${YELLOW}🔍 Running validation tests...${NC}"
        check_environment
        if cargo test --test integration_tests $RELEASE_FLAG test_validation --timeout $TIMEOUT; then
            echo -e "${GREEN}✅ Validation tests passed${NC}"
        else
            echo -e "${RED}❌ Validation tests failed${NC}"
            exit 1
        fi
        ;;
        
    violations)
        echo -e "${YELLOW}⚠️ Running violation detection tests...${NC}"
        check_environment
        if cargo test --test integration_tests $RELEASE_FLAG --ignored --timeout $TIMEOUT; then
            echo -e "${GREEN}✅ Violation tests passed${NC}"
        else
            echo -e "${RED}❌ Violation tests failed${NC}"
            exit 1
        fi
        ;;
        
    no-violations)
        echo -e "${YELLOW}✅ Running no-violation tests...${NC}"
        check_environment
        if cargo test --test e2e_tests $RELEASE_FLAG test_no_violations_cases --timeout $TIMEOUT; then
            echo -e "${GREEN}✅ No-violation tests passed${NC}"
        else
            echo -e "${RED}❌ No-violation tests failed${NC}"
            exit 1
        fi
        ;;
        
    benchmark)
        echo -e "${YELLOW}📊 Running benchmarks...${NC}"
        check_environment
        if cargo test --test integration_tests $RELEASE_FLAG benchmark_validation_performance --ignored --timeout $TIMEOUT; then
            echo -e "${GREEN}✅ Benchmarks completed${NC}"
        else
            echo -e "${RED}❌ Benchmarks failed${NC}"
            exit 1
        fi
        ;;
        
    single)
        TEST_ID=$2
        if [[ -z "$TEST_ID" ]] || ! [[ "$TEST_ID" =~ ^[1-9]$|^1[0-5]$ ]]; then
            echo -e "${RED}❌ Error: Please provide a valid test ID (1-15)${NC}"
            echo "Usage: $0 single <id>"
            exit 1
        fi
        echo -e "${YELLOW}🎯 Running single test case #$TEST_ID...${NC}"
        check_environment
        if cargo run --bin e2e_runner $RELEASE_FLAG -- $TEST_ID; then
            echo -e "${GREEN}✅ Test case #$TEST_ID passed${NC}"
        else
            echo -e "${RED}❌ Test case #$TEST_ID failed${NC}"
            exit 1
        fi
        ;;
        
    full)
        echo -e "${YELLOW}🌟 Running full E2E test suite...${NC}"
        echo -e "${YELLOW}⏰ This may take 10-15 minutes...${NC}"
        check_environment
        if cargo run --bin e2e_runner $RELEASE_FLAG; then
            echo -e "${GREEN}✅ Full E2E test suite passed${NC}"
        else
            echo -e "${RED}❌ Full E2E test suite failed${NC}"
            exit 1
        fi
        ;;
        
    *)
        echo -e "${RED}❌ Unknown command: $COMMAND${NC}"
        echo ""
        show_help
        exit 1
        ;;
esac

echo ""
echo -e "${BLUE}📁 Check output in:${NC}"
echo "   test_reports/     - Test results and HTML reports"
echo "   test_logs/        - Individual test execution logs"  
echo "   test_artifacts/   - Downloaded test artifacts"
echo ""
echo -e "${GREEN}🎉 Test execution completed!${NC}"
