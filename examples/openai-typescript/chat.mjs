// OpenAI client through Fusebox — single-file demo.
//
// Run:
//   npm install openai
//   node chat.mjs
//
// Tip: set FUSEBOX_TENANT to scope the budget / breaker to a specific user.

import OpenAI from 'openai';

const FUSEBOX_URL = process.env.FUSEBOX_URL ?? 'http://localhost:8080/v1';
const TENANT = process.env.FUSEBOX_TENANT ?? 'demo-user';

const client = new OpenAI({
  baseURL: FUSEBOX_URL,
  apiKey: process.env.OPENAI_API_KEY,
  defaultHeaders: {
    'X-Fusebox-Tenant': TENANT,
    'X-Fusebox-Project': 'examples/openai-typescript',
  },
});

const resp = await client.chat.completions.create({
  model: 'gpt-4o-mini',
  messages: [
    { role: 'system', content: 'You are concise.' },
    { role: 'user', content: 'Give me 3 reasons to like Rust.' },
  ],
  max_tokens: 200,
});

console.log(resp.choices[0].message.content);
console.log('---');
console.log('usage:', resp.usage);
