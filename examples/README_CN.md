# Fusebox 示例代码集

快速集成示例。每个示例都是独立的，可直接复制到你的项目中使用。

| 示例 | 技术栈 | 演示内容 |
|------|--------|----------|
| [`openai-python`](./openai-python/)            | Python + `openai` | 单行 `base_url` 替换；通过 header 进行租户标记 |
| [`openai-typescript`](./openai-typescript/)    | Node + `openai`   | 同上，TypeScript 版本 |
| [`anthropic-python`](./anthropic-python/)      | Python + `anthropic` | Anthropic Messages API 通过 Fusebox |
| [`runaway-demo`](./runaway-demo/)              | bash + `curl`     | 重现"Agent 进入循环"场景；观察 Fusebox 自动熔断 |

> Phase 2 将添加 Claude Code、OpenHands、LangGraph、CrewAI 等集成示例。欢迎提交 PR - 参见 [CONTRIBUTING.md](../CONTRIBUTING.md)。

## 通用设置

所有示例都假设代理在本地运行：

```bash
# 从仓库根目录
cargo run --bin fusebox -- start
# 代理现在监听 http://localhost:8080
```

然后导出你的真实上游 API 密钥（Fusebox 是透传的，我们从不存储密钥）：

```bash
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
```

## 快速开始示例

### Python (OpenAI)

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://localhost:8080/v1",
    api_key=os.environ["OPENAI_API_KEY"],
    default_headers={
        "X-Fusebox-Tenant": "my-user",
        "X-Fusebox-Project": "my-app",
    },
)

resp = client.chat.completions.create(
    model="gpt-4o-mini",
    messages=[{"role": "user", "content": "Hello!"}],
)
print(resp.choices[0].message.content)
```

### TypeScript (OpenAI)

```typescript
import OpenAI from 'openai';

const client = new OpenAI({
  baseURL: 'http://localhost:8080/v1',
  apiKey: process.env.OPENAI_API_KEY,
  defaultHeaders: {
    'X-Fusebox-Tenant': 'my-user',
    'X-Fusebox-Project': 'my-app',
  },
});

const resp = await client.chat.completions.create({
  model: 'gpt-4o-mini',
  messages: [{ role: 'user', content: 'Hello!' }],
});
console.log(resp.choices[0].message.content);
```

### Python (Anthropic)

```python
from anthropic import Anthropic

client = Anthropic(
    base_url="http://localhost:8080",
    api_key=os.environ["ANTHROPIC_API_KEY"],
    default_headers={
        "X-Fusebox-Tenant": "my-user",
    },
)

resp = client.messages.create(
    model="claude-sonnet-4-6",
    max_tokens=200,
    messages=[{"role": "user", "content": "Hello!"}],
)
print(resp.content[0].text)
```

## 核心概念

### 租户隔离

通过 `X-Fusebox-Tenant` header 标记每个请求：

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -H "X-Fusebox-Tenant: user-123" \
  -d '{"model":"gpt-4o-mini","messages":[...]}'
```

每个租户有独立的：
- 预算限制
- 熔断器状态
- 消费统计

### 预算配置

在 `fusebox.yaml` 中设置：

```yaml
policy:
  default_budget:
    limit_usd: 50.0
    window: day
  
  tenant_budgets:
    power-user:
      - limit_usd: 10.0
        window: hour
      - limit_usd: 100.0
        window: day
```

或通过 CLI 动态设置：

```bash
fusebox budget set --tenant user-123 --limit 5.0/hour
```

### 熔断器状态

查看当前状态：

```bash
fusebox breaker status --tenant user-123
```

输出：
```
tenant   : user-123
breaker  : open
reason   : budget exceeded (1h)
opened_at: 2026-06-21T10:30:00Z
```

手动重置：

```bash
fusebox breaker reset --tenant user-123
```

## 常见问题

### Q: 如何在开发环境禁用预算限制？

A: 设置一个非常高的限制：

```yaml
policy:
  default_budget:
    limit_usd: 999999.0
    window: day
```

或使用内存存储（重启后清零）：

```yaml
storage:
  type: memory
```

### Q: 如何查看实时消费？

A: 使用 `tail` 命令：

```bash
fusebox tail --tenant user-123
```

或通过 HTTP API：

```bash
curl http://localhost:8080/v1/spend?tenant=user-123
```

### Q: Fusebox 会增加多少延迟？

A: 通常 < 5ms (p99)。策略检查在内存中完成，只有记录时才写数据库。

### Q: 可以用于生产环境吗？

A: 可以！建议配置：
- 使用 Postgres 替代 SQLite
- 启用 Prometheus metrics
- 配置合理的预算和冷却时间
- 设置日志为 JSON 格式

```yaml
storage:
  type: postgres
  url: postgresql://...

telemetry:
  json_logs: true
  metrics_enabled: true
```

## 更多示例

### LangChain 集成 (即将推出)

```python
from langchain_openai import ChatOpenAI

llm = ChatOpenAI(
    base_url="http://localhost:8080/v1",
    model="gpt-4o-mini",
    default_headers={
        "X-Fusebox-Tenant": "langchain-app",
    },
)
```

### LlamaIndex 集成 (即将推出)

```python
from llama_index.llms.openai import OpenAI

llm = OpenAI(
    base_url="http://localhost:8080/v1",
    model="gpt-4o-mini",
    additional_kwargs={
        "headers": {"X-Fusebox-Tenant": "llamaindex-app"}
    },
)
```

## 贡献

欢迎提交新的示例！请参考 [CONTRIBUTING.md](../CONTRIBUTING.md)。

特别欢迎的示例：
- CrewAI 集成
- AutoGen 集成
- Microsoft Semantic Kernel
- Haystack 集成
- 其他流行的 LLM 框架

---

**遇到问题？** 在 [GitHub Issues](https://github.com/fusebox-dev/fusebox/issues) 提问或加入我们的 [Discord](https://discord.gg/fusebox)。
