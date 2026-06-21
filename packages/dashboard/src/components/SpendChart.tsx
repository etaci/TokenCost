'use client'

import { useQuery } from '@tanstack/react-query'
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from 'recharts'

const FUSEBOX_URL = process.env.NEXT_PUBLIC_FUSEBOX_URL || 'http://localhost:8080'

export function SpendChart() {
  const { data, isLoading } = useQuery({
    queryKey: ['spend', 'default'],
    queryFn: async () => {
      const res = await fetch(`${FUSEBOX_URL}/v1/spend?tenant=default&window=1d`)
      if (!res.ok) throw new Error('Failed to fetch spend')
      return res.json()
    },
  })

  // Mock data for demo
  const chartData = [
    { time: '00:00', cost: 2.3 },
    { time: '04:00', cost: 1.8 },
    { time: '08:00', cost: 5.2 },
    { time: '12:00', cost: 8.7 },
    { time: '16:00', cost: 12.4 },
    { time: '20:00', cost: 9.1 },
    { time: 'Now', cost: data?.used_usd || 0 },
  ]

  return (
    <div className="bg-surface border border-border rounded-lg p-6">
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-lg font-semibold">Spending Over Time</h2>
        <div className="text-sm text-muted">Last 24 hours</div>
      </div>

      {isLoading ? (
        <div className="h-64 flex items-center justify-center text-muted">
          Loading...
        </div>
      ) : (
        <ResponsiveContainer width="100%" height={300}>
          <LineChart data={chartData}>
            <CartesianGrid strokeDasharray="3 3" stroke="#E7E1D7" />
            <XAxis dataKey="time" stroke="#5C635D" />
            <YAxis stroke="#5C635D" />
            <Tooltip
              contentStyle={{
                background: '#FBF9F5',
                border: '1px solid #E7E1D7',
                borderRadius: '8px',
              }}
            />
            <Line
              type="monotone"
              dataKey="cost"
              stroke="#C4612F"
              strokeWidth={2}
              dot={{ fill: '#C4612F' }}
            />
          </LineChart>
        </ResponsiveContainer>
      )}
    </div>
  )
}
