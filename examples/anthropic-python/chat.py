"""
Anthropic client through Fusebox — single-file demo.

Run:
    pip install anthropic
    python chat.py

Anthropic clients use a `base_url` (no `/v1` suffix); Fusebox listens at
`/v1/messages` to match the official API path.
"""

import os
from anthropic import Anthropic

FUSEBOX_URL = os.environ.get("FUSEBOX_URL", "http://localhost:8080")
TENANT = os.environ.get("FUSEBOX_TENANT", "demo-user")

client = Anthropic(
    base_url=FUSEBOX_URL,
    api_key=os.environ["ANTHROPIC_API_KEY"],
    default_headers={
        "X-Fusebox-Tenant": TENANT,
        "X-Fusebox-Project": "examples/anthropic-python",
    },
)

msg = client.messages.create(
    model="claude-sonnet-4-5",
    max_tokens=256,
    messages=[
        {"role": "user", "content": "Give me 3 reasons to like Rust."},
    ],
)
for block in msg.content:
    if block.type == "text":
        print(block.text)
print("---")
print(f"usage: {msg.usage}")
