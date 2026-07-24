# 详细设计

> ⚠️ **历史设计文档** — 2026-07-15 初始规划阶段编写。本文档描述的数据库改动和 API 设计已在 v0.1.x 中全部实现，具体实现可能与原文有差异。以当前源码和 [design.md](design.md) 为准。
>
> 基于 entity-model.md + ui-design.md | 2026-07-15

---

## 1. 数据库改动

### 1.1 `endpoint_api_keys` — 区分创建者和使用者

当前 `user_id` 语义模糊。拆成两个明确字段：

| 新字段 | 含义 | Key 自己用时 |
|---|---|---|
| `created_by` | 谁创建的 Key（拥有者） | = 创建者 |
| `assigned_to` | 分配给谁使用（调用者） | = 创建者（自己用） |

迁移：

```sql
ALTER TABLE endpoint_api_keys ADD COLUMN created_by TEXT NOT NULL DEFAULT '';
ALTER TABLE endpoint_api_keys ADD COLUMN assigned_to TEXT NOT NULL DEFAULT '';

-- 存量数据：user_id 当前含义是"分配给谁"，也兼当创建者（因为旧版没有分享概念）
UPDATE endpoint_api_keys SET created_by = user_id;
UPDATE endpoint_api_keys SET assigned_to = user_id;
```

`user_id` 列为 deprecated 冗余列，代码不再读写。

**最终表**：

```sql
endpoint_api_keys:
  id          TEXT PK
  endpoint_id TEXT → endpoints.id
  created_by  TEXT     -- 谁创建的（Key 的拥有者、分发者）
  assigned_to TEXT     -- 分配给谁（调用方 user_id）
  name        TEXT     -- Key 名称
  key_value   TEXT     -- 明文密钥，认证用
  key_hash    TEXT
  key_prefix  TEXT     -- 前 12 字符，UI 展示用
  enabled     INTEGER
  created_at  TEXT
  last_used_at TEXT
  user_id     TEXT     -- deprecated，留着兼容
```

### 1.2 `usage_records` — 补全归属链

```sql
ALTER TABLE usage_records ADD COLUMN provider_id   TEXT NOT NULL DEFAULT '';
ALTER TABLE usage_records ADD COLUMN provider_name TEXT NOT NULL DEFAULT '';
ALTER TABLE usage_records ADD COLUMN key_owner_id  TEXT NOT NULL DEFAULT '';
```

**最终表**：

```sql
usage_records:
  id                TEXT PK
  api_key_id        TEXT → endpoint_api_keys.id   -- 哪个 Key 发起的
  key_owner_id      TEXT                          -- Key 的拥有者（created_by）
  endpoint_id       TEXT → endpoints.id           -- 通过哪个 Endpoint
  user_id           TEXT                          -- 调用者（= Key 的 assigned_to）
  provider_id       TEXT                          -- 上游 Provider
  provider_name     TEXT                          -- Provider 名称（冗余便于查询）
  model             TEXT                          -- 请求的模型名
  input_tokens      INTEGER
  output_tokens     INTEGER
  cache_read_tokens INTEGER
  created_at        TEXT
```

数据归属链：`key_owner_id → api_key_id → assigned_to(=user_id) → endpoint_id → provider_id → model`

### 1.3 `models` — 去掉 `display_name`

`display_name` 列保留不动（避免 SQLite DROP COLUMN 复杂性），代码中不再使用。展示统一用 `slug`。

---

## 2. Handler 层改动

### 2.1 `verify_api_key` — 返回值增加 `key_owner_id`

当前返回 `ProxyAuth { user_id, api_key_id, username }`。

改动后 `user_id` 来源从 `endpoint_api_keys.user_id` 改为 `assigned_to`：

```rust
pub(crate) struct ProxyAuth {
    pub(crate) user_id: String,       // 调用者 = assigned_to
    pub(crate) username: String,
    pub(crate) api_key_id: String,
    pub(crate) key_owner_id: String,  // 新增 = created_by
}
```

### 2.2 `UsageContext` — 补全字段

```rust
pub(crate) struct UsageContext {
    pub api_key_id: String,
    pub key_owner_id: String,    // 新增：= auth.key_owner_id
    pub endpoint_id: String,
    pub user_id: String,         // 调用者：= auth.user_id
    pub provider_id: String,     // 新增：handler 中查到后填入
    pub provider_name: String,   // 新增
    pub model: String,
}
```

### 2.3 用量写入

非流式（`write_usage_record`）和流式（`UsageRecordingStream`）中两处构造 `UsageRecordRow`，补充 `key_owner_id`、`provider_id`、`provider_name`。

### 2.4 创建 Key

`POST /api/admin/endpoints/:id/keys`

