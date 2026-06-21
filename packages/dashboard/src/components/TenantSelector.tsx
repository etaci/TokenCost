'use client'

import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'

const FUSEBOX_URL = process.env.NEXT_PUBLIC_FUSEBOX_URL || 'http://localhost:8080'

export function TenantSelector() {
  const [selectedTenant, setSelectedTenant] = useState('default')

  // In a real app, fetch available tenants from the API
  const tenants = ['default', 'power-user', 'enterprise', 'free-tier']

  return (
    <div className="flex items-center gap-4">
      <label htmlFor="tenant" className="text-sm font-medium">
        Tenant:
      </label>
      <select
        id="tenant"
        value={selectedTenant}
        onChange={(e) => setSelectedTenant(e.target.value)}
        className="px-4 py-2 border border-border rounded-lg bg-surface focus:outline-none focus:ring-2 focus:ring-accent"
      >
        {tenants.map((tenant) => (
          <option key={tenant} value={tenant}>
            {tenant}
          </option>
        ))}
      </select>
    </div>
  )
}
