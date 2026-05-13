<div align="center">

<!-- Logo placeholder — replace with docs/logo.svg before launch -->
<h1>⚡ Fusebox</h1>

**The autonomous cost circuit breaker for AI agents.**

Stop runaway agents before they burn your budget.

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Status: Alpha](https://img.shields.io/badge/status-alpha-orange.svg)](#-project-status)
[![Made with Rust](https://img.shields.io/badge/made%20with-rust-orange.svg)](https://www.rust-lang.org)
[![Discord](https://img.shields.io/badge/discord-join-5865F2.svg)](#-community)
[![Twitter Follow](https://img.shields.io/badge/follow-@getfusebox-1da1f2.svg)](#-community)

[Website](https://fusebox.dev) · [Docs](https://fusebox.dev/docs) · [Discord](#-community) · [Quick Start](#-quick-start) · [中文](./README.zh-CN.md)

</div>

---

> *"I went to bed. My agent hit a loop. I woke up to a $4,700 bill."*
>
> — Every AI engineer in 2026. Fusebox is the last line of defense.

---

## 🤔 What is Fusebox?

Fusebox sits between your AI agent and the LLM provider. It watches every dollar in real time. When budgets are exceeded **or** spending anomalies appear, **it trips the circuit breaker** — instantly stopping requests *before* the damage is done.

Think of it as `Hystrix` / `failsafe` for LLM calls — but built for the autonomous-agent era.

```
┌──────────┐      ┌──────────────┐      ┌────────────┐
│ Your App │─────▶│   Fusebox    │─────▶│ OpenAI /   │
│  Agent   │      │ ⚡ Circuit  │      │ Anthropic /│
│   SDK    │◀─────│   Breaker   │◀─────│ Bedrock /…│
└──────────┘      └──────┬───────┘      └────────────┘
                         │
                         ▼
              ❌ DENY  ⬇️ DOWNGRADE  🚦 QUEUE
              (when budget / anomaly tripped)
```

## 😱 Why does this exist?

LLM agents in 2026 are **autonomous, long-running, and expensive**.

Today's tools (LiteLLM, Helicone, Langfuse) tell you *after* the money is gone. They're a thermometer when you needed a thermostat.

| Pain | Tools today | Fusebox |
|---|---|---|
| Background agent stuck in a loop overnight | Wake up, discover damage, fight for refund | **Auto-trip on anomaly**, push notification, $4 lost instead of $4000 |
| Customer abuses your AI feature on free trial | Ban them tomorrow, eat the cost | **Per-user budget**, enforced in-flight |
| Multi-team enterprise spend governance | Manual quarterly reports, finger-pointing | **Real-time per-team budgets + alerts**, audit log |
| Multi-provider chaos (OpenAI + Anthropic + Bedrock) | Three dashboards, three bills, no unified view | **One proxy, one ledger, unified policy** |

## ✨ Features (alpha — what works today)

- ⚡ **Real-time circuit breaker** — `Closed → Open → Half-Open` state machine with automatic cooldown + manual reset
- 💰 **Multi-window budgets** — `1m / 1h / 1d / 1w / 1mo`, per-tenant overrides, exceedance trips the breaker on the spot
- 📈 **Anomaly detection out of the box** — online EWMA + 3-sigma spend-rate guard, no training data needed
- 🔌 **OpenAI + Anthropic compatible** — drop-in replacement for `baseURL`, supports streaming pass-through
- 🧮 **Accurate cost accounting** — `tiktoken-rs` for OpenAI, character heuristic for Anthropic, post-flight reconciliation against the upstream `usage` field
- 🗂️ **Persistent ledger** — SQLite by default (zero config), Postgres + TimescaleDB for production
- 🛡️ **Audit log** — every breaker transition (auto trip, half-open recovery, manual reset) is durably recorded
- 🛠️ **Operator CLI** — `start`, `status`, `tail`, `doctor`, `config`, `budget`, `breaker`
- 🦀 **Rust core** — single static binary; sub-millisecond decisions on the hot path
- 🧰 **Self-hosted, Apache-2.0** — your data stays yours, no SaaS lock-in

> Heads up: SDKs, dashboard, and the MCP server are part of Phase 2 — see the [roadmap](#%EF%B8%8F-roadmap) for what's coming.

## 🚀 Quick Start (60 seconds, from source)

```bash
# 1. Clone + build the workspace
git clone https://github.com/fusebox-dev/fusebox && cd fusebox
cargo build --release

# 2. Start the proxy (zero config — defaults to SQLite at ~/.fusebox/data.db)
cargo run -p fusebox-cli --release -- start

# 3. Point your app at Fusebox instead of OpenAI / Anthropic directly
export OPENAI_API_BASE="http://localhost:8080/v1"
export ANTHROPIC_BASE_URL="http://localhost:8080"

# 4. (Optional) tighten the default $50/day budget
cargo run -p fusebox-cli --release -- budget set --tenant default --limit '10/day'
```

That's it. Run your agent. If it tries to burn $4,700, Fusebox will stop at $10.

> Pre-built binaries (`curl install`, Docker Compose, Helm) are part of Phase 3. For now build from source.

## 🧑‍💻 Using it from your code

Until the typed SDKs (Phase 2) ship, Fusebox is fully driven by **environment variables** — drop-in for any OpenAI / Anthropic client.

### TypeScript / JavaScript

```ts
import OpenAI from 'openai';

// The entire integration is one URL change.
const openai = new OpenAI({
  baseURL: 'http://localhost:8080/v1',
  apiKey: process.env.OPENAI_API_KEY,
  defaultHeaders: {
    // Optional but recommended: tag spend by user / project.
    'X-Fusebox-Tenant': 'user-42',
    'X-Fusebox-Project': 'background-coding-agent',
  },
});

await openai.chat.completions.create({
  model: 'gpt-4o',
  messages: [{ role: 'user', content: 'hi' }],
});
// Fusebox now tracks every cent. Trip happens automatically.
```

### Python

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://localhost:8080/v1",
    default_headers={
        "X-Fusebox-Tenant": "user-42",
        "X-Fusebox-Project": "agent-x",
    },
)
```

### Anthropic

```python
from anthropic import Anthropic

# Anthropic clients accept a base_url just the same; Fusebox speaks /v1/messages.
client = Anthropic(base_url="http://localhost:8080", api_key="sk-...")
```

## 🧰 Operator CLI

The single `fusebox` binary is the operator's swiss-army knife.

```bash
fusebox start                                    # boot the proxy in the foreground
fusebox status --tenant user-42                  # current breaker state for a tenant
fusebox tail                                     # live spend stream (ts / tenant / model / cost)
fusebox doctor                                   # config + ledger + pricing self-check
fusebox config init                              # write a starter fusebox.yaml
fusebox config validate                          # parse + summarise the active config

fusebox budget set --tenant user-42 --limit '5/hour'   # tighten one tenant
fusebox budget set --tenant default --limit 50/day      # change the global fallback
fusebox budget list                                     # show every budget that's set
fusebox budget clear --tenant user-42                   # drop the override

fusebox breaker list                             # snapshot every breaker (talks to the running proxy)
fusebox breaker status --tenant user-42          # one tenant
fusebox breaker reset --tenant user-42           # manual override → Closed
```

## 🌐 Control-plane HTTP endpoints

The proxy doubles as a small JSON control plane so dashboards, automation, and the future MCP server can introspect state without a side-channel database.

| Method | Path | What it returns |
|---|---|---|
| `GET`  | `/health`                    | Liveness probe |
| `GET`  | `/metrics`                   | Prometheus exposition |
| `GET`  | `/v1/breaker/state`          | Current label for the caller's tenant |
| `POST` | `/v1/breaker/reset`          | Operator override → Closed (`{ "tenant": "..." }`) |
| `GET`  | `/v1/breakers`               | Snapshot of every tenant's breaker |
| `GET`  | `/v1/spend?window=1d`        | Used / limit / fraction for one tenant |
| `GET`  | `/v1/events?seconds=3600`    | Recent spend events (for live dashboards) |
| `GET`  | `/v1/audit/breakers`         | Breaker transition audit log |
| `POST` | `/v1/chat/completions`       | OpenAI passthrough (streaming + non-streaming) |
| `POST` | `/v1/messages`               | Anthropic passthrough |

All control-plane endpoints accept an `X-Fusebox-Tenant` header (defaults to `default`).

## 📊 Dashboard

[ Screenshot placeholder — `docs/dashboard.png` after Phase 2 ]

The dashboard (Phase 2) is going to give you:
- 🔴 **Breaker status cards** — live `Closed / Open / Half-Open` per tenant, with reason + cooldown countdown
- 📈 **Spend stream** — real-time event log (Vercel-deployments-style), powered by `/v1/events`
- 🕯️ **Burn-rate gauge** — current vs. budget velocity, with sigma anomaly markers
- 📐 **Budget editor** — declarative YAML or click-and-save UI, fed by `/v1/spend` + `fusebox budget set`

## ⚖️ How does it compare?

| | Fusebox | LiteLLM | Helicone | Langfuse | Cloudflare AI Gateway |
|---|---|---|---|---|---|
| Open-source | ✅ Apache-2.0 | ✅ MIT | 🟡 Partial | ✅ MIT | ❌ |
| Self-hostable | ✅ | ✅ | ✅ | ✅ | ❌ |
| **In-flight cost block** | ✅ **(core)** | 🟡 callback only | ❌ | ❌ | 🟡 (rate-limit only) |
| Circuit-breaker state machine | ✅ | ❌ | ❌ | ❌ | ❌ |
| Anomaly detection (no training) | ✅ EWMA | ❌ | 🟡 alerts | ❌ | ❌ |
| MCP integration | 🟡 Phase 2 | ❌ | ❌ | ❌ | ❌ |
| Multi-provider | ✅ 2 (Phase 1), 6+ (Phase 2) | ✅ 100+ | ✅ | ✅ | 🟡 |
| Sub-5ms p99 overhead | ✅ Rust | ❌ Python | ❌ | n/a | ✅ |
| Beautiful dashboard | 🟡 Phase 2 | 🟡 | ✅ | ✅ | ✅ |

**Fusebox plays nicely with LiteLLM** — chain them. LiteLLM does smart routing across 100+ providers; Fusebox watches the wallet. We're not competing; we're complementary.

## 🗺️ Roadmap

### Phase 1 — Core (Day 1-30) ✅ *we are here*
- [x] Repo + workspace skeleton
- [x] Proxy MVP — OpenAI compat, streaming pass-through, accurate `usage` reconciliation
- [x] Anthropic API support (`/v1/messages`)
- [x] Cost ledger — SQLite default + Postgres / TimescaleDB schema (TimescaleDB hypertable optional)
- [x] Circuit-breaker state machine — Closed / Open / Half-Open with cooldown + half-open trial gating
- [x] EWMA + 3-sigma anomaly detection (per-tenant, online, no training data)
- [x] Per-tenant budget overrides (`fusebox budget set`)
- [x] Breaker manual override (`POST /v1/breaker/reset` + `fusebox breaker reset`)
- [x] Audit log for every breaker transition (`/v1/audit/breakers`)
- [x] Operator CLI: `start / status / tail / doctor / config / budget / breaker`
- [x] Prometheus `/metrics` exporter
- [ ] Pricing data sync from upstream (LiteLLM `model_prices…json` → `pricing/*.yaml` PR)

### Phase 2 — Beta (Day 31-60)
- [ ] Next.js 15 dashboard with real-time SSE
- [ ] Auth (Better-Auth, no SaaS lock-in)
- [ ] TypeScript + Python SDKs (`@fusebox/sdk`, `fusebox` on PyPI)
- [ ] Bedrock / Google / OpenRouter providers
- [ ] MCP server — let agents self-check their own budget (`get_budget`, `request_budget_increase`)
- [ ] Streaming SSE token-accurate post-flight reconciliation
- [ ] Postgres ledger backend (the schema is shipped; the runtime path lands here)
- [ ] Hot-reload `fusebox.yaml` on SIGHUP

### Phase 3 — Launch (Day 61-90)
- [ ] One-line install (`curl -fsSL fusebox.dev/install.sh | sh`)
- [ ] Helm chart + systemd unit + Docker Compose
- [ ] Mintlify docs site
- [ ] 5 integration tutorials (Claude Code / OpenHands / LangGraph / CrewAI / self-built)
- [ ] Demo video + landing page
- [ ] HN / PH / Reddit launch

### Beyond
- [ ] Self-hosted Cloud Console (multi-instance management)
- [ ] ML-based anomaly detection (Isolation Forest via `linfa`)
- [ ] Prompt-content guardrails (NeMo / Llama Guard pluggable)
- [ ] Audit log export (SIEM-friendly)
- [ ] SOC 2 Type 1 (for the enterprise edition, separate repo)

## 📦 Project status

> **Alpha — under heavy active development.** The Rust core (proxy + policy + ledger + CLI) is functional end-to-end. APIs for SDKs and the dashboard will land in Phase 2; expect breaking changes on `/v1/*` admin endpoints until then.
>
> Star ⭐ to follow along, [open an issue](https://github.com/fusebox-dev/fusebox/issues/new) to shape the product, or [join Discord](#-community) to talk to the maintainer directly.

Today is **Day 2** of the 90-day public-build journey.

## 🏗️ Architecture

For the deep dive, see:
- [`架构.md`](./架构.md) — product vision, competitive analysis, 90-day roadmap
- [`技术.md`](./技术.md) — tech stack rationale, per-component decisions, dependency lock
- [Architecture overview](./docs/architecture.md) — public version *(coming Phase 2)*

Quick mental model:

```
                        ┌─────────────────────────┐
   Your App / Agent ───▶│   Pre-flight Validator  │  ← parses request, counts tokens
                        ├─────────────────────────┤
                        │      Policy Engine      │  ← multi-window budgets +
                        │  (budget · anomaly)     │    EWMA spend-rate detector
                        ├─────────────────────────┤
                        │  Circuit Breaker (SM)   │  ← Closed / Open / Half-Open
                        ├─────────────────────────┤
                        │     Cost Ledger         │  ← SQLite default,
                        │  + Breaker audit log    │    Postgres + TimescaleDB optional
                        └────────────┬────────────┘
                                     │
                            ▼ Decision: ALLOW | DENY | DOWNGRADE | QUEUE
```

### Workspace layout

```
fusebox/
├── crates/
│   ├── fusebox-core/        # shared types: Budget, Decision, Pricing, TenantId, …
│   ├── fusebox-ledger/      # LedgerStore trait + SQLite + in-memory backends
│   ├── fusebox-policy/      # PolicyEngine + Breaker SM + EWMA anomaly detector
│   ├── fusebox-proxy/       # HTTP gateway (axum 0.7) + control-plane endpoints
│   └── fusebox-cli/         # `fusebox` binary
├── pricing/                 # USD-per-1M-tokens YAML, embedded at build time
│   ├── openai.yaml
│   └── anthropic.yaml
├── 架构.md / 技术.md         # Chinese-first design docs (Phase 1)
└── README.md                # this file
```

## 🤝 Contributing

We love early contributors. **Day-1 stars and PRs help shape Fusebox forever.**

- 🔰 [Good first issues](https://github.com/fusebox-dev/fusebox/labels/good%20first%20issue)
- 🦀 Rust folks: proxy / policy engine / CLI
- ⚛️ React folks: dashboard (Phase 2)
- 🐍 Python folks: SDK + agent integrations (Phase 2)
- ✍️ Writers: docs / blog posts / tutorials
- 🎨 Designers: logo, dashboard polish, landing page

### Local dev loop

```bash
git clone https://github.com/fusebox-dev/fusebox && cd fusebox

# Build everything (proxy + CLI share a workspace)
cargo build

# Run the full test suite
cargo test --workspace

# Format + lint (must pass in CI)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings

# Boot the proxy with verbose logs
RUST_LOG=fusebox=debug,info cargo run -p fusebox-cli -- start
```

The Rust toolchain is pinned in [`rust-toolchain.toml`](./rust-toolchain.toml) (currently 1.82). No nightly required.

## 💬 Community

- 💬 [Discord server](https://discord.gg/fusebox) — fastest way to reach the maintainer
- 🐦 [@getfusebox on Twitter/X](https://twitter.com/getfusebox) — release notes + horror-story-of-the-week
- 📰 [Newsletter](https://fusebox.dev/newsletter) — monthly digest
- 📺 [YouTube](https://youtube.com/@fusebox) — demos + deep-dive talks

## 🛡️ Security

Found a vulnerability? **Please don't open a public issue.** Email `security@fusebox.dev` (or DM on Discord). See [SECURITY.md](./SECURITY.md).

We take security seriously: no API keys are ever stored (pass-through only), TLS-first, full audit log, and SBOM published with every release.

## 📜 License

[Apache License 2.0](./LICENSE) — use it for anything, including commercial products. We only ask that you keep the LICENSE file when you redistribute.

## 🙏 Acknowledgements

Fusebox stands on the shoulders of:
- [LiteLLM](https://github.com/BerriAI/litellm) — pricing data we sync from upstream
- [Tokio](https://tokio.rs/) + [Axum](https://github.com/tokio-rs/axum) — Rust async runtime
- [TimescaleDB](https://www.timescale.com/) — time-series Postgres extension
- [Tremor](https://tremor.so/) — the dashboard charts you'll love
- [Anthropic](https://anthropic.com/) — for shipping MCP and making agent self-awareness possible
- All the engineers who lost money to runaway agents and shared the story publicly. This is for you.

---

<div align="center">

**Built with ⚡ in the open. Star to follow along.**

[⭐ Star on GitHub](https://github.com/fusebox-dev/fusebox) · [Try the demo](https://fusebox.dev) · [Read the manifesto](https://fusebox.dev/blog/manifesto)

</div>