```json
// Request
{ "name": "给Bob的Key", "assigned_to": "bob的用户ID" }

// 如果 assigned_to 为空，默认 = created_by（自己用）
```

Handler 中 `key_owner_id` 固定为 `auth.user_id`（当前登录用户），`assigned_to` 由请求传参。

---

## 3. API 设计

### 3.1 端点列表 `GET /api/admin/endpoints`

返回两种 Endpoint：

| 来源 | 条件 | 权限 |
|---|---|---|
| 我创建的 | `endpoints.user_id = $me` | 完全读写 |
| 分享给我的 | 存在 Key：`created_by != $me AND assigned_to = $me`（反查 Key 所属 Endpoint） | 只读 |

每个 Endpoint 返回：
```json
{
  "id": "...",
  "name": "Codex入口",
  "listen_path": "/u/alice/codex",
  "protocol": "openai_responses",
  "is_mine": true,
  "shared_by": null
}
```

SQL（两条查询 UNION）：

```sql
-- 我创建的
SELECT e.*, 1 as is_mine, NULL as shared_by
FROM endpoints e
WHERE e.user_id = $me

UNION ALL

-- 分享给我的：别人创建了 Key 分配给我，其所属 Endpoint
SELECT e.*, 0 as is_mine, e.user_id as shared_by
FROM endpoints e
JOIN endpoint_api_keys k ON k.endpoint_id = e.id
WHERE k.assigned_to = $me AND k.created_by != $me
  AND e.id NOT IN (SELECT id FROM endpoints WHERE user_id = $me)  -- 去重：不重复列出我自己的
```

### 3.2 端点详情 `GET /api/admin/endpoints/:id`

#### 场景 A：我的 Endpoint

```json
{
  "endpoint": { /* 完整信息 */ },
  "models": [ { "slug": "gpt-4o", "context_window": 128000, "visible": true } ],
  "keys": [
    { "id": "...", "name": "给Bob", "assigned_to": "bob", "assigned_username": "bob",
      "key_value": "olr_xxx...", "enabled": true, "can_manage": true }
  ],
  "can_edit": true
}
```

#### 场景 B：分享给我的 Endpoint

```json
{
  "endpoint": { "id": "...", "name": "...", "listen_path": "...", "protocol": "..." },
  "models": [ { "slug": "gpt-4o", "context_window": 128000 } ],
  "my_keys": [
    { "id": "...", "name": "Bob日常", "key_value": "olr_xxx...", "key_prefix": "olr_abc123..." }
  ],
  "shared_by": "alice",
  "can_edit": false
}
```

**区别**：
- 我的 Endpoint：可编辑，看到所有 Key，可管理 Model 可见性
- 分享给我的 Endpoint：只读，只看到分配给我的 Key，可复制 key_value

### 3.3 Key 管理 `GET/POST /api/admin/endpoints/:id/keys`

只在我的 Endpoint 中允许操作。分享端点中屏蔽 POST/PUT/DELETE。

`POST` 创建 Key 时增加 `assigned_to` 字段。

### 3.4 Model 可见性管理

只在我的 Endpoint 中可操作。

```
PUT /api/admin/endpoints/:id/models/visibility
Body: { model_ids: ["m1", "m2"] }
→ 替换该 Endpoint 的全部可见 Model 列表

GET /api/admin/endpoints/:id/models
→ 返回所有 Model 及其对该 Endpoint 的可见性
Response: [{ slug, context_window, provider_name, visible: true/false }]
```

分享端点的 Model 列表只读（`GET` 返回值但 `visible` 固定为当前已可见的）。

### 3.5 仪表板 `GET /api/admin/dashboard`（重写）

```json
// Response:
{
  "my_providers": 3,
  "my_endpoints": 5,
  "my_keys": 12,
  "keys_assigned_to_others": 7,         // 我发给别人的 Key 数
  "keys_assigned_to_me": 4,             // 别人发给我的 Key 数
  "shared_endpoints": 2,                // 别人分享给我的 Endpoint 数
  "today_my_tokens": 150000,            // 今天我自己消耗的
  "today_shared_tokens": 1200000,       // 今天别人用我的 Key 消耗的
  "recent_trend": [                     // 最近 7 天（我的 + 分享的合计）
    { "date": "2026-07-09", "tokens": 950000 },
    ...
  ]
}
```

SQL（recent_trend）：
```sql
SELECT date(created_at) as d, SUM(input_tokens + output_tokens)
FROM usage_records
WHERE user_id = $me OR key_owner_id = $me
  AND created_at >= date('now', '-7 days')
GROUP BY d ORDER BY d
```

### 3.6 我的用量 `/usage/my`

