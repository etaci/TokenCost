# Fusebox TypeScript SDK Examples

## Basic Usage

```typescript
import { FuseboxClient } from '@fusebox/sdk'

const fusebox = new FuseboxClient({
  baseURL: 'http://localhost:8080',
  tenant: 'my-app',
})

// Check breaker state
const state = await fusebox.breaker.getState()
console.log(`Breaker is ${state.state}`)

// Get spending
const spend = await fusebox.spend.get({ window: '1d' })
console.log(`Spent $${spend.used_usd} of $${spend.limit_usd}`)
```

## OpenAI Integration

```typescript
import { FuseboxOpenAI } from '@fusebox/sdk/openai'

const openai = new FuseboxOpenAI({
  apiKey: process.env.OPENAI_API_KEY!,
  fusebox: {
    baseURL: 'http://localhost:8080',
    tenant: 'my-app',
  },
})

async function chat(message: string) {
  try {
    const completion = await openai.chat.completions.create({
      model: 'gpt-4o-mini',
      messages: [{ role: 'user', content: message }],
    })
    
    return completion.choices[0].message.content
  } catch (error) {
    if (error instanceof BudgetExceededError) {
      console.log('Budget exceeded! Request was blocked.')
      return 'Budget exceeded, please try again later.'
    }
    throw error
  }
}
```

## React Integration

```typescript
import { useBreakerState, useSpend } from '@fusebox/sdk/react'

function Dashboard() {
  const { data: breaker, loading: breakerLoading } = useBreakerState('my-app')
  const { data: spend, loading: spendLoading } = useSpend('my-app', '1d')

  if (breakerLoading || spendLoading) {
    return <div>Loading...</div>
  }

  return (
    <div>
      <h2>Circuit Breaker: {breaker?.state}</h2>
      <p>Spent: ${spend?.used_usd} / ${spend?.limit_usd}</p>
      <ProgressBar value={spend?.fraction ?? 0} />
    </div>
  )
}
```

## Budget Request Workflow

```typescript
import { FuseboxClient } from '@fusebox/sdk'

const fusebox = new FuseboxClient({
  baseURL: 'http://localhost:8080',
  tenant: 'my-app',
})

// Agent requests budget increase
async function requestMoreBudget() {
  const request = await fusebox.budget.requestIncrease({
    limit_usd: 200,
    window: 'day',
    reason: 'Running large batch job',
    ttl_seconds: 86400, // 24 hours
  })
  
  console.log(`Request created: ${request.request_id}`)
  console.log(`Status: ${request.status}`) // 'pending'
  
  return request
}

// Admin approves request
async function approveRequest(requestId: string) {
  const approved = await fusebox.budget.approveRequest(requestId)
  console.log(`Approved! New limit: $${approved.limit_usd}`)
}

// List pending requests
async function listPending() {
  const { requests } = await fusebox.budget.listRequests({ 
    status: 'pending' 
  })
  
  for (const req of requests) {
    console.log(`${req.request_id}: $${req.limit_usd}/${req.window}`)
    console.log(`Reason: ${req.reason}`)
  }
}
```

## Event Streaming

```typescript
import { FuseboxClient } from '@fusebox/sdk'

const fusebox = new FuseboxClient({
  baseURL: 'http://localhost:8080',
  tenant: 'my-app',
})

// Stream events in real-time
async function watchEvents() {
  console.log('Watching events...')
  
  for await (const event of fusebox.events.stream()) {
    console.log(`[${event.kind}] ${event.tenant}`)
    console.log(event.data)
  }
}

// Or list historical events
async function getRecentEvents() {
  const { events } = await fusebox.events.list({ 
    limit: 100,
    offset: 0,
  })
  
  return events
}
```

## Error Handling

```typescript
import { 
  FuseboxError, 
  BudgetExceededError, 
  BreakerOpenError 
} from '@fusebox/sdk'

async function handleRequest() {
  try {
    const response = await openai.chat.completions.create({...})
    return response
  } catch (error) {
    if (error instanceof BudgetExceededError) {
      // Budget limit hit - request was blocked
      console.log('Budget exceeded')
      // Maybe request an increase?
      await fusebox.budget.requestIncrease({...})
    } else if (error instanceof BreakerOpenError) {
      // Circuit breaker is open - system is protecting itself
      console.log('Breaker open, try again later')
    } else if (error instanceof FuseboxError) {
      // Other Fusebox error
      console.log(`Fusebox error: ${error.message}`)
    } else {
      // Upstream error (OpenAI, network, etc.)
      console.log('Upstream error:', error)
    }
  }
}
```

## TypeScript Types

```typescript
import type {
  FuseboxConfig,
  BreakerState,
  SpendResponse,
  BudgetRequest,
  Event,
  TimeWindow,
  BudgetWindow,
} from '@fusebox/sdk'

// Fully typed responses
const state: BreakerState = await fusebox.breaker.getState()
const spend: SpendResponse = await fusebox.spend.get({ window: '1d' })
const request: BudgetRequest = await fusebox.budget.requestIncrease({...})
```

## Advanced: Custom Middleware

```typescript
import { FuseboxOpenAI } from '@fusebox/sdk/openai'

class CustomFuseboxOpenAI extends FuseboxOpenAI {
  async chat.completions.create(params: any) {
    // Before request
    console.log('Making request...')
    
    try {
      const response = await super.chat.completions.create(params)
      
      // After successful request
      console.log('Request succeeded')
      return response
    } catch (error) {
      // After failed request
      console.log('Request failed:', error)
      throw error
    }
  }
}
```
