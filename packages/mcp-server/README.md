# @fusebox/mcp-server

**MCP Server for Fusebox** - Allows AI agents to monitor their own LLM costs and request budget increases via the [Model Context Protocol](https://modelcontextprotocol.io/).

## What is this?

This is an MCP server that exposes Fusebox's monitoring and budget management capabilities to AI agents. When connected to Claude Desktop, Cursor, or any MCP-compatible client, AI agents can:

- 📊 **Check their current budget** and spending limits
- 💰 **View real-time spending** across different time windows
- 🔴 **Monitor circuit breaker status** to understand if they're rate-limited
- 📝 **Request budget increases** when they need more capacity

This enables true **agent self-awareness** around costs — your AI assistant can proactively manage its own budget instead of running blind.

## Installation

### Via npm (when published)

```bash
npm install -g @fusebox/mcp-server
```

### From source

```bash
cd packages/mcp-server
npm install
npm run build
```

## Usage

### With Claude Desktop

Add to your `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "fusebox": {
      "command": "npx",
      "args": ["-y", "@fusebox/mcp-server"],
      "env": {
        "FUSEBOX_URL": "http://localhost:8080",
        "FUSEBOX_TENANT": "my-tenant"
      }
    }
  }
}
```

### With Cursor

Add to your Cursor MCP configuration:

```json
{
  "fusebox": {
    "command": "npx",
    "args": ["-y", "@fusebox/mcp-server"],
    "env": {
      "FUSEBOX_URL": "http://localhost:8080",
      "FUSEBOX_TENANT": "cursor-user"
    }
  }
}
```

### Standalone

```bash
# Set environment variables
export FUSEBOX_URL=http://localhost:8080
export FUSEBOX_TENANT=my-app

# Run the server
npx @fusebox/mcp-server
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `FUSEBOX_URL` | Fusebox proxy base URL | `http://localhost:8080` |
| `FUSEBOX_TENANT` | Default tenant ID | `default` |

## Available Tools

### `get_budget`

Check the current budget configuration for a tenant.

**Arguments:**
- `tenant` (optional): Tenant ID

**Example:**
```
Claude: Let me check my current budget
→ Uses: get_budget
← Returns: { limit_usd: 50, window: "day", used: 12.34 }
```

### `get_spend`

View current spending in a specific time window.

**Arguments:**
- `tenant` (optional): Tenant ID
- `window` (default: `1d`): Time window - one of `1m`, `1h`, `1d`, `1w`, `1mo`

**Example:**
```
Claude: How much have I spent in the last hour?
→ Uses: get_spend { window: "1h" }
← Returns: { cost_usd: 2.45, input_tokens: 150000, output_tokens: 8000 }
```

### `get_breaker`

Check the circuit breaker state.

**Arguments:**
- `tenant` (optional): Tenant ID

**Example:**
```
Claude: Am I currently rate-limited?
→ Uses: get_breaker
← Returns: { state: "closed", last_trip: null }
```

### `request_budget_increase`

Request a budget increase (creates a pending request for admin approval).

**Arguments:**
- `tenant` (optional): Tenant ID
- `limit_usd` (required): Requested budget limit in USD
- `window` (required): Budget window - one of `minute`, `hour`, `day`, `week`, `month`
- `reason` (optional): Reason for the increase
- `ttl_seconds` (optional): TTL if approved

**Example:**
```
Claude: I need a higher budget - let me request an increase
→ Uses: request_budget_increase { 
    limit_usd: 100, 
    window: "day", 
    reason: "Large batch processing job" 
  }
← Returns: { request_id: "...", status: "pending" }
```

## Example Interaction

Here's how an AI agent might use these tools:

```
User: Can you process this 100-page document for me?

Claude: Let me first check if I have enough budget for this task.

[Uses get_spend to see current usage]
[Uses get_budget to see remaining budget]

Claude: I see I've used $12 of my $50 daily budget. This task will cost 
approximately $8. I have enough budget, proceeding...

[Processes document]

Claude: Task complete! I've now used $20 of my $50 daily budget.
```

Or, when hitting limits:

```
User: Process another large batch

Claude: I'm currently approaching my daily budget limit ($48/$50 used).
Let me request a budget increase.

[Uses request_budget_increase]

Claude: I've submitted a request to increase my daily budget to $100.
Once approved by an admin, I can proceed with the batch.
```

## How It Works

The MCP server communicates with your Fusebox proxy over HTTP:

```
┌──────────┐         ┌─────────────┐         ┌──────────┐
│  Claude  │  MCP    │  MCP Server │  HTTP   │ Fusebox  │
│ Desktop  │◄───────▶│  (this pkg) │◄───────▶│  Proxy   │
└──────────┘         └─────────────┘         └──────────┘
```

1. Claude Desktop calls MCP tools
2. MCP server translates to Fusebox API calls
3. Fusebox proxy responds with current state
4. Results are returned to Claude

## Development

```bash
# Install dependencies
npm install

# Build
npm run build

# Watch mode
npm run dev

# Run locally
npm start
```

## Troubleshooting

### "Connection refused" error

Make sure Fusebox proxy is running:

```bash
cargo run --bin fusebox -- start
```

### Tools not showing up in Claude Desktop

1. Check your `claude_desktop_config.json` syntax
2. Restart Claude Desktop after config changes
3. Check the MCP logs: `~/Library/Logs/Claude/mcp*.log`

### Budget requests not working

Ensure you have the latest Fusebox version with budget request persistence:

```bash
cd fusebox
git pull
cargo build --release
```

## Related

- [Fusebox main repo](https://github.com/fusebox-dev/fusebox)
- [Model Context Protocol spec](https://spec.modelcontextprotocol.io/)
- [MCP SDK for TypeScript](https://github.com/modelcontextprotocol/typescript-sdk)

## License

Apache 2.0 - see [LICENSE](../../LICENSE) in the repo root.
