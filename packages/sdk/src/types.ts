export interface FuseboxConfig {
  baseURL: string
  tenant: string
  timeout?: number
}

export interface BreakerState {
  state: 'closed' | 'open' | 'half_open'
  tenant: string
  reason?: string
  opened_at?: string
}

export interface SpendResponse {
  tenant: string
  window: string
  used_usd: number
  limit_usd: number
  fraction: number
  budgets: BudgetRule[]
}

export interface BudgetRule {
  label: string
  limit_usd: number
  window: string
}

export interface BudgetRequest {
  request_id: string
  tenant: string
  limit_usd: number
  window: string
  reason?: string
  status: 'pending' | 'approved' | 'rejected'
  created_at: string
  resolved_at?: string
  resolved_by?: string
}

export interface Event {
  id: string
  tenant: string
  kind: string
  timestamp: string
  data: Record<string, any>
}

export type TimeWindow = '1m' | '1h' | '1d' | '1w' | '1mo'
export type BudgetWindow = 'minute' | 'hour' | 'day' | 'week' | 'month'

export class FuseboxError extends Error {
  constructor(
    message: string,
    public statusCode?: number,
    public response?: any
  ) {
    super(message)
    this.name = 'FuseboxError'
  }
}

export class BudgetExceededError extends FuseboxError {
  constructor(message: string = 'Budget exceeded') {
    super(message, 429)
    this.name = 'BudgetExceededError'
  }
}

export class BreakerOpenError extends FuseboxError {
  constructor(message: string = 'Circuit breaker is open') {
    super(message, 503)
    this.name = 'BreakerOpenError'
  }
}
