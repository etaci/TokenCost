# @fusebox/sdk

Official TypeScript/JavaScript SDK for Fusebox.

## Installation

```bash
npm install @fusebox/sdk
# or
yarn add @fusebox/sdk
# or
pnpm add @fusebox/sdk
```

## Quick Start

```typescript
import { FuseboxClient } from '@fusebox/sdk'

const fusebox = new FuseboxClient({
  baseURL: 'http://localhost:8080',
  tenant: 'my-app',
})

// Check breaker state
const state = await fusebox.breaker.getState()
console.log(state.state) // 'closed' | 'open' | 'half_open'

// Get spending
const spend = await fusebox.spend.get({ window: '1d' })
console.log(`Used: $${spend.used_usd} / $${spend.limit_usd}`)

// Request budget increase
const request = await fusebox.budget.requestIncrease({
  limit_usd: 100,
  window: 'day',
  reason: 'Scaling up for launch',
})
console.log(`Request ID: ${request.request_id}`)
```

## OpenAI Integration

Drop-in replacement for OpenAI SDK with automatic Fusebox protection:

```typescript
import { FuseboxOpenAI } from '@fusebox/sdk/openai'

const openai = new FuseboxOpenAI({
  apiKey: process.env.OPENAI_API_KEY,
  fusebox: {
    baseURL: 'http://localhost:8080',
    tenant: 'my-app',
  },
})

// Use exactly like OpenAI SDK
const completion = await openai.chat.completions.create({
  model: 'gpt-4o-mini',
  messages: [{ role: 'user', content: 'Hello!' }],
})
```

## Anthropic Integration

```typescript
import { FuseboxAnthropic } from '@fusebox/sdk/anthropic'

const anthropic = new FuseboxAnthropic({
  apiKey: process.env.ANTHROPIC_API_KEY,
  fusebox: {
    baseURL: 'http://localhost:8080',
    tenant: 'my-app',
  },
})

const message = await anthropic.messages.create({
  model: 'claude-sonnet-4-6',
  max_tokens: 1024,
  messages: [{ role: 'user', content: 'Hello!' }],
})
```

## API Reference

### FuseboxClient

Main client for interacting with Fusebox API.

```typescript
const fusebox = new FuseboxClient({
  baseURL: 'http://localhost:8080',
  tenant: 'my-app',
  timeout: 10000, // optional, default 10s
})
```

#### Breaker API

```typescript
// Get breaker state
await fusebox.breaker.getState()

// Reset breaker
await fusebox.breaker.reset()
```

#### Spend API

```typescript
// Get spending for a time window
await fusebox.spend.get({ 
  window: '1m' | '1h' | '1d' | '1w' | '1mo' 
})
```

#### Budget API

```typescript
// Request budget increase
await fusebox.budget.requestIncrease({
  limit_usd: 100,
  window: 'minute' | 'hour' | 'day' | 'week' | 'month',
  reason: 'Scaling up',
  ttl_seconds: 3600, // optional
})

// List budget requests
await fusebox.budget.listRequests({ status: 'pending' })

// Approve budget request (admin)
await fusebox.budget.approveRequest('request-id')

// Reject budget request (admin)
await fusebox.budget.rejectRequest('request-id', 'Reason')
```

#### Events API

```typescript
// Get events
await fusebox.events.list({ 
  limit: 100,
  offset: 0,
})

// Stream events (Server-Sent Events)
const stream = fusebox.events.stream()
for await (const event of stream) {
  console.log(event)
}
```

## TypeScript Support

Full TypeScript support with type definitions included.

```typescript
import type { 
  BreakerState, 
  SpendResponse, 
  BudgetRequest,
  Event 
} from '@fusebox/sdk'
```

## Error Handling

```typescript
import { FuseboxError, BudgetExceededError } from '@fusebox/sdk'

try {
  await openai.chat.completions.create({ ... })
} catch (error) {
  if (error instanceof BudgetExceededError) {
    console.log('Budget exceeded, request blocked')
  } else if (error instanceof FuseboxError) {
    console.log('Fusebox error:', error.message)
  } else {
    console.log('Other error:', error)
  }
}
```

## React Hooks

React hooks for easy integration:

```typescript
import { useBreakerState, useSpend } from '@fusebox/sdk/react'

function Dashboard() {
  const { data: breaker } = useBreakerState('my-app')
  const { data: spend } = useSpend('my-app', '1d')

  return (
    <div>
      <p>Breaker: {breaker?.state}</p>
      <p>Spend: ${spend?.used_usd}</p>
    </div>
  )
}
```

## Examples

See the [examples](./examples) directory for more usage examples.

## License

Apache 2.0
