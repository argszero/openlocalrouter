# 用量追踪 — 设计文档

> **版本**: 0.1.5 | **日期**: 2026-07-24 | **状态**: 已实现

---

## 1. 目标

实现 API Key 级别的用量追踪，完成场景 2（团队服务器部署）的核心闭环：

```
管理员配置 Provider → 给成员发 API Key → 追踪每人用量 → 查看报表
```

### 1.1 范围

| 包含 | 不含（后续阶段） |
|---|---|
| 每次代理请求写入 usage_records | 用量限制/配额 |
| 管理界面按 Key/模型/时间查看用量 | 用量告警 |
| Dashboard 仪表板用量概览 | 用量导出（CSV 等） |
| 流式和非流式请求均记录 | Token 计费 |

---

## 2. 数据模型

### 2.1 现有表（已就绪）

```sql
CREATE TABLE usage_records (
    id                TEXT PRIMARY KEY,
    api_key_id        TEXT NOT NULL REFERENCES endpoint_api_keys(id),
    endpoint_id       TEXT NOT NULL REFERENCES endpoints(id),
    user_id           TEXT NOT NULL REFERENCES users(id),
    model             TEXT NOT NULL,
    input_tokens      INTEGER NOT NULL DEFAULT 0,
    output_tokens     INTEGER NOT NULL DEFAULT 0,
    cache_read_tokens INTEGER NOT NULL DEFAULT 0,
    created_at        TEXT NOT NULL DEFAULT (datetime('now'))
);
```

已有索引：
```sql
CREATE INDEX idx_usage_user ON usage_records(user_id);
```

### 2.2 新增索引

```sql
CREATE INDEX idx_usage_api_key ON usage_records(api_key_id);
CREATE INDEX idx_usage_created ON usage_records(created_at);
CREATE INDEX idx_usage_endpoint ON usage_records(endpoint_id);
```

---

## 3. 后端设计

### 3.1 整体数据流

```
客户端请求 → handler 代理转发 → 上游响应
                                   │
                    ┌──────────────┼──────────────┐
                    ▼              ▼              ▼
              非流式透传      非流式转换       流式（含转换）
                    │              │              │
                    ▼              ▼              ▼
             读取完整 body   读取完整 body   UsageRecordingStream
             提取 usage      提取 usage      包装原始 stream
                    │              │              │
                    ▼              ▼              ▼
             同步写 DB       同步写 DB       stream 结束时
                                            后台写 DB
```

### 3.2 新增文件：`src/router/usage.rs`

```rust
//! 用量提取和记录

use crate::db::{Database, dao::UsageRecordRow};
use serde_json::Value;
use std::sync::Arc;

/// 从响应 body 中提取的 token 用量
#[derive(Debug, Default)]
pub(crate) struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_read_tokens: u32,
}

/// 从各协议的非流式响应 JSON 中提取 usage
///
/// OpenAI Chat:     { "usage": { "prompt_tokens": N, "completion_tokens": N } }
/// OpenAI Responses: { "usage": { "input_tokens": N, "output_tokens": N } }
/// Anthropic:       { "usage": { "input_tokens": N, "output_tokens": N } }
pub(crate) fn extract_usage_from_body(data: &[u8]) -> TokenUsage { ... }

/// 从 OpenAI Chat SSE 的 usage chunk 提取
/// 格式: { "usage": { "prompt_tokens": N, "completion_tokens": N, ... } }
pub(crate) fn extract_usage_from_chat_sse(chunk: &Value) -> Option<TokenUsage> { ... }

/// 从 Anthropic SSE 的 message_delta 事件提取
/// 格式: { "type": "message_delta", "usage": { "input_tokens": N, "output_tokens": N } }
pub(crate) fn extract_usage_from_anthropic_sse(chunk: &Value) -> Option<TokenUsage> { ... }

/// 从 Responses SSE 的 response.completed 事件提取
/// 格式: { "type": "response.completed", "response": { "usage": { ... } } }
pub(crate) fn extract_usage_from_responses_sse(chunk: &Value) -> Option<TokenUsage> { ... }
```

### 3.3 非流式路径（同步写入）

非流式路径已缓冲完整响应 body，直接在响应返回前提取并同步写入 DB。

**修改 `handle_transformed_response`**（[handler.rs:612](/Users/argszero/scm/github.com/argszero/openlocalrouter/src/router/handler.rs:612)）：

```rust
async fn handle_transformed_response(
    resp: reqwest::Response,
    status: StatusCode,
    expected_protocol: &str,
    forwarding_protocol: &str,
    usage_ctx: &UsageContext,  // NEW
) -> axum::response::Response {
    // ... existing transform logic ...

    // 返回前记录用量
    if let Ok(usage) = extract_usage_from_body(&data) {
        let _ = db.insert_usage_record(&UsageRecordRow { ... }).await;
    }

    // ... existing response ...
}
```

**修改 `handle_passthrough_response`**（[handler.rs:671](/Users/argszero/scm/github.com/argszero/openlocalrouter/src/router/handler.rs:671)）：

