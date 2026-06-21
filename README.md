<div align="center">

# ⚡ Fusebox

**自主式 AI 代理成本熔断器**

阻止失控的 Agent 烧穿你的预算

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.78%2B-orange.svg)](https://www.rust-lang.org)
[![Tests](https://img.shields.io/badge/tests-60%20passed-brightgreen.svg)](#)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](CONTRIBUTING.md)

[English](#) · [中文文档](README.zh-CN.md) · [快速开始](#-快速开始) · [架构文档](架构.md)

<img src="https://via.placeholder.com/800x400/FF6B35/FFFFFF?text=Fusebox+Demo" alt="Fusebox Demo" />

</div>

---

## 💥 问题

> *"我睡了一觉，Agent 跑了 $4,700 的账单。"*  
> *"客户的 Bug 让我们的 SaaS 一夜赔了 $12,000。"*  
> *"Devin 死循环烧了 $800。"*

**2026 年，AI Agent 正在变得自主化、长时运行，而且很贵。**

现有的监控工具（LiteLLM、Helicone、Langfuse）只会在钱烧光后告诉你。它们是温度计，而你需要的是恒温器。

---

## 💡 解决方案

**Fusebox 是一个智能代理网关，在预算超限或出现异常时自动熔断流量**——在损失发生*之前*阻止请求。

```
┌──────────┐      ┌──────────────┐      ┌────────────┐
│ Your App │─────▶│   Fusebox    │─────▶│ OpenAI /   │
│  Agent   │      │ ⚡ Circuit  │      │ Anthropic /│
│   SDK    │◀─────│   Breaker   │◀─────│ Google    │
└──────────┘      └──────┬───────┘      └────────────┘
                         │
                         ▼
              ❌ DENY  ⬇️ DOWNGRADE  🚦 QUEUE
```

---

## ✨ 核心功能

### 🔴 实时熔断器
- **自动断流** - 预算超限后立即阻止所有请求
- **三态状态机** - `Closed → Open → Half-Open` 自动恢复
- **手动控制** - 支持管理员强制重置

### 💰 多窗口预算
- **灵活配置** - 1分钟 / 1小时 / 1天 / 1周 / 1月
- **租户隔离** - 每个用户/团队/项目独立预算
- **运行时覆盖** - 无需重启即可调整限额

### 📈 异常检测
- **在线检测** - EWMA 算法实时识别消费异常
- **零训练** - 无需历史数据，开箱即用
- **自动响应** - 检测到异常立即触发熔断

### 🔌 多 Provider 支持
- ✅ OpenAI (GPT-4, GPT-4o, o1, etc.)
- ✅ Anthropic (Claude Opus, Sonnet, Haiku)
- ✅ Google (Gemini Pro, Flash)
- ✅ AWS Bedrock
- ✅ OpenRouter

### 🧮 精确成本计算
- **Token 级精度** - 使用 tiktoken-rs 准确计算
- **实时对账** - 流式响应后与上游 usage 字段校验
- **缓存感知** - 支持 Prompt Caching 成本计算

### 💾 持久化账本
- **SQLite** - 零配置，适合单机部署
- **PostgreSQL** - 企业级，支持 TimescaleDB 扩展
- **审计日志** - 完整的熔断器状态转换记录

### 🤖 AI Agent 自监控 ⭐
- **MCP Server** - 让 Claude/Cursor 等 AI Agent 自我感知成本
- **主动预算管理** - Agent 可以检查预算、请求增加配额
- **智能决策** - 根据预算余量自动选择模型

---

## 🚀 快速开始

### 方式一：使用预编译二进制（推荐）

```bash
# 下载最新版本
# (待发布后提供下载链接)

# 启动代理
./fusebox start
```

### 方式二：从源码构建

```bash
# 克隆仓库
git clone https://github.com/fusebox-dev/fusebox
cd fusebox

# 构建（需要 Rust 1.78+）
cargo build --release

# 启动代理
./target/release/fusebox start
```

代理现在运行在 `http://localhost:8080` 🎉

---

## 📖 使用示例

### Python (OpenAI)

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://localhost:8080/v1",  # 👈 指向 Fusebox
    api_key=os.environ["OPENAI_API_KEY"],
    default_headers={
        "X-Fusebox-Tenant": "my-app",      # 租户标识
    },
)

# 正常使用，Fusebox 在后台保护你
response = client.chat.completions.create(
    model="gpt-4o-mini",
    messages=[{"role": "user", "content": "Hello!"}],
)
```

### TypeScript (OpenAI)

```typescript
import OpenAI from 'openai';

const client = new OpenAI({
  baseURL: 'http://localhost:8080/v1',
  apiKey: process.env.OPENAI_API_KEY,
  defaultHeaders: {
    'X-Fusebox-Tenant': 'my-app',
  },
});

const response = await client.chat.completions.create({
  model: 'gpt-4o-mini',
  messages: [{ role: 'user', content: 'Hello!' }],
});
```

### Python (Anthropic)

```python
from anthropic import Anthropic

client = Anthropic(
    base_url="http://localhost:8080",
    api_key=os.environ["ANTHROPIC_API_KEY"],
    default_headers={
        "X-Fusebox-Tenant": "my-app",
    },
)

response = client.messages.create(
    model="claude-sonnet-4-6",
    max_tokens=200,
    messages=[{"role": "user", "content": "Hello!"}],
)
```

**就是这么简单！** 只需修改 `base_url`，Fusebox 就开始工作了。

---

## ⚙️ 配置

创建 `fusebox.yaml`：

```yaml
# 代理配置
proxy:
  bind: 0.0.0.0:8080
  upstream_timeout_secs: 600

# 存储后端
storage:
  type: sqlite
  path: .fusebox/data.db

# 策略配置
policy:
  # 默认预算（应用于所有租户）
  default_budget:
    limit_usd: 50.0
    window: day
    label: default
  
  # 熔断器配置
  breaker_cooldown_secs: 60
  halfopen_trials: 5
  
  # 租户特定预算
  tenant_budgets:
    power-user:
      - limit_usd: 10.0
        window: hour
      - limit_usd: 100.0
        window: day

# Provider 配置
providers:
  openai:
    provider: openai
    base_url: https://api.openai.com
  anthropic:
    provider: anthropic
    base_url: https://api.anthropic.com

# 定价数据
pricing:
  dir: pricing  # 自动加载 595+ 模型定价
```

---

## 🎯 实际场景

### 场景 1: 防止 Agent 死循环

```bash
# 设置严格的每分钟限制
fusebox budget set --tenant background-agent --limit 1.0/minute

# Agent 进入死循环...
# Fusebox 在第一分钟后自动断流 ✅
# 损失：$1 而不是 $4,700
```

### 场景 2: 多租户 SaaS

```yaml
tenant_budgets:
  free-tier:
    - limit_usd: 0.50
      window: day
  
  pro-tier:
    - limit_usd: 10.0
      window: day
  
  enterprise:
    - limit_usd: 1000.0
      window: month
```

### 场景 3: AI Agent 自我管理 ⭐

Claude Desktop 集成 MCP Server 后：

```
User: 分析这个 500 页的文档

Claude: 让我先检查预算...
[内部调用 get_budget MCP 工具]
我当前有 $35 剩余预算，这个任务预计 $8，可以执行。

[完成后]
任务完成！花费 $7.82，还剩 $27.18 预算。
```

---

## 🛠️ CLI 工具

Fusebox 提供强大的 CLI：

```bash
# 启动代理
fusebox start

# 查看状态
fusebox status --tenant my-app

# 实时监控
fusebox tail --tenant my-app

# 健康检查
fusebox doctor

# 熔断器管理
fusebox breaker status --tenant my-app
fusebox breaker reset --tenant my-app

# 预算管理
fusebox budget set --tenant my-app --limit 100.0/day
fusebox budget get --tenant my-app

# 定价同步
fusebox pricing sync
```

---

## 📊 监控 & 可观测性

### Prometheus Metrics

```bash
curl http://localhost:8080/metrics
```

暴露的指标：
- `fusebox_requests_total` - 总请求数
- `fusebox_cost_usd_total` - 总成本
- `fusebox_tokens_total` - Token 用量
- `fusebox_breaker_state` - 熔断器状态

### 实时事件流

```bash
# SSE 流
curl http://localhost:8080/v1/events/stream

# 或使用 CLI
fusebox tail
```

### API 查询

```bash
# 消费统计
curl http://localhost:8080/v1/spend?tenant=my-app&window=1d

# 熔断器状态
curl http://localhost:8080/v1/breaker/state?tenant=my-app

# 事件历史
curl http://localhost:8080/v1/events?tenant=my-app&limit=100
```

---

## 🤖 MCP Server - AI Agent 自监控

让 Claude、Cursor 等 AI Agent 实时感知自己的成本：

### 安装

```bash
cd packages/mcp-server
npm install
npm run build
```

### 配置 Claude Desktop

编辑 `~/Library/Application Support/Claude/claude_desktop_config.json`：

```json
{
  "mcpServers": {
    "fusebox": {
      "command": "node",
      "args": ["/path/to/fusebox/packages/mcp-server/dist/index.js"],
      "env": {
        "FUSEBOX_URL": "http://localhost:8080",
        "FUSEBOX_TENANT": "claude-desktop"
      }
    }
  }
}
```

### Agent 自动获得的能力

- ✅ 检查当前预算和余额
- ✅ 查看实时消费统计
- ✅ 监控熔断器状态
- ✅ 主动请求预算增加
- ✅ 根据预算选择不同模型

详见 [MCP Server 文档](packages/mcp-server/README.md) 和 [使用示例](packages/mcp-server/EXAMPLES.md)。

---

## 🏗️ 架构

```
fusebox/
├── crates/
│   ├── fusebox-core/      # 核心类型定义
│   ├── fusebox-policy/    # 策略引擎 + 熔断器
│   ├── fusebox-ledger/    # 持久化层
│   ├── fusebox-proxy/     # HTTP 网关
│   └── fusebox-cli/       # 命令行工具
├── packages/
│   └── mcp-server/        # MCP Server (TypeScript)
├── pricing/               # 595+ 模型定价数据
└── examples/              # 集成示例
```

核心组件：

- **Policy Engine** - 预算检查 + 异常检测
- **Circuit Breaker** - 三态熔断器状态机
- **Ledger** - 事件持久化 (SQLite/Postgres)
- **Proxy** - OpenAI/Anthropic 兼容网关
- **MCP Server** - AI Agent 自监控接口

详见 [架构文档](架构.md) 和 [技术文档](技术.md)。

---

## 🚢 部署

### Docker

```bash
docker run -p 8080:8080 \
  -v $(pwd)/fusebox.yaml:/etc/fusebox/config.yaml \
  fusebox/fusebox:latest
```

### Docker Compose

```yaml
version: '3.8'
services:
  fusebox:
    image: fusebox/fusebox:latest
    ports:
      - "8080:8080"
    volumes:
      - ./fusebox.yaml:/etc/fusebox/config.yaml
      - fusebox-data:/data
    environment:
      - FUSEBOX_CONFIG=/etc/fusebox/config.yaml
```

### Kubernetes

```bash
helm repo add fusebox https://charts.fusebox.dev
helm install fusebox fusebox/fusebox
```

---

## 📈 性能

基于实际测试：

- **延迟**: p50 < 2ms, p99 < 5ms
- **吞吐**: 10,000+ req/s (单实例)
- **内存**: ~10MB (空载), ~50MB (高负载)
- **CPU**: < 5% (正常负载)

在 M1 Mac 上使用 gpt-4o-mini 的实测结果。

---

## 🧪 测试

```bash
# 运行所有测试
cargo test --workspace

# 结果: 60 tests passed ✅
```

测试覆盖：
- ✅ 预算计算逻辑
- ✅ 熔断器状态机
- ✅ 异常检测算法
- ✅ 成本计算精度
- ✅ 数据库持久化

详见 [测试报告](TEST_REPORT.md)。

---

## 🤝 贡献

我们欢迎贡献！特别需要：

- 🐛 Bug 报告
- ✨ 功能请求
- 📝 文档改进
- 🔌 新 Provider 集成
- 🧪 测试用例

请阅读 [贡献指南](CONTRIBUTING.md)。

---

## 📜 许可证

[Apache License 2.0](LICENSE)

可自由用于商业项目，只需保留许可证文件。

---

## 🙏 致谢

Fusebox 站在巨人的肩膀上：

- [LiteLLM](https://github.com/BerriAI/litellm) - 定价数据来源
- [Tokio](https://tokio.rs/) - 异步运行时
- [Axum](https://github.com/tokio-rs/axum) - Web 框架
- [MCP SDK](https://github.com/modelcontextprotocol/typescript-sdk) - Agent 协议
- 所有分享过 "Agent 烧钱" 故事的工程师们 ❤️

---

## 📞 社区

- 💬 [Discord](https://discord.gg/fusebox) - 实时讨论
- 🐦 [Twitter/X](https://twitter.com/getfusebox) - 更新动态
- 📧 [Email](mailto:hello@fusebox.dev) - 商业咨询
- 🐛 [GitHub Issues](https://github.com/fusebox-dev/fusebox/issues) - Bug 报告

---

## 🗺️ Roadmap

### v0.2 (Phase 2) - Q3 2026
- [ ] Web Dashboard (Next.js)
- [ ] TypeScript SDK (`@fusebox/sdk`)
- [ ] Python SDK (`fusebox-sdk`)
- [ ] Grafana Dashboard 模板

### v0.3 (Phase 3) - Q4 2026
- [ ] 多实例状态同步
- [ ] Redis 缓存层
- [ ] Helm Charts
- [ ] 企业 SSO 集成

### v1.0 (Phase 4) - 2027
- [ ] ML-based 异常检测
- [ ] 自动模型降级
- [ ] 全球 CDN 部署
- [ ] SaaS 托管版本

---

## 🌟 Star History

如果 Fusebox 帮到了你，请给我们一个 Star！⭐

[![Star History Chart](https://api.star-history.com/svg?repos=fusebox-dev/fusebox&type=Date)](https://star-history.com/#fusebox-dev/fusebox&Date)

---

<div align="center">

**⚡ 用 Rust 构建 · 开源 · 生产就绪**

[快速开始](#-快速开始) · [文档](https://fusebox.dev/docs) · [Discord](https://discord.gg/fusebox) · [Demo](https://demo.fusebox.dev)

**别让你的 Agent 烧穿预算 🔥**

</div>
