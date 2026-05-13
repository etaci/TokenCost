#!/usr/bin/env python3
"""Pull LiteLLM's model price feed and rewrite Fusebox's `pricing/*.yaml`.

We treat LiteLLM as the source of truth because their list is community-
maintained and updated within hours of a price change. We only re-emit
the providers we actively forward (OpenAI, Anthropic, Google, Bedrock,
OpenRouter); everything else is ignored.

Run manually:    `python scripts/sync-pricing.py`
Run from CI:     see .github/workflows/pricing-sync.yml
Exit codes:      0 = no change, 0 = updated, 1 = network/parse failure.
"""
from __future__ import annotations

import json
import sys
import urllib.error
import urllib.request
from collections import OrderedDict
from pathlib import Path
from typing import Any

LITELLM_URL = (
    "https://raw.githubusercontent.com/BerriAI/litellm/main/"
    "model_prices_and_context_window.json"
)

# (litellm provider key, our pricing/<name>.yaml stem, Fusebox Provider enum)
PROVIDERS: list[tuple[str, str, str]] = [
    ("openai", "openai", "openai"),
    ("anthropic", "anthropic", "anthropic"),
    ("gemini", "google", "google"),
    ("vertex_ai-language-models", "google", "google"),
    ("bedrock", "bedrock", "bedrock"),
    ("openrouter", "openrouter", "openrouter"),
]

REPO_ROOT = Path(__file__).resolve().parents[1]
PRICING_DIR = REPO_ROOT / "pricing"


def fetch_litellm() -> dict[str, Any]:
    """Download the upstream JSON. Fails fast on network errors so the CI
    job stays visibly red rather than silently committing nothing."""
    req = urllib.request.Request(LITELLM_URL, headers={"User-Agent": "fusebox-sync/0.1"})
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            data = resp.read()
    except (urllib.error.URLError, TimeoutError) as e:
        print(f"fatal: failed to fetch LiteLLM pricing: {e}", file=sys.stderr)
        sys.exit(1)
    try:
        return json.loads(data)
    except json.JSONDecodeError as e:
        print(f"fatal: LiteLLM JSON parse failed: {e}", file=sys.stderr)
        sys.exit(1)


def matches_provider(entry: dict[str, Any], litellm_key: str) -> bool:
    """LiteLLM tags each model with `litellm_provider`. We accept exact
    matches and the `vertex_ai-*` prefix family so gemini-on-vertex stays
    in the google.yaml file."""
    provider = entry.get("litellm_provider")
    if not provider:
        return False
    if provider == litellm_key:
        return True
    if litellm_key.endswith("-language-models") and provider.startswith(litellm_key.rstrip("-language-models")):
        return True
    return False


def to_per_million(value: Any) -> float:
    """LiteLLM stores prices as USD per single token. We store per 1M for
    human readability (matches the public pricing pages)."""
    try:
        return round(float(value) * 1_000_000.0, 6)
    except (TypeError, ValueError):
        return 0.0


def extract_model_pricing(entry: dict[str, Any]) -> dict[str, Any] | None:
    input_cost = entry.get("input_cost_per_token")
    output_cost = entry.get("output_cost_per_token")
    if input_cost is None or output_cost is None:
        return None
    pricing: dict[str, Any] = OrderedDict()
    pricing["input_per_1m"] = to_per_million(input_cost)
    pricing["output_per_1m"] = to_per_million(output_cost)
    cache_read = entry.get("cache_read_input_token_cost")
    if cache_read is not None:
        pricing["cache_read_per_1m"] = to_per_million(cache_read)
    cache_write = entry.get("cache_creation_input_token_cost")
    if cache_write is not None:
        pricing["cache_write_per_1m"] = to_per_million(cache_write)
    return pricing


def models_for(data: dict[str, Any], litellm_key: str) -> dict[str, dict[str, Any]]:
    """Pull every model belonging to one provider out of LiteLLM's blob."""
    out: dict[str, dict[str, Any]] = {}
    for raw_name, entry in data.items():
        if raw_name == "sample_spec" or not isinstance(entry, dict):
            continue
        if not matches_provider(entry, litellm_key):
            continue
        pricing = extract_model_pricing(entry)
        if pricing is None:
            continue
        # LiteLLM names like "anthropic/claude-3-5-sonnet" → strip vendor prefix
        # so our YAML matches the bare model id clients send.
        name = raw_name.split("/", 1)[-1] if "/" in raw_name else raw_name
        out[name] = pricing
    return out


def merge_providers(data: dict[str, Any]) -> dict[str, dict[str, dict[str, Any]]]:
    """Several litellm keys map to the same Fusebox file (e.g. gemini +
    vertex_ai-language-models → google.yaml)."""
    by_file: dict[str, dict[str, dict[str, Any]]] = {}
    for litellm_key, file_stem, _ in PROVIDERS:
        bucket = by_file.setdefault(file_stem, {})
        bucket.update(models_for(data, litellm_key))
    return by_file


# --- YAML emitter (hand-rolled, no PyYAML dep so the CI image stays small) -----

def quote_if_needed(s: str) -> str:
    if any(c in s for c in (":", "#", "{", "}", "[", "]", ",", "&", "*", "!", "|", ">", "\"", "'", "%", "@", "`")):
        return '"' + s.replace('"', '\\"') + '"'
    return s


def dump_yaml(provider_enum: str, last_updated: str, models: dict[str, dict[str, Any]]) -> str:
    lines = [
        f"provider: {provider_enum}",
        f'last_updated: "{last_updated}"',
        "# Auto-generated by scripts/sync-pricing.py. Source: LiteLLM",
        "# (BerriAI/litellm model_prices_and_context_window.json).",
        "# Hand-edits are okay but will be overwritten on the next sync.",
        "models:",
    ]
    for name in sorted(models):
        pricing = models[name]
        lines.append(f"  {quote_if_needed(name)}:")
        for key in ("input_per_1m", "output_per_1m", "cache_read_per_1m", "cache_write_per_1m"):
            if key in pricing:
                lines.append(f"    {key}: {pricing[key]}")
    return "\n".join(lines) + "\n"


def main() -> int:
    if not PRICING_DIR.exists():
        PRICING_DIR.mkdir(parents=True)
    print(f"fetching: {LITELLM_URL}")
    data = fetch_litellm()
    print(f"got {len(data)} model entries")

    today = __import__("datetime").datetime.utcnow().strftime("%Y-%m-%d")
    by_file = merge_providers(data)

    changed = False
    for _, file_stem, provider_enum in PROVIDERS:
        models = by_file.get(file_stem, {})
        if not models:
            continue
        out_path = PRICING_DIR / f"{file_stem}.yaml"
        body = dump_yaml(provider_enum, today, models)
        existing = out_path.read_text(encoding="utf-8") if out_path.exists() else ""
        # `last_updated` changes every run, so compare with it normalised out.
        def normalise(s: str) -> str:
            return "\n".join(
                line for line in s.splitlines() if not line.startswith("last_updated:")
            )
        if normalise(existing) == normalise(body):
            print(f"  {file_stem}.yaml: {len(models)} models — unchanged")
            continue
        out_path.write_text(body, encoding="utf-8")
        print(f"  {file_stem}.yaml: wrote {len(models)} models")
        changed = True

    if not changed:
        print("no pricing changes")
    return 0


if __name__ == "__main__":
    sys.exit(main())
