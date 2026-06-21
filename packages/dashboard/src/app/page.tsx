import { Activity, AlertCircle, DollarSign, Zap } from 'lucide-react'
import { BreakerStatus } from '@/components/BreakerStatus'
import { BudgetChart } from '@/components/BudgetChart'
import { SpendChart } from '@/components/SpendChart'
import { TenantSelector } from '@/components/TenantSelector'

export default function Home() {
  return (
    <main className="min-h-screen p-8">
      <div className="max-w-7xl mx-auto">
        {/* Header */}
        <div className="mb-8">
          <h1 className="text-4xl font-bold mb-2">
            ⚡ Fusebox Dashboard
          </h1>
          <p className="text-muted">
            Real-time cost monitoring and circuit breaker management
          </p>
        </div>

        {/* Tenant Selector */}
        <div className="mb-6">
          <TenantSelector />
        </div>

        {/* Overview Cards */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6 mb-8">
          <OverviewCard
            icon={<DollarSign className="w-5 h-5" />}
            title="Today's Spend"
            value="$24.56"
            change="+12.5%"
            trend="up"
          />
          <OverviewCard
            icon={<Activity className="w-5 h-5" />}
            title="Requests"
            value="1,234"
            change="+5.2%"
            trend="up"
          />
          <OverviewCard
            icon={<Zap className="w-5 h-5" />}
            title="Budget Used"
            value="49%"
            change="-2.1%"
            trend="down"
          />
          <OverviewCard
            icon={<AlertCircle className="w-5 h-5" />}
            title="Circuit Breaker"
            value="Closed"
            status="healthy"
          />
        </div>

        {/* Main Content */}
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          {/* Left Column - Charts */}
          <div className="lg:col-span-2 space-y-6">
            <SpendChart />
            <BudgetChart />
          </div>

          {/* Right Column - Status */}
          <div className="space-y-6">
            <BreakerStatus />
          </div>
        </div>
      </div>
    </main>
  )
}

function OverviewCard({
  icon,
  title,
  value,
  change,
  trend,
  status,
}: {
  icon: React.ReactNode
  title: string
  value: string
  change?: string
  trend?: 'up' | 'down'
  status?: 'healthy' | 'warning' | 'critical'
}) {
  return (
    <div className="bg-surface border border-border rounded-lg p-6">
      <div className="flex items-center justify-between mb-4">
        <div className="text-muted">{icon}</div>
        {change && (
          <span
            className={`text-sm ${
              trend === 'up' ? 'text-green-600' : 'text-red-600'
            }`}
          >
            {change}
          </span>
        )}
      </div>
      <h3 className="text-sm text-muted mb-1">{title}</h3>
      <p className="text-2xl font-bold">{value}</p>
      {status && (
        <div className="mt-2">
          <span
            className={`inline-block w-2 h-2 rounded-full ${
              status === 'healthy'
                ? 'bg-green-500'
                : status === 'warning'
                ? 'bg-yellow-500'
                : 'bg-red-500'
            }`}
          />
        </div>
      )}
    </div>
  )
}
