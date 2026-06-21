import type {
  FuseboxConfig,
  BreakerState,
  SpendResponse,
  BudgetRequest,
  Event,
  TimeWindow,
  BudgetWindow,
} from './types'
import { FuseboxError, BudgetExceededError, BreakerOpenError } from './types'

export class FuseboxClient {
  private baseURL: string
  private tenant: string
  private timeout: number

  constructor(config: FuseboxConfig) {
    this.baseURL = config.baseURL.replace(/\/$/, '')
    this.tenant = config.tenant
    this.timeout = config.timeout || 10000
  }

  private async request<T>(path: string, options?: RequestInit): Promise<T> {
    const url = `${this.baseURL}${path}`
    const controller = new AbortController()
    const timeoutId = setTimeout(() => controller.abort(), this.timeout)

    try {
      const response = await fetch(url, {
        ...options,
        signal: controller.signal,
        headers: {
          'Content-Type': 'application/json',
          'X-Fusebox-Tenant': this.tenant,
          ...options?.headers,
        },
      })

      clearTimeout(timeoutId)

      if (!response.ok) {
        const text = await response.text()
        if (response.status === 429) {
          throw new BudgetExceededError(text)
        } else if (response.status === 503) {
          throw new BreakerOpenError(text)
        } else {
          throw new FuseboxError(text, response.status)
        }
      }

      return response.json()
    } catch (error) {
      clearTimeout(timeoutId)
      if (error instanceof FuseboxError) {
        throw error
      }
      throw new FuseboxError(
        error instanceof Error ? error.message : 'Unknown error'
      )
    }
  }

  public breaker = {
    getState: async (): Promise<BreakerState> => {
      return this.request<BreakerState>(`/v1/breaker/state?tenant=${this.tenant}`)
    },

    reset: async (): Promise<void> => {
      await this.request(`/v1/breaker/reset?tenant=${this.tenant}`, {
        method: 'POST',
      })
    },
  }

  public spend = {
    get: async (options: { window?: TimeWindow } = {}): Promise<SpendResponse> => {
      const window = options.window || '1d'
      return this.request<SpendResponse>(
        `/v1/spend?tenant=${this.tenant}&window=${window}`
      )
    },
  }

  public budget = {
    requestIncrease: async (params: {
      limit_usd: number
      window: BudgetWindow
      reason?: string
      ttl_seconds?: number
    }): Promise<BudgetRequest> => {
      return this.request<BudgetRequest>('/v1/budget/requests', {
        method: 'POST',
        body: JSON.stringify({
          tenant: this.tenant,
          ...params,
        }),
      })
    },

    listRequests: async (options: {
      status?: 'pending' | 'approved' | 'rejected'
    } = {}): Promise<{ requests: BudgetRequest[] }> => {
      const params = new URLSearchParams({
        tenant: this.tenant,
        ...(options.status && { status: options.status }),
      })
      return this.request(`/v1/budget/requests?${params}`)
    },

    approveRequest: async (requestId: string): Promise<BudgetRequest> => {
      return this.request(`/v1/budget/requests/${requestId}/approve`, {
        method: 'POST',
      })
    },

    rejectRequest: async (requestId: string, reason?: string): Promise<BudgetRequest> => {
      return this.request(`/v1/budget/requests/${requestId}/reject`, {
        method: 'POST',
        body: JSON.stringify({ reason }),
      })
    },
  }

  public events = {
    list: async (options: {
      limit?: number
      offset?: number
    } = {}): Promise<{ events: Event[] }> => {
      const params = new URLSearchParams({
        tenant: this.tenant,
        ...(options.limit && { limit: options.limit.toString() }),
        ...(options.offset && { offset: options.offset.toString() }),
      })
      return this.request(`/v1/events?${params}`)
    },

    async *stream(): AsyncGenerator<Event> {
      const response = await fetch(
        `${this.baseURL}/v1/events/stream?tenant=${this.tenant}`
      )

      if (!response.ok || !response.body) {
        throw new FuseboxError('Failed to connect to event stream')
      }

      const reader = response.body.getReader()
      const decoder = new TextDecoder()

      try {
        while (true) {
          const { done, value } = await reader.read()
          if (done) break

          const chunk = decoder.decode(value)
          const lines = chunk.split('\n')

          for (const line of lines) {
            if (line.startsWith('data: ')) {
              const data = line.slice(6)
              try {
                yield JSON.parse(data)
              } catch {
                // Skip invalid JSON
              }
            }
          }
        }
      } finally {
        reader.releaseLock()
      }
    },
  }
}

export * from './types'