```rust
async fn handle_passthrough_response(
    resp: reqwest::Response,
    status: StatusCode,
    usage_ctx: &UsageContext,  // NEW
) -> axum::response::Response {
    // ... read body ...
    // 提取 usage 写入 DB
    // ... return response ...
}
```

### 3.4 流式路径（Stream Wrapper）

流式路径的核心问题是：响应已经开始返回给客户端，不能等完整 body 再写 DB。方案是用一个 stream wrapper 在数据流经时**抓取 usage 信息**。

**关键观察**：在现有流式转换函数中，usage 信息已经存在于 SSE chunk 里：

- OpenAI Chat SSE → `"[DONE]"` 前的最后一个 chunk 包含 `"usage"` 字段
- Anthropic SSE → `message_delta` 事件携带 `usage`
- Responses SSE → `response.completed` 事件携带 `usage`

**方案**：在 `handle_streaming_response` 中，对 passthrough 流式引入一个轻量级 wrapper stream，对每条 SSE 行尝试 parse usage，stream 结束时（或收到 `[DONE]`）写入 DB。

```rust
/// 流式响应用量记录包装器
///
/// 包装一个 `ByteStream`，对于 protocol 已知的 SSE 流（OpenAI Chat / Anthropic），
/// 在数据流经时尝试从 SSE chunk 提取 usage 信息，stream 自然结束时写入 DB。
struct UsageRecordingStream<S> {
    inner: S,
    buffer: Vec<u8>,            // 累积当前 SSE line
    last_usage: Option<TokenUsage>,
    ctx: UsageContext,
    db: Arc<Database>,
}

impl<S: Stream<Item = Result<Bytes, E>>> Stream for UsageRecordingStream<S> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // 逐 chunk 透传，同时解析 SSE line 提取 usage
        match self.inner.poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                // 对 Anthropic 和 OpenAI Chat 的 SSE 跳过 data: 前缀后搜 usage
                // 对 Responses 的 SSE 搜 "usage" 字段
                self.try_extract_usage(&bytes);
                Poll::Ready(Some(Ok(bytes)))
            }
            Poll::Ready(None) => {
                // Stream 结束，后台写入 usage record
                if let Some(usage) = self.last_usage.take() {
                    let db = self.db.clone();
                    let row = self.ctx.to_usage_record(usage);
                    tokio::spawn(async move { let _ = db.insert_usage_record(&row).await; });
                }
                Poll::Ready(None)
            }
            other => other,
        }
    }
}
```

**对于已转换的流式**（`handle_streaming_response` 的 transform 分支）：转换函数返回的 stream 已经是 `impl Stream<Item = Result<Bytes, _>>`，可以用同样的 wrapper 包装。但由于每个协议对应的 SSE usage 位置不同，需要传入 protocol 参数用于匹配解析逻辑。

### 3.5 UsageContext

```rust
/// 在 proxy_request 中构建，传给所有响应处理函数
pub(crate) struct UsageContext {
    pub api_key_id: String,
    pub endpoint_id: String,
    pub user_id: String,
    pub model: String,
    pub timestamp: String,
}
```

此结构在 `proxy_request` 的 Step 2（认证通过后）即可构建。

### 3.6 DAO 层新增方法

在 `src/db/dao.rs` 新增：

```rust
/// 写入一条用量记录
pub async fn insert_usage_record(&self, row: &UsageRecordRow) -> Result<(), AppError>;

/// 查询用量记录（支持多条件过滤 + 分页）
pub async fn list_usage_records(
    &self,
    api_key_id: Option<&str>,
    endpoint_id: Option<&str>,
    user_id: Option<&str>,
    from: Option<&str>,      // ISO 8601 datetime
    to: Option<&str>,
    limit: u32,
    offset: u32,
) -> Result<Vec<UsageRecordRow>, AppError>;

/// 按维度聚合用量
pub async fn aggregate_usage(
    &self,
    user_id: Option<&str>,
    group_by: &str,           // "key" | "model" | "day"
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<UsageAggregateRow>, AppError>;
```

### 3.7 管理 API

在 `src/admin/usage.rs` 新增：

```
GET  /api/admin/usage
     查询参数: api_key_id, endpoint_id, from, to, limit, offset
     → { records: [...], total: N }

GET  /api/admin/usage/summary
     查询参数: group_by ("key" | "model" | "day"), from, to
     → { groups: [{ key, total_input_tokens, total_output_tokens, count }] }

GET  /api/admin/keys/:id/usage
     查询参数: from, to, limit, offset
     → 单个 API Key 的用量明细
```

**权限**：管理员可查看所有用户，普通用户只看到自己的数据（`WHERE user_id = ?`）。

### 3.8 Dashboard 增强

在 `src/admin/dashboard.rs` 的 `DashboardStats` 新增字段：

```rust
pub total_input_tokens: i64,
pub total_output_tokens: i64,
```

---

## 4. 前端设计

### 4.1 新增页面：用量概览

路由 `/usage`，仅管理员可见。

