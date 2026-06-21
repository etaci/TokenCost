#!/bin/bash
# Integration test script for Fusebox

set -e

echo "🧪 Fusebox Integration Tests"
echo "============================"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
FUSEBOX_PORT=8080
FUSEBOX_URL="http://localhost:${FUSEBOX_PORT}"
TEST_TENANT="integration-test"

# Cleanup function
cleanup() {
    echo -e "\n${YELLOW}Cleaning up...${NC}"
    if [ ! -z "$FUSEBOX_PID" ]; then
        kill $FUSEBOX_PID 2>/dev/null || true
    fi
    rm -rf .fusebox/test-*.db
}

trap cleanup EXIT

# Start Fusebox in background
echo -e "${YELLOW}Starting Fusebox...${NC}"
cargo build --release --bin fusebox-proxy
FUSEBOX_DATA_DIR=.fusebox/test cargo run --release --bin fusebox-proxy &
FUSEBOX_PID=$!

# Wait for server to be ready
echo "Waiting for server to start..."
for i in {1..30}; do
    if curl -s "${FUSEBOX_URL}/health" > /dev/null 2>&1; then
        echo -e "${GREEN}✓ Server started${NC}"
        break
    fi
    if [ $i -eq 30 ]; then
        echo -e "${RED}✗ Server failed to start${NC}"
        exit 1
    fi
    sleep 1
done

# Test counter
TESTS_PASSED=0
TESTS_FAILED=0

# Helper function to run a test
run_test() {
    local test_name="$1"
    local test_command="$2"

    echo -e "\n${YELLOW}Testing: ${test_name}${NC}"

    if eval "$test_command"; then
        echo -e "${GREEN}✓ PASSED${NC}"
        ((TESTS_PASSED++))
        return 0
    else
        echo -e "${RED}✗ FAILED${NC}"
        ((TESTS_FAILED++))
        return 1
    fi
}

# Test 1: Health check
run_test "Health endpoint" \
    "curl -sf '${FUSEBOX_URL}/health' | grep -q 'ok'"

# Test 2: Breaker state endpoint
run_test "Breaker state endpoint" \
    "curl -sf '${FUSEBOX_URL}/v1/breaker/state?tenant=${TEST_TENANT}' | grep -q 'closed'"

# Test 3: Spend endpoint
run_test "Spend endpoint" \
    "curl -sf '${FUSEBOX_URL}/v1/spend?tenant=${TEST_TENANT}' | grep -q 'used_usd'"

# Test 4: Metrics endpoint
run_test "Prometheus metrics" \
    "curl -sf '${FUSEBOX_URL}/metrics' | grep -q 'fusebox_'"

# Test 5: OpenAI proxy request (dry run without real API key)
run_test "OpenAI API structure" \
    "curl -sf -X POST '${FUSEBOX_URL}/v1/chat/completions' \
        -H 'Content-Type: application/json' \
        -H 'Authorization: Bearer fake-key' \
        -H 'X-Fusebox-Tenant: ${TEST_TENANT}' \
        -d '{\"model\":\"gpt-4o-mini\",\"messages\":[{\"role\":\"user\",\"content\":\"test\"}]}' \
        2>&1 | grep -qE '(error|upstream)'"

# Test 6: Anthropic proxy request (dry run)
run_test "Anthropic API structure" \
    "curl -sf -X POST '${FUSEBOX_URL}/v1/messages' \
        -H 'Content-Type: application/json' \
        -H 'x-api-key: fake-key' \
        -H 'anthropic-version: 2023-06-01' \
        -H 'X-Fusebox-Tenant: ${TEST_TENANT}' \
        -d '{\"model\":\"claude-sonnet-4-6\",\"max_tokens\":100,\"messages\":[{\"role\":\"user\",\"content\":\"test\"}]}' \
        2>&1 | grep -qE '(error|upstream)'"

# Test 7: Budget request creation
run_test "Budget request creation" \
    "curl -sf -X POST '${FUSEBOX_URL}/v1/budget/requests' \
        -H 'Content-Type: application/json' \
        -d '{\"tenant\":\"${TEST_TENANT}\",\"limit_usd\":100.0,\"window\":\"day\",\"reason\":\"test\"}' \
        | grep -q 'pending'"

# Test 8: Budget request listing
run_test "Budget request listing" \
    "curl -sf '${FUSEBOX_URL}/v1/budget/requests?tenant=${TEST_TENANT}' \
        | grep -q 'requests'"

# Test 9: Events endpoint
run_test "Events endpoint" \
    "curl -sf '${FUSEBOX_URL}/v1/events?tenant=${TEST_TENANT}&limit=10' \
        | grep -q 'events'"

# Test 10: Invalid tenant handling
run_test "Invalid tenant handling" \
    "curl -sf '${FUSEBOX_URL}/v1/spend?tenant=invalid%00tenant' \
        2>&1 | grep -qE '(error|invalid)'"

# Summary
echo -e "\n============================"
echo -e "${YELLOW}Test Results:${NC}"
echo -e "${GREEN}Passed: ${TESTS_PASSED}${NC}"
echo -e "${RED}Failed: ${TESTS_FAILED}${NC}"
echo -e "============================"

if [ $TESTS_FAILED -eq 0 ]; then
    echo -e "\n${GREEN}✓ All integration tests passed!${NC}"
    exit 0
else
    echo -e "\n${RED}✗ Some tests failed!${NC}"
    exit 1
fi
