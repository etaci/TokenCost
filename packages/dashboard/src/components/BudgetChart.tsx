'use client'

import { useQuery } from '@tanstack/react-query'

const FUSEBOX_URL = process.env.NEXT_PUBLIC_FUSEBOX_URL || 'http://localhost:8080'

interface SpendData {
  used_usd: number
  limit_usd: number
  fraction: number
  tenant: string
  window: string
}

export function BudgetChart() {
  const { data, isLoading } = useQuery<SpendData>({
    queryKey: ['spend', 'default'],
    queryFn: async () => {
      const res = await fetch(`${FUSEBOX_URL}/v1/spend?tenant=default&window=1d`)
      if (!res.ok) throw new Error('Failed to fetch budget')
      return res.json()
    },
  })

  const percentage = data ? (data.fraction * 100).toFixed(1) : 0
  const used = data?.used_usd.toFixed(2) || '0.00'
  const limit = data?.limit_usd.toFixed(2) || '50.00'

  return (
    <div className="bg-surface border border-border rounded-lg p-6">
      <h2 className="text-lg font-semibold mb-6">Budget Usage</h2>

      {isLoading ? (
        <div className="h-32 flex items-center justify-center text-muted">
          Loading...
        </div>
      ) : (
        <div className="space-y-4">
          {/* Progress Bar */}
          <div className="relative h-8 bg-background rounded-full overflow-hidden">
            <div
              className="absolute inset-y-0 left-0 bg-accent transition-all duration-500"
              style={{ width: `${percentage}%` }}
            />
            <div className="absolute inset-0 flex items-center justify-center text-sm font-medium">
              {percentage}%
            </div>
          </div>

          {/* Stats */}
          <div className="grid grid-cols-3 gap-4 text-center">
            <div>
              <p className="text-2xl font-bold">${used}</p>
              <p className="text-sm text-muted">Used</p>
            </div>
            <div>
              <p className="text-2xl font-bold">${limit}</p>
              <p className="text-sm text-muted">Limit</p>
            </div>
            <div>
              <p className="text-2xl font-bold">
                ${(parseFloat(limit) - parseFloat(used)).toFixed(2)}
              </p>
              <p className="text-sm text-muted">Remaining</p>
            </div>
          </div>

          {/* Warning */}
          {data && data.fraction > 0.8 && (
            <div className="p-3 bg-yellow-50 border border-yellow-200 rounded-lg">
              <p className="text-sm text-yellow-800">
                ⚠️ Budget is {percentage}% used. Consider requesting an increase.
              </p>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
