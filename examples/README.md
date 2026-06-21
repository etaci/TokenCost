# Examples

Drop-in integration recipes. Each example is a self-contained scratchpad —
copy the relevant snippet into your own project and adjust to taste.

| Example | Stack | What it shows |
|---|---|---|
| [`openai-python`](./openai-python/)            | Python + `openai` | Single-line `base_url` swap; per-tenant tagging via headers |
| [`openai-typescript`](./openai-typescript/)    | Node + `openai`   | Same, in TS |
| [`anthropic-python`](./anthropic-python/)      | Python + `anthropic` | Anthropic Messages API through Fusebox |
| [`runaway-demo`](./runaway-demo/)              | bash + `curl`     | Reproduce the "agent hits a loop" story; watch Fusebox trip |

> Phase 2 will add Claude Code, OpenHands, LangGraph, CrewAI, and a self-hosted
> SDK example. PRs welcome — see [CONTRIBUTING.md](../CONTRIBUTING.md).

## Common setup

All examples assume the proxy is running locally:

```bash
# from repo root
cargo run -p fusebox-cli --release -- start
# proxy now listening on http://localhost:8080
```

Then export your real upstream key (Fusebox is pass-through; we never store
keys):

```bash
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
```
