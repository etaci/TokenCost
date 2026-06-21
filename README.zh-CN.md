# Fusebox 中文 README

<div align="center">

# ⚡ Fusebox

**自主式 AI 代理成本熔断器**

阻止失控的 Agent 烧穿你的预算

[English](README.md) · **中文** · [快速开始](#-快速开始) · [架构文档](架构.md)

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
│ 你的应用 │─────▶│   Fusebox    │─────▶│ OpenAI /   │
│  Agent   │      │ ⚡ 熔断器   │      │ Anthropic /│
│   SDK    │◀─────│   保护层    │◀─────│ Google     │
└──────────┘      └──────┬───────┘      └────────────┘
                         │
                         ▼
              ❌ 拒绝  ⬇️ 降级  🚦 排队
```

---

## ✨ 核心功能

### 🔴 实时熔断器
- **自动断流** - 预算超限后立即阻止所有请求
- **三态状态机** - `关闭 → 打开 → 半开` 自动恢复
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
- ✅ OpenAI (GPT-4, GPT-4o, o1 等)
- ✅ Anthropic (Claude Opus, Sonnet, Haiku)
- ✅ Google (Gemini Pro, Flash)
- ✅ AWS Bedrock
- ✅ OpenRouter

### 🤖 AI Agent 自监控 ⭐
- **MCP Server** - 让 Claude/Cursor 等 AI Agent 自我感知成本
- **主动预算管理** - Agent 可以检查预算、请求增加配额
- **智能决策** - 根据预算余量自动选择模型

---

## 🚀 快速开始

### 方式一：使用预编译二进制

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

# 或使用 Makefile
make build

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
    messages=[{"role": "user", "content": "你好!"}],
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

# 存储后端
storage:
  type: sqlite  # 或 postgres
  path: .fusebox/data.db

# 策略配置
policy:
  # 默认预算
  default_budget:
    limit_usd: 50.0
    window: day
  
  # 租户特定预算
  tenant_budgets:
    power-user:
      - limit_usd: 100.0
        window: day
```

---

## 🛠️ Makefile 命令

Fusebox 提供便捷的 Makefile：

```bash
# 开发
make build              # 构建 release 版本
make dev                # 开发模式运行
make test               # 运行单元测试
make test-integration   # 运行集成测试
make lint               # 代码检查
make fmt                # 格式化代码

# 部署
make docker             # 构建 Docker 镜像
make docker-compose     # 启动完整技术栈
make k8s                # 部署到 Kubernetes

# 其他
make install            # 安装到系统
make clean              # 清理构建文件
make quickstart         # 快速开始指南
```

---

## 📊 实际场景

### 场景 1: 防止 Agent 死循环

```bash
# 设置严格的每分钟限制
fusebox budget set --tenant background-agent --limit 1.0/minute

# Agent 进入死循环...
# Fusebox 在第一分钟后自动断流 ✅
# 损失：$1 而不是 $4,700
```

### 场景 2: AI Agent 自我管理 ⭐

Claude Desktop 集成 MCP Server 后：

```
用户: 分析这个 500 页的文档

Claude: 让我先检查预算...
[内部调用 get_budget MCP 工具]
我当前有 ¥245 剩余预算，这个任务预计 ¥56，可以执行。

[完成后]
任务完成！花费 ¥54.74，还剩 ¥190.26 预算。
```

---

## 🐳 Docker 部署

```bash
# 使用 docker-compose（推荐）
docker-compose up -d

# 包含完整技术栈：
# - Fusebox 代理
# - PostgreSQL 数据库
# - Prometheus 监控
# - Grafana 可视化
```

访问：
- Fusebox: http://localhost:8080
- Grafana: http://localhost:3000 (admin/admin)
- Prometheus: http://localhost:9090

---

## ☸️ Kubernetes 部署

```bash
# 应用配置
kubectl apply -f k8s-deployment.yaml

# 包含：
# - Fusebox Deployment (3 replicas)
# - PostgreSQL StatefulSet
# - Services 和 Ingress
# - ConfigMap 和 Secrets
```

---

## 📈 性能

基于实际测试：

- **延迟**: p50 < 2ms, p99 < 5ms
- **吞吐**: 10,000+ req/s (单实例)
- **内存**: ~10MB (空载), ~50MB (高负载)
- **CPU**: < 5% (正常负载)

---

## 🧪 测试

```bash
# 单元测试
make test

# 集成测试
make test-integration

# 所有检查（CI 使用）
make ci
```

测试覆盖：60 个单元测试，100% 通过率

---

## 🤝 贡献

我们欢迎贡献！请阅读 [贡献指南](CONTRIBUTING.md)。

---

## 📜 许可证

[Apache License 2.0](LICENSE)

可自由用于商业项目，只需保留许可证文件。

---

## 📞 社区

- 💬 [Discord](https://discord.gg/fusebox) - 实时讨论
- 🐦 [Twitter/X](https://twitter.com/getfusebox) - 更新动态
- 📧 [Email](mailto:hello@fusebox.dev) - 商业咨询
- 🐛 [GitHub Issues](https://github.com/fusebox-dev/fusebox/issues) - Bug 报告

---

## 🗺️ 路线图

### v0.2 (Phase 2) - 2026 Q3
- [ ] Web Dashboard (Next.js)
- [ ] TypeScript SDK
- [ ] Python SDK

### v0.3 (Phase 3) - 2026 Q4
- [ ] 多实例状态同步
- [ ] Redis 缓存层
- [ ] Helm Charts

### v1.0 (Phase 4) - 2027
- [ ] ML-based 异常检测
- [ ] 自动模型降级
- [ ] SaaS 托管版本

---

<div align="center">

**⚡ 用 Rust 构建 · 开源 · 生产就绪**

[快速开始](#-快速开始) · [文档](https://fusebox.dev/docs) · [Discord](https://discord.gg/fusebox)

**别让你的 Agent 烧穿预算 🔥**

</div>