#### 3.6.1 概览 `GET /api/admin/usage/my/summary`

```json
// Query: from, to
// Response:
{ "total_tokens": ..., "total_input": ..., "total_output": ...,
  "total_requests": ..., "active_models": ..., "active_keys": ... }
```

SQL：
```sql
SELECT SUM(input_tokens + output_tokens), SUM(input_tokens), SUM(output_tokens),
       COUNT(*), COUNT(DISTINCT model), COUNT(DISTINCT api_key_id)
FROM usage_records
WHERE user_id = $me AND created_at >= $from AND created_at < date($to, '+1 day')
```

#### 3.6.2 趋势 `GET /api/admin/usage/my/trend`

```json
// Query: from, to, granularity=day|hour
// Response:
{ "points": [{ "timestamp": "2026-07-15", "input_tokens": ..., "output_tokens": ... }] }
```

SQL：同分享用量趋势，WHERE 条件为 `user_id = $me`。

#### 3.6.3 TOP 榜单 `GET /api/admin/usage/my/top`

```json
// Query: from, to, rank_by=model|provider, limit=10
// Response:
{ "items": [{ "key": "gpt-5.5", "total_tokens": ..., "count": ... }] }
```

按模型或按 Provider 聚合。

#### 3.6.4 明细 `GET /api/admin/usage/my/records`

```json
// Query: from, to, api_key_id?, model?, limit, offset
// Response: 标准分页
{
  "records": [{ "id", "api_key_id", "key_name", "provider_name", "model",
                "input_tokens", "output_tokens", "cache_read_tokens", "created_at" }],
  "total": 42
}
```

### 3.7 分享用量 `/usage/shared`

#### 3.7.1 今日概览 `GET /api/admin/usage/shared/summary`

```json
// Query: from, to
// Response:
{
  "today_tokens": 1200000,
  "yesterday_tokens": 1050000,
  "trend_pct": 14.3,       // 正数涨负数跌
  "active_keys": 8,        // 今日有调用的 Key
  "total_keys": 12,        // 我发出的全部 Key
  "active_users": 5        // 今日有调用的客户数
}
```

SQL：
```sql
-- today / yesterday：
SELECT SUM(input_tokens + output_tokens)
FROM usage_records
WHERE key_owner_id = $me AND date(created_at) = [date('now') | date('now','-1 day')]
-- 排除自己用自己 Key 的（那部分属于"我的用量"）
AND user_id != key_owner_id

-- active_keys：COUNT(DISTINCT api_key_id) WHERE date(created_at) = date('now')
-- total_keys：COUNT(*) FROM endpoint_api_keys WHERE created_by = $me
-- active_users：COUNT(DISTINCT user_id) WHERE date(created_at) = date('now')
```

#### 3.7.2 趋势 `GET /api/admin/usage/shared/trend`

```json
// Query: from, to, granularity=day|hour
// Response:
{ "points": [{ "timestamp": "2026-07-15", "input_tokens": ..., "output_tokens": ... }] }
```

SQL：
```sql
SELECT date(created_at) as ts, SUM(input_tokens), SUM(output_tokens)
FROM usage_records
WHERE key_owner_id = $me AND user_id != key_owner_id
  AND created_at >= $from AND created_at < date($to, '+1 day')
GROUP BY ts ORDER BY ts
```

#### 3.7.3 TOP 榜单 `GET /api/admin/usage/shared/top`

```json
// Query: from, to, rank_by=customer|model, limit=10
// Response:
{ "items": [{ "key": "bob", "total_tokens": 420000, "count": 35 }] }
```

SQL：
```sql
-- rank_by=customer：
SELECT u.user_id as grp, SUM(u.input_tokens + u.output_tokens), COUNT(*)
FROM usage_records u
WHERE u.key_owner_id = $me AND u.user_id != u.key_owner_id AND ...
GROUP BY u.user_id ORDER BY 2 DESC LIMIT 10

-- rank_by=model：
SELECT u.model as grp, SUM(u.input_tokens + u.output_tokens), COUNT(*)
...
GROUP BY u.model ORDER BY 2 DESC
```

#### 3.7.4 Key 健康度 `GET /api/admin/usage/shared/keys`

```json
// Query: from, to, status_filter=active|low|silent|unused
// Response:
{ "keys": [{
    "id": "...",
    "name": "Bob日常",
    "assigned_to": "bob",
    "assigned_username": "bob",
    "last_used_at": "2026-07-15T14:32:00",
    "today_tokens": 420000,
    "status": "active"
}]}
```

