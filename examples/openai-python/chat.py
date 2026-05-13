"""
OpenAI client through Fusebox — single-file demo.

Run:
    pip install openai
    python chat.py

Tip: set FUSEBOX_TENANT to anything you like (e.g. your username); Fusebox
will scope the budget and breaker to that tenant.
"""

import os
from openai import OpenAI

FUSEBOX_URL = os.environ.get("FUSEBOX_URL", "http://localhost:8080/v1")
TENANT = os.environ.get("FUSEBOX_TENANT", "demo-user")

client = OpenAI(
    base_url=FUSEBOX_URL,
    api_key=os.environ["OPENAI_API_KEY"],
    default_headers={
        "X-Fusebox-Tenant": TENANT,
        "X-Fusebox-Project": "examples/openai-python",
    },
)

resp = client.chat.completions.create(
    model="gpt-4o-mini",
    messages=[
        {"role": "system", "content": "You are concise."},
        {"role": "user", "content": "Give me 3 reasons to like Rust."},
    ],
    max_tokens=200,
)
print(resp.choices[0].message.content)
print("---")
print(f"usage: {resp.usage}")
