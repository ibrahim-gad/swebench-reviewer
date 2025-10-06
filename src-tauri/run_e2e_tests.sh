#!/usr/bin/env bash

# E2E Test Runner for SWE Reviewer
# Usage: ./run_e2e_tests.sh [options] [test_ids...]
#
# Options:
#   --no-violations     Run only tests expecting no violations
#   --f2p-violations    Run only tests expecting F2P violations  
#   --p2p-violations    Run only tests expecting P2P violations
#   --src-diff          Run only tests expecting src diff violations
#   --multiple          Run only tests expecting multiple violations
#   --fail-fast         Stop on first failure
#   --sequential        Run tests sequentially (default)
#   --help              Show this help message
#
# Examples:
#   ./run_e2e_tests.sh                    # Run all tests
#   ./run_e2e_tests.sh --no-violations    # Run only no-violation tests
#   ./run_e2e_tests.sh --fail-fast        # Stop on first failure
#   ./run_e2e_tests.sh 1 3 5              # Run specific test IDs
#   ./run_e2e_tests.sh --f2p-violations   # Run F2P violation tests

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TAURI_DIR="$SCRIPT_DIR"

echo -e "${BLUE}🧪 SWE Reviewer E2E Test Runner${NC}"
echo -e "${BLUE}════════════════════════════════${NC}"

# Check if we're in the right directory
if [[ ! -f "$TAURI_DIR/Cargo.toml" ]]; then
    echo -e "${RED}❌ Error: Cannot find Cargo.toml. Please run from src-tauri directory.${NC}"
    exit 1
fi

# Check environment
echo -e "${YELLOW}🔧 Checking environment...${NC}"

# Check for Google credentials
if [[ -z "${GOOGLE_CLIENT_SECRET:-}" ]] && [[ -z "${SWE_REVIEWER_GOOGLE_CLIENT_SECRET:-}" ]]; then
    echo -e "${YELLOW}⚠️ Warning: No Google OAuth credentials found in environment${NC}"
    echo -e "${YELLOW}   Tests will attempt to use saved credentials from app settings${NC}"
else
    echo -e "${GREEN}✅ Google OAuth credentials found${NC}"
fi

# Check for OpenAI API key (optional)
if [[ -n "${OPENAI_API_KEY:-}" ]]; then
    echo -e "${GREEN}✅ OpenAI API key found - AI analysis enabled${NC}"
else
    echo -e "${YELLOW}⚠️ OPENAI_API_KEY not set - analysis features may be limited${NC}"
fi

# Create output directories
mkdir -p test_logs test_reports test_artifacts

echo -e "${GREEN}✅ Environment check completed${NC}"
echo ""

# Show help if requested
if [[ "$1" == "--help" ]] || [[ "$1" == "-h" ]]; then
    echo "E2E Test Runner for SWE Reviewer"
    echo ""
    echo "Usage: $0 [options] [test_ids...]"
    echo ""
    echo "Options:"
    echo "  --no-violations     Run only tests expecting no violations (3,4,5,6,7,8,10,12,15)"
    echo "  --f2p-violations    Run only tests expecting F2P violations (1,2,11)"
    echo "  --p2p-violations    Run only tests expecting P2P violations (14)"
    echo "  --src-diff          Run only tests expecting src diff violations (9)"
    echo "  --multiple          Run only tests expecting multiple violations (13)"
    echo "  --fail-fast         Stop on first failure"
    echo "  --sequential        Run tests sequentially (default)"
    echo "  --help, -h          Show this help message"
    echo ""
    echo "Test Cases:"
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
    echo ""
    echo "Examples:"
    echo "  $0                        # Run all tests"
    echo "  $0 --no-violations        # Run only no-violation tests"
    echo "  $0 --fail-fast            # Stop on first failure"
    echo "  $0 1 3 5                  # Run specific test IDs"
    exit 0
fi

# Run the tests
echo -e "${YELLOW}🚀 Starting E2E tests...${NC}"
echo "Arguments: $*"
echo ""

# Execute with all arguments passed through
if ! cargo run --bin e2e_runner -- "$@"; then
    echo ""
    echo -e "${RED}❌ Tests failed${NC}"
    echo ""
    echo -e "${BLUE}📁 Check output files:${NC}"
    echo "   test_reports/     - Test results and HTML reports"
    echo "   test_logs/        - Individual test logs"
    echo "   test_artifacts/   - Downloaded test artifacts"
    exit 1
else
    echo ""
    echo -e "${GREEN}✅ All tests passed!${NC}"
    echo ""
    echo -e "${BLUE}📁 Output files generated:${NC}"
    echo "   test_reports/     - Test results and HTML reports" 
    echo "   test_logs/        - Individual test logs"
    echo "   test_artifacts/   - Downloaded test artifacts"
fi