sql：
```sql
select k.id, k.name, k.assigned_to, k.last_used_at,
       coalesce(sum(case when date(u.created_at) = date('now')
                    then u.input_tokens + u.output_tokens else 0 end), 0) as today_tokens
from endpoint_api_keys k
left join usage_records u on u.api_key_id = k.id
where k.created_by = $me
group by k.id
order by today_tokens desc
```

状态判定（服务端计算）：
- `last_used_at` 是今天 → `active`
- 3 天内但非今天 → `low`
- 3 到 7 天内 → `silent`
- 为空或超过 7 天 → `unused`

#### 3.7.5 明细 `GET /api/admin/usage/shared/records`

```json
// Query: from, to, api_key_id?, user_id?, model?, limit, offset
// Response:
{ "records": [{
    "id": "...",
    "api_key_id": "...",
    "key_name": "Bob日常",
    "user_id": "bob",
    "username": "bob",
    "provider_name": "OpenAI",
    "model": "gpt-5.5",
    "input_tokens": 10000,
    "output_tokens": 5200,
    "cache_read_tokens": 0,
    "created_at": "2026-07-15T14:32:00"
}], "total": 42 }
```

sql：
```sql
select u.id, u.api_key_id, coalesce(k.name, '') as key_name,
       u.user_id, u.provider_name, u.model,
       u.input_tokens, u.output_tokens, u.cache_read_tokens, u.created_at
from usage_records u
left join endpoint_api_keys k on k.id = u.api_key_id
where u.key_owner_id = $me and u.user_id != u.key_owner_id and ...
order by u.created_at desc
limit $limit offset $offset
```

---

## 4. 前端页面清单

| 页面 | 路由 | 状态 | 说明 |
|---|---|---|---|
| 登录 | `/login` | 保持 | |
| 仪表板 | `/dashboard` | **重写** | 我的资产统计 + 最近趋势 |
| 端点列表 | `/endpoints` | **重写** | Tab: 我的端点 / 分享给我的 |
| 端点详情 | `/endpoints/:id` | **新增** | 我的：Model 可见性 + Key 管理 + 分配 Key<br>分享的：只读 Endpoint 信息 + 我的 Key |
| Provider 列表 | `/providers` | 保持 | |
| Provider 详情 | `/providers/:id` | 保持 | Model 管理 |
| 我的用量 | `/usage/my` | **重写** | 概览 + 趋势 + TOP + 明细 |
| 分享用量 | `/usage/shared` | **新增** | 店铺老板视角五屏 |
| 用户管理 | `/users` | 保持 | 仅 Admin |

删除的页面：

| 旧页面 | 处理 |
|---|---|
| `/endpoints/:id/keys`（ApiKeysPage） | 合并到 `/endpoints/:id` 内部（Tab 或展开） |
| `/usage`（单页） | 拆分为 `/usage/my` + `/usage/shared` |

---

## 5. 侧边导航

```
  📊 仪表板       /dashboard
  🌐 端点         /endpoints
  🖥 Provider     /providers
  📈 我的用量     /usage/my
  📤 分享用量     /usage/shared
  👤 用户         /users          ← 仅 Admin
```

---

## 6. 实现顺序

| 序 | 任务 | 依赖 |
|---|---|---|
| 1 | DB migration：endpoint_api_keys 加 created_by + assigned_to，迁移数据 | — |
| 2 | DB migration：usage_records 加 provider_id + provider_name + key_owner_id | — |
| 3 | DAO：EndpointApiKeyRow 加字段，创建/查询方法更新 | 1 |
| 4 | DAO：UsageRecordRow 加字段，insert/list/aggregate 更新 + 分享用量查询方法 | 2 |
| 5 | Handler：verify_api_key 读取 assigned_to/created_by，返回 key_owner_id | 1,3 |
| 6 | Handler：UsageContext 加字段，proxy_request 中填充所有字段 | 2,4,5 |
| 7 | Admin API：创建 Key 增加 assigned_to 参数 | 1,3 |
| 8 | Admin API：端点列表/详情区分我的和分享的 | 1,3,5 |
| 9 | Admin API：Model 可见性管理（PUT endpoint models visibility） | 3 |
| 10 | Admin API：仪表板接口重写 | 4,5 |
| 11 | Admin API：我的用量 4 个接口（summary, trend, top, records） | 4 |
| 12 | Admin API：分享用量 5 个接口（summary, trend, top, keys, records） | 4 |
| 13 | 前端：删除 models.display_name 代码 | — |
| 14 | 前端：分享用量页面（五屏） | 12 |
| 15 | 前端：我的用量页面 | 11 |
| 16 | 前端：端点列表 + 详情页（我的 + 分享 + Model 可见性） | 8,9 |
| 17 | 前端：仪表板重写 | 10 |
| 18 | 前端：导航更新、删除旧 ApiKeysPage / UsagePage | — |