```
┌─────────────────────────────────────────────────────┐
│  用量概览                                            │
│                                                     │
│  [时间范围选择器]  [按 Key 分组] [按模型分组] [按天分组]  │
│                                                     │
│  ┌─────────────────────────────────────────────┐    │
│  │  用量趋势图（近 7 天/30 天折线图）              │    │
│  │  input / output tokens 两条线                │    │
│  └─────────────────────────────────────────────┘    │
│                                                     │
│  ┌─────────────────────────────────────────────┐    │
│  │  按 API Key 分布（饼图或横向柱状图）            │    │
│  └─────────────────────────────────────────────┘    │
│                                                     │
│  ┌─────────────────────────────────────────────┐    │
│  │  用量明细表（可筛选、分页）                     │    │
│  │  Key | 模型 | Input | Output | Cache | 时间   │    │
│  └─────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────┘
```

**技术选型**：考虑到简单性，V1 不做图表库依赖，用纯 HTML/CSS 的横向柱状图 + 表格展示。后续可引入 recharts。

### 4.2 API Key 页面增强

在现有 `/endpoints/:id/keys` 页面中，每个 Key 行增加用量列：

```
名称 | Key | 状态 | 本月用量 | 创建时间 | 操作
```

点击"本月用量"跳转到该 Key 的用量详情。

### 4.3 Dashboard 增强

仪表板已有 6 个统计卡片，新增第 7 个：

```
┌─────────┐
│  Token  │
│ 12,345  │  ← 今日总 token 消耗
│ 今日用量 │
└─────────┘
```

### 4.4 前端 API 新增

在 `frontend/src/lib/api.ts`：

```typescript
export interface UsageRecord {
  id: string; api_key_id: string; endpoint_id: string
  user_id: string; model: string
  input_tokens: number; output_tokens: number; cache_read_tokens: number
  created_at: string
}

export interface UsageAggregate {
  key: string; total_input_tokens: number
  total_output_tokens: number; count: number
}

export const getUsageRecords = (params: UsageQuery) =>
  request<{ records: UsageRecord[]; total: number }>('/usage', { params })

export const getUsageSummary = (params: SummaryQuery) =>
  request<{ groups: UsageAggregate[] }>('/usage/summary')

export const getKeyUsage = (keyId: string, params: UsageQuery) =>
  request<{ records: UsageRecord[]; total: number }>(`/keys/${keyId}/usage`, { params })
```

---

## 5. 性能考量

| 关注点 | 方案 |
|---|---|
| **非流式路径** | 同步写 DB（<1ms），不显著影响响应延迟 |
| **流式路径** | Stream wrapper 零拷贝透传（只读不修改 bytes），stream 结束后 `tokio::spawn` 异步写 DB |
| **DB 写入** | SQLite WAL 模式下写入不阻塞读，单条 INSERT 很快 |
| **清理** | 暂不做自动清理（SQLite 单文件够用），后续可加 TTL 配置 |

## 6. 实现步骤

### Step 1: 基础
- [ ] `src/router/usage.rs` — 新增 `TokenUsage`, `UsageContext`, `extract_usage_from_body()`
- [ ] `src/db/dao.rs` — 新增 `UsageRecordRow`, `insert_usage_record()`, `list_usage_records()`, `aggregate_usage()`
- [ ] `src/db/schema.rs` — 新增 3 个 usage 索引

### Step 2: 非流式集成
- [ ] `handler.rs` — `proxy_request` 中构建 `UsageContext`
- [ ] `handler.rs` — `handle_passthrough_response` 提取 usage 写 DB
- [ ] `handler.rs` — `handle_transformed_response` 提取 usage 写 DB

### Step 3: 流式集成
- [ ] `handler.rs` — 新增 `UsageRecordingStream` struct
- [ ] `handler.rs` — `handle_streaming_response` 用 wrapper 包装流

### Step 4: 管理 API
- [ ] `src/admin/usage.rs` — 新增用量查询/聚合 handlers
- [ ] `src/admin/mod.rs` — 注册路由
- [ ] `src/admin/dashboard.rs` — 增加 token 统计字段

### Step 5: 前端
- [ ] 新增 `frontend/src/pages/UsagePage.tsx`
- [ ] 更新 `App.tsx` 路由
- [ ] 更新 `api.ts` 接口
- [ ] 更新 `DashboardPage.tsx` 仪表板
- [ ] 更新 `ApiKeysPage.tsx` 用量列

### Step 6: 验证
- [ ] `cargo test` 通过
- [ ] `cargo clippy` 通过
- [ ] 前端 `npm run build` 通过
- [ ] curl 测试非流式请求记录
- [ ] curl 测试流式请求记录

---

## 7. 测试策略

```rust
// src/router/usage.rs tests
#[test]
fn test_extract_usage_openai_chat() { ... }
#[test]
fn test_extract_usage_openai_responses() { ... }
#[test]
fn test_extract_usage_anthropic() { ... }
#[test]
fn test_extract_usage_invalid_json() { ... }
```
