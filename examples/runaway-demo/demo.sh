#!/usr/bin/env bash
# Reproduce the canonical "background agent hits a loop" horror story.
#
# What this does:
#   1. Tightens the budget to $0.01/minute for the demo tenant.
#   2. Hammers the proxy with $0.001-ish chat requests in a loop.
#   3. Watches the breaker flip Closed → Open after a few iterations.
#   4. Every subsequent request returns 429 + a structured deny payload.
#
# Run:
#   chmod +x demo.sh
#   ./demo.sh
#
# Requires:
#   - fusebox-cli built (`cargo build --release`)
#   - the proxy running on localhost:8080
#   - $OPENAI_API_KEY exported
set -euo pipefail

TENANT="runaway-demo"
URL="${FUSEBOX_URL:-http://localhost:8080}"
MODEL="${MODEL:-gpt-4o-mini}"

step() {
  printf '\n\033[1;33m▶ %s\033[0m\n' "$*"
}

step "Pinning a $0.01/minute budget on tenant=$TENANT"
fusebox budget set --tenant "$TENANT" --limit '0.01/minute'
echo "  (restart the proxy if it was already running)"

step "Spamming chat completions until Fusebox trips"
for i in $(seq 1 50); do
  status=$(curl -s -o /tmp/fb-resp.json -w '%{http_code}' \
    -X POST "$URL/v1/chat/completions" \
    -H "content-type: application/json" \
    -H "authorization: Bearer ${OPENAI_API_KEY}" \
    -H "x-fusebox-tenant: $TENANT" \
    --data "{\"model\":\"$MODEL\",\"messages\":[{\"role\":\"user\",\"content\":\"hi\"}],\"max_tokens\":16}")
  printf '  request %02d → HTTP %s\n' "$i" "$status"
  if [[ "$status" == "429" ]]; then
    echo "  ↳ tripped! response body:"
    sed 's/^/      /' /tmp/fb-resp.json
    break
  fi
done

step "Breaker state after the run"
fusebox breaker status --tenant "$TENANT" --url "$URL"

step "Manual reset (simulates an admin clicking 'Close' in the dashboard)"
fusebox breaker reset --tenant "$TENANT" --url "$URL"

step "All clean again"
fusebox breaker status --tenant "$TENANT" --url "$URL"
