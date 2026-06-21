# Fusebox MCP Server - Example Usage

This directory contains examples of how AI agents use the Fusebox MCP server to monitor their own costs.

## Scenario 1: Claude Checking Budget Before Large Task

**User:** Can you analyze this 500-page research paper?

**Claude's internal process:**

```typescript
// 1. Check current budget status
const budget = await use_mcp_tool("get_budget", {});
// Returns: { limit_usd: 50, window: "day", used: 15.20, remaining: 34.80 }

// 2. Check recent spending rate
const spend = await use_mcp_tool("get_spend", { window: "1h" });
// Returns: { cost_usd: 2.50, input_tokens: 100000, output_tokens: 5000 }

// 3. Estimate cost for this task (rough)
const estimatedCost = 500 * 0.02; // ~10 USD for 500 pages

// 4. Make decision
if (estimatedCost < budget.remaining) {
  // Proceed with task
} else {
  // Request budget increase or decline task
}
```

**Claude's response:**

> I can analyze this paper. I've checked my budget and have $34.80 remaining 
> of my $50 daily limit. This task will cost approximately $10, so I have 
> sufficient budget. Let me proceed...

---

## Scenario 2: Agent Hitting Budget Limit

**User:** Process another 1000 documents

**Claude's internal process:**

```typescript
// 1. Check breaker state
const breaker = await use_mcp_tool("get_breaker", {});
// Returns: { state: "open", reason: "budget exceeded (1d)", opened_at: "..." }

// 2. Agent realizes it's blocked
```

**Claude's response:**

> I'm currently unable to process this request because I've hit my daily 
> budget limit. My circuit breaker is open due to budget exhaustion. 
> 
> Current status:
> - Spent: $50.00 / $50.00 (100%)
> - Breaker: OPEN
> - Will reset: in 8 hours
>
> Would you like me to:
> 1. Wait until the budget resets tomorrow
> 2. Request a budget increase from an admin

---

## Scenario 3: Agent Proactively Requesting Budget Increase

**User:** I need you to run a large batch job tonight

**Claude's internal process:**

```typescript
// 1. Estimate batch job cost
const estimatedCost = 150; // $150 USD

// 2. Check current budget
const budget = await use_mcp_tool("get_budget", {});
// Returns: { limit_usd: 50, window: "day", ... }

// 3. Request increase proactively
const request = await use_mcp_tool("request_budget_increase", {
  limit_usd: 200,
  window: "day",
  reason: "Large batch processing job scheduled for tonight (estimated $150)",
  ttl_seconds: 86400, // 24 hours
});
// Returns: { request_id: "abc123", status: "pending" }
```

**Claude's response:**

> I've analyzed the batch job requirements and estimate it will cost ~$150.
> My current daily budget is $50, which is insufficient.
>
> I've submitted a budget increase request:
> - Requested limit: $200/day
> - Reason: Large batch processing job
> - Request ID: abc123
> - Status: Pending admin approval
>
> Once an admin approves this request, I can proceed with the batch job.
> Would you like me to notify you when the approval comes through?

---

## Scenario 4: Self-Aware Cost Optimization

**User:** Summarize these 10 articles

**Claude's internal process:**

```typescript
// 1. Check budget headroom
const spend = await use_mcp_tool("get_spend", { window: "1d" });
// Returns: { cost_usd: 45.00, ... }

const budget = await use_mcp_tool("get_budget", {});
// Returns: { limit_usd: 50, ... }

const remaining = budget.limit_usd - spend.cost_usd; // $5

// 2. Choose model based on budget
if (remaining < 10) {
  // Use cheaper model: gpt-4o-mini instead of gpt-4o
  useModel = "gpt-4o-mini";
} else {
  useModel = "gpt-4o";
}
```

**Claude's response:**

> I'll summarize these articles for you. I'm currently at 90% of my daily 
> budget ($45/$50 used), so I'm using a cost-efficient model (gpt-4o-mini) 
> to ensure I stay within budget while still delivering quality results.
>
> [Summaries...]
>
> Cost for this task: $0.25 (budget remaining: $4.75)

---

## Scenario 5: Cross-Window Budget Awareness

**User:** Can you help me with a quick question?

**Claude's internal process:**

```typescript
// Check multiple windows
const hour = await use_mcp_tool("get_spend", { window: "1h" });
const day = await use_mcp_tool("get_spend", { window: "1d" });

// Agent sees spike in last hour
// hour: $15, day: $20 → $15 in the last hour is unusual

// Check breaker
const breaker = await use_mcp_tool("get_breaker", {});
// Returns: { state: "half_open", ... }
```

**Claude's response:**

> Of course! I notice I had unusually high usage in the past hour ($15) 
> compared to my typical pattern. My cost monitoring system put me in 
> "half-open" mode to prevent runaway spending. Everything is working 
> normally now.
>
> [Answers question...]

---

## Implementation Notes

All these examples work automatically when:

1. Fusebox proxy is running with the MCP server
2. Claude Desktop (or other MCP client) is configured
3. The AI agent has access to the MCP tools

The agent makes these decisions **autonomously** without explicit user 
instruction - it's truly self-aware about costs!

---

## Try It Yourself

1. Start Fusebox:
```bash
cargo run --bin fusebox -- start
```

2. Build and start MCP server:
```bash
cd packages/mcp-server
npm install
npm run build
npm start
```

3. Configure Claude Desktop with the MCP server (see README)

4. Chat with Claude and watch it proactively manage its budget!
