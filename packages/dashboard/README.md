# Fusebox Dashboard

Real-time web dashboard for monitoring and managing Fusebox.

## Features

- 📊 Real-time cost monitoring
- 💰 Budget usage visualization
- 🔴 Circuit breaker status
- 📈 Historical trends
- 👥 Multi-tenant management
- 📝 Budget request approval workflow
- ⚙️ Configuration management

## Tech Stack

- **Framework**: Next.js 14 (App Router)
- **UI**: React 18 + TailwindCSS
- **Charts**: Recharts
- **Icons**: Lucide React
- **Data Fetching**: TanStack Query
- **Type Safety**: TypeScript

## Development

```bash
# Install dependencies
npm install

# Start dev server
npm run dev

# Open http://localhost:3000
```

## Environment Variables

Create `.env.local`:

```env
NEXT_PUBLIC_FUSEBOX_URL=http://localhost:8080
NEXT_PUBLIC_REFRESH_INTERVAL=5000
```

## Building for Production

```bash
# Build
npm run build

# Start production server
npm start
```

## Docker Deployment

```bash
# Build image
docker build -t fusebox-dashboard .

# Run container
docker run -p 3000:3000 \
  -e NEXT_PUBLIC_FUSEBOX_URL=http://fusebox:8080 \
  fusebox-dashboard
```

## Features Roadmap

### Phase 1 (v0.2.0) ✅
- [x] Dashboard home with overview
- [x] Real-time cost charts
- [x] Budget usage meters
- [x] Circuit breaker status
- [x] Tenant selector

### Phase 2 (v0.3.0)
- [ ] Budget request management
- [ ] Historical data explorer
- [ ] Alert configuration
- [ ] User management
- [ ] API key management

### Phase 3 (v0.4.0)
- [ ] Cost forecasting
- [ ] Anomaly detection UI
- [ ] Custom dashboards
- [ ] Export reports
- [ ] Webhooks configuration

## Screenshots

(Coming soon)

## License

Apache 2.0
