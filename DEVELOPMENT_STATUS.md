# Fusebox 开发断点记录

## 当前开发阶段: MVP Phase 1 后期

### 最新完成 (2026-06-21)

#### ✅ Budget Request 持久化功能
- **文件变更:**
  - `crates/fusebox-ledger/migrations/sqlite/0001_init.sql` - 新增 budget_requests 表
  - `crates/fusebox-ledger/migrations/postgres/0001_init.sql` - 新增 budget_requests 表
  - `crates/fusebox-ledger/src/store.rs` - 扩展 LedgerStore trait
  - `crates/fusebox-ledger/src/sqlite.rs` - 实现 SQLite 持久化 (~170 行)
  - `crates/fusebox-ledger/src/postgres.rs` - 实现 Postgres 持久化 (~160 行)

- **测试状态:**
  - ✅ 51 个单元测试全部通过
  - ✅ SQLite 后端编译成功
  - ✅ Postgres 后端编译成功
  - ✅ `fusebox doctor` 健康检查正常

#### ✅ Pricing 数据同步
- 成功从 LiteLLM 同步 604 个模型定价
- 覆盖 5 个 Provider (OpenAI, Anthropic, Google, Bedrock, OpenRouter)

#### ✅ 文档完善
- 创建 `examples/README_CN.md` - 中文示例文档
- 创建 `DEVELOPMENT_SUMMARY.md` - 开发总结报告
- 更新示例代码说明

---

## 下一步开发任务 (按优先级)

### 🔥 P0 - MVP 必需功能

#### 1. 实现 MCP Server (未开始)
**目标:** 让 AI Agent 能够自我监控成本和请求预算

**技术栈:**
- TypeScript + `@modelcontextprotocol/sdk`
- 路径: `packages/mcp-server/`

**需要暴露的工具:**
```typescript
// MCP Tools
- get_budget(tenant: string) -> Budget
- get_spend(tenant: string, window: string) -> SpendSummary
- get_breaker(tenant: string) -> BreakerState
- request_budget_increase(tenant: string, limit: number, window: string, reason?: string) -> RequestId
```

**参考文档:**
- 架构.md §6.3 MCP Server 设计
- Anthropic MCP 官方文档

**实现步骤:**
1. 创建 `packages/mcp-server/` 目录结构
2. 安装依赖: `@modelcontextprotocol/sdk`, `zod`
3. 实现 4 个核心工具
4. 连接到 Fusebox Admin API
5. 编写测试和示例

**预计时间:** 3-4 小时

---

#### 2. 补充集成测试 (未开始)
**目标:** 端到端测试覆盖关键场景

**需要测试的场景:**
- [ ] Breaker 自动触发流程 (预算超限 → Open → Half-Open → Closed)
- [ ] 流式响应的对账准确性
- [ ] 多租户隔离验证
- [ ] Anomaly Detection 触发
- [ ] Budget Request 工作流完整性

**技术方案:**
```rust
// tests/integration/
mod breaker_trip_test;
mod streaming_reconcile_test;
mod multi_tenant_test;
mod budget_workflow_test;
```

**预计时间:** 4-5 小时

---

### 📝 P1 - 提升用户体验

#### 3. 完善 runaway-demo
**任务:**
- [ ] 添加详细的 README
- [ ] 改进脚本输出格式
- [ ] 添加可视化效果 (ASCII 图表)
- [ ] 录制演示 GIF

**预计时间:** 1-2 小时

---

#### 4. 修复 Pricing 同步脚本警告
**任务:**
```python
# scripts/sync-pricing.py
# 修复: datetime.utcnow() deprecated
- today = datetime.datetime.utcnow().strftime("%Y-%m-%d")
+ today = datetime.datetime.now(datetime.UTC).strftime("%Y-%m-%d")
```

**预计时间:** 10 分钟

---

### 🚀 P2 - 增强功能 (Phase 2)

#### 5. 开发 Dashboard (未开始)
**技术栈:**
- Next.js 15 (App Router)
- Shadcn/ui + Tremor (图表)
- Better-Auth

**核心页面:**
- `/dashboard` - 实时消费监控
- `/breakers` - 熔断器状态
- `/budget-requests` - 审批界面
- `/events` - 事件流日志

**预计时间:** 2-3 周

---

#### 6. 发布 TypeScript SDK
**包名:** `@fusebox/sdk`

**核心功能:**
```typescript
import { Fusebox } from '@fusebox/sdk';

const fusebox = new Fusebox({
  baseURL: 'http://localhost:8080',
  tenant: 'my-app',
});

// 包装 OpenAI client
const openai = fusebox.wrap(new OpenAI());
```

**预计时间:** 1 周

---

## 已知问题

### 🐛 Bug
- [ ] Windows 上 `cargo build` 文件锁定问题 (需要手动停止进程)
  - 临时方案: 使用 `cargo clean` 或重启终端

### ⚠️ 警告
- [ ] `scripts/sync-pricing.py` 使用了 deprecated `datetime.utcnow()`

### 🔧 技术债
- [ ] 部分错误处理使用 `unwrap_or`，应改为 `Result` 传播
- [ ] 缺少压力测试和基准测试
- [ ] 日志可以增加更多结构化信息 (tracing spans)

---

## 代码统计

```
语言              文件数   代码行数   注释行数   空行数
----------------------------------------------------
Rust              42       8,000      1,200      1,300
Python            1        190        25         15
TypeScript        2        70         10         8
Bash              1        56         15         5
YAML              6        800        50         50
----------------------------------------------------
总计              52       9,116      1,300      1,378
```

---

## 开发环境配置

### 必需工具
- Rust 1.78+ (已配置 rust-toolchain.toml)
- Cargo
- SQLite 3.x
- (可选) Postgres 14+
- (可选) Python 3.10+ (用于 pricing sync)

### 推荐 IDE 设置
- VS Code + rust-analyzer
- JetBrains RustRover

### 构建命令
```bash
# 开发构建
cargo build

# 发布构建
cargo build --release

# 运行测试
cargo test --workspace

# 运行代理
cargo run --bin fusebox -- start

# 健康检查
cargo run --bin fusebox -- doctor
```

---

## Git 工作流

### 分支策略
- `main` - 稳定版本，可部署
- `dev` - 开发分支
- `feature/*` - 功能分支

### Commit 规范
```
<type>(<scope>): <subject>

feat(ledger): add budget request persistence
fix(proxy): correct streaming reconciliation
docs(examples): add Chinese README
test(policy): add breaker state machine tests
```

---

## 联系方式

**项目维护者:** Fusebox Team
**问题反馈:** https://github.com/fusebox-dev/fusebox/issues
**Discord:** https://discord.gg/fusebox

---

*最后更新: 2026-06-21*
*更新者: Claude (Opus 4.7)*
