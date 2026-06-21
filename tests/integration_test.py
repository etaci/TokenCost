#!/usr/bin/env python3
"""
Integration tests for Fusebox using Python
Tests real API interactions with the proxy
"""

import os
import sys
import time
import subprocess
import requests
from typing import Optional

# Configuration
FUSEBOX_URL = os.getenv("FUSEBOX_URL", "http://localhost:8080")
TEST_TENANT = "python-integration-test"

class Colors:
    GREEN = '\033[0;32m'
    RED = '\033[0;31m'
    YELLOW = '\033[1;33m'
    NC = '\033[0m'

class TestRunner:
    def __init__(self):
        self.passed = 0
        self.failed = 0
        self.fusebox_process: Optional[subprocess.Popen] = None

    def start_fusebox(self):
        """Start Fusebox in background"""
        print(f"{Colors.YELLOW}Starting Fusebox...{Colors.NC}")

        # Build first
        subprocess.run(["cargo", "build", "--release", "--bin", "fusebox-proxy"], check=True)

        # Start server
        self.fusebox_process = subprocess.Popen(
            ["cargo", "run", "--release", "--bin", "fusebox-proxy"],
            env={**os.environ, "FUSEBOX_DATA_DIR": ".fusebox/test-python"},
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL
        )

        # Wait for server to be ready
        for i in range(30):
            try:
                response = requests.get(f"{FUSEBOX_URL}/health", timeout=1)
                if response.status_code == 200:
                    print(f"{Colors.GREEN}✓ Server started{Colors.NC}")
                    return
            except requests.RequestException:
                time.sleep(1)

        raise Exception("Server failed to start")

    def stop_fusebox(self):
        """Stop Fusebox"""
        if self.fusebox_process:
            self.fusebox_process.terminate()
            self.fusebox_process.wait(timeout=5)

    def run_test(self, name: str, test_func):
        """Run a single test"""
        print(f"\n{Colors.YELLOW}Testing: {name}{Colors.NC}")
        try:
            test_func()
            print(f"{Colors.GREEN}✓ PASSED{Colors.NC}")
            self.passed += 1
        except AssertionError as e:
            print(f"{Colors.RED}✗ FAILED: {e}{Colors.NC}")
            self.failed += 1
        except Exception as e:
            print(f"{Colors.RED}✗ ERROR: {e}{Colors.NC}")
            self.failed += 1

    def test_health_check(self):
        """Test health endpoint"""
        response = requests.get(f"{FUSEBOX_URL}/health")
        assert response.status_code == 200
        assert response.text == "ok"

    def test_breaker_state(self):
        """Test breaker state endpoint"""
        response = requests.get(
            f"{FUSEBOX_URL}/v1/breaker/state",
            params={"tenant": TEST_TENANT}
        )
        assert response.status_code == 200
        data = response.json()
        assert "state" in data
        assert data["state"] in ["closed", "open", "half_open"]

    def test_spend_endpoint(self):
        """Test spend endpoint"""
        response = requests.get(
            f"{FUSEBOX_URL}/v1/spend",
            params={"tenant": TEST_TENANT, "window": "1d"}
        )
        assert response.status_code == 200
        data = response.json()
        assert "used_usd" in data
        assert "limit_usd" in data

    def test_metrics_endpoint(self):
        """Test Prometheus metrics"""
        response = requests.get(f"{FUSEBOX_URL}/metrics")
        assert response.status_code == 200
        assert "fusebox_" in response.text

    def test_budget_request_creation(self):
        """Test creating a budget request"""
        response = requests.post(
            f"{FUSEBOX_URL}/v1/budget/requests",
            json={
                "tenant": TEST_TENANT,
                "limit_usd": 100.0,
                "window": "day",
                "reason": "Integration test"
            }
        )
        assert response.status_code == 200
        data = response.json()
        assert data["status"] == "pending"

    def test_budget_request_listing(self):
        """Test listing budget requests"""
        response = requests.get(
            f"{FUSEBOX_URL}/v1/budget/requests",
            params={"tenant": TEST_TENANT}
        )
        assert response.status_code == 200
        data = response.json()
        assert "requests" in data

    def test_events_endpoint(self):
        """Test events endpoint"""
        response = requests.get(
            f"{FUSEBOX_URL}/v1/events",
            params={"tenant": TEST_TENANT, "limit": 10}
        )
        assert response.status_code == 200
        data = response.json()
        assert "events" in data

    def test_openai_proxy_structure(self):
        """Test OpenAI API proxy structure (without real API key)"""
        response = requests.post(
            f"{FUSEBOX_URL}/v1/chat/completions",
            headers={
                "Authorization": "Bearer fake-key-for-testing",
                "X-Fusebox-Tenant": TEST_TENANT,
            },
            json={
                "model": "gpt-4o-mini",
                "messages": [{"role": "user", "content": "test"}]
            }
        )
        # Should fail with upstream error (no real API key)
        # but structure should be valid
        assert response.status_code in [401, 500]

    def test_anthropic_proxy_structure(self):
        """Test Anthropic API proxy structure (without real API key)"""
        response = requests.post(
            f"{FUSEBOX_URL}/v1/messages",
            headers={
                "x-api-key": "fake-key-for-testing",
                "anthropic-version": "2023-06-01",
                "X-Fusebox-Tenant": TEST_TENANT,
            },
            json={
                "model": "claude-sonnet-4-6",
                "max_tokens": 100,
                "messages": [{"role": "user", "content": "test"}]
            }
        )
        # Should fail with upstream error (no real API key)
        assert response.status_code in [401, 500]

    def run_all_tests(self):
        """Run all integration tests"""
        print(f"\n{Colors.YELLOW}🧪 Fusebox Python Integration Tests{Colors.NC}")
        print("=" * 40)

        try:
            self.start_fusebox()

            # Run all tests
            self.run_test("Health check", self.test_health_check)
            self.run_test("Breaker state", self.test_breaker_state)
            self.run_test("Spend endpoint", self.test_spend_endpoint)
            self.run_test("Metrics endpoint", self.test_metrics_endpoint)
            self.run_test("Budget request creation", self.test_budget_request_creation)
            self.run_test("Budget request listing", self.test_budget_request_listing)
            self.run_test("Events endpoint", self.test_events_endpoint)
            self.run_test("OpenAI proxy structure", self.test_openai_proxy_structure)
            self.run_test("Anthropic proxy structure", self.test_anthropic_proxy_structure)

        finally:
            self.stop_fusebox()

        # Summary
        print("\n" + "=" * 40)
        print(f"{Colors.YELLOW}Test Results:{Colors.NC}")
        print(f"{Colors.GREEN}Passed: {self.passed}{Colors.NC}")
        print(f"{Colors.RED}Failed: {self.failed}{Colors.NC}")
        print("=" * 40)

        if self.failed == 0:
            print(f"\n{Colors.GREEN}✓ All integration tests passed!{Colors.NC}")
            return 0
        else:
            print(f"\n{Colors.RED}✗ Some tests failed!{Colors.NC}")
            return 1

if __name__ == "__main__":
    runner = TestRunner()
    sys.exit(runner.run_all_tests())
