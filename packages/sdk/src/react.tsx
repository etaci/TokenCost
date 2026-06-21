import { useState, useEffect } from 'react'
import type { BreakerState, SpendResponse, TimeWindow } from './types'
import { FuseboxClient } from './index'

interface UseFuseboxOptions {
  baseURL: string
  tenant: string
  refreshInterval?: number
}

export function useFuseboxClient(options: UseFuseboxOptions) {
  const [client] = useState(
    () =>
      new FuseboxClient({
        baseURL: options.baseURL,
        tenant: options.tenant,
      })
  )
  return client
}

export function useBreakerState(tenant: string, baseURL = 'http://localhost:8080') {
  const [data, setData] = useState<BreakerState | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<Error | null>(null)

  const client = useFuseboxClient({ baseURL, tenant })

  useEffect(() => {
    let cancelled = false

    async function fetch() {
      try {
        setLoading(true)
        const state = await client.breaker.getState()
        if (!cancelled) {
          setData(state)
          setError(null)
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err : new Error('Unknown error'))
        }
      } finally {
        if (!cancelled) {
          setLoading(false)
        }
      }
    }

    fetch()
    const interval = setInterval(fetch, 5000) // Refresh every 5s

    return () => {
      cancelled = true
      clearInterval(interval)
    }
  }, [tenant, baseURL])

  return { data, loading, error }
}

export function useSpend(
  tenant: string,
  window: TimeWindow = '1d',
  baseURL = 'http://localhost:8080'
) {
  const [data, setData] = useState<SpendResponse | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<Error | null>(null)

  const client = useFuseboxClient({ baseURL, tenant })

  useEffect(() => {
    let cancelled = false

    async function fetch() {
      try {
        setLoading(true)
        const spend = await client.spend.get({ window })
        if (!cancelled) {
          setData(spend)
          setError(null)
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err : new Error('Unknown error'))
        }
      } finally {
        if (!cancelled) {
          setLoading(false)
        }
      }
    }

    fetch()
    const interval = setInterval(fetch, 5000) // Refresh every 5s

    return () => {
      cancelled = true
      clearInterval(interval)
    }
  }, [tenant, window, baseURL])

  return { data, loading, error }
}

export function useBudgetRequests(tenant: string, baseURL = 'http://localhost:8080') {
  const [data, setData] = useState<any[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<Error | null>(null)

  const client = useFuseboxClient({ baseURL, tenant })

  const refresh = async () => {
    try {
      setLoading(true)
      const result = await client.budget.listRequests()
      setData(result.requests)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err : new Error('Unknown error'))
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    refresh()
    const interval = setInterval(refresh, 10000) // Refresh every 10s
    return () => clearInterval(interval)
  }, [tenant, baseURL])

  return { data, loading, error, refresh }
}
