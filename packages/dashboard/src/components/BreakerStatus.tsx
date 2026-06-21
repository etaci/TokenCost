'use client'

import { useQuery } from '@tanstack/react-query'
import { AlertCircle, CheckCircle, XCircle } from 'lucide-react'

const FUSEBOX_URL = process.env.NEXT_PUBLIC_FUSEBOX_URL || 'http://localhost:8080'

interface BreakerState {
  state: 'closed' | 'open' | 'half_open'
  tenant: string
  reason?: string
  opened_at?: string
}

export function BreakerStatus() {
  const { data, isLoading, error } = useQuery<BreakerState>({
    queryKey: ['breaker', 'default'],
    queryFn: async () => {
      const res = await fetch(`${FUSEBOX_URL}/v1/breaker/state?tenant=default`)
      if (!res.ok) throw new Error('Failed to fetch breaker state')
      return res.json()
    },
  })

  return (
    <div className="bg-surface border border-border rounded-lg p-6">
      <h2 className="text-lg font-semibold mb-4">Circuit Breaker</h2>

      {isLoading && <p className="text-muted">Loading...</p>}
      {error && <p className="text-red-600">Error loading status</p>}

      {data && (
        <div className="space-y-4">
          <div className="flex items-center gap-3">
            {data.state === 'closed' && (
              <>
                <CheckCircle className="w-8 h-8 text-green-500" />
                <div>
                  <p className="font-semibold">Closed</p>
                  <p className="text-sm text-muted">Traffic flowing normally</p>
                </div>
              </>
            )}
            {data.state === 'open' && (
              <>
                <XCircle className="w-8 h-8 text-red-500" />
                <div>
                  <p className="font-semibold">Open</p>
                  <p className="text-sm text-muted">Traffic blocked</p>
                </div>
              </>
            )}
            {data.state === 'half_open' && (
              <>
                <AlertCircle className="w-8 h-8 text-yellow-500" />
                <div>
                  <p className="font-semibold">Half-Open</p>
                  <p className="text-sm text-muted">Testing recovery</p>
                </div>
              </>
            )}
          </div>

          {data.reason && (
            <div className="p-3 bg-background rounded border border-border">
              <p className="text-sm text-muted">Reason:</p>
              <p className="text-sm">{data.reason}</p>
            </div>
          )}

          {data.state !== 'closed' && (
            <button className="w-full px-4 py-2 bg-accent hover:bg-accent-hover text-white rounded-lg transition-colors">
              Reset Breaker
            </button>
          )}
        </div>
      )}
    </div>
  )
}
