# OpenLocalRouter 设计文档

> **版本**: 0.2.0 | **更新**: 2026-07-14

---

## 目录

1. [愿景与范围](#1-愿景与范围)
2. [架构总览](#2-架构总览)
3. [数据模型](#3-数据模型)
4. [多用户与认证](#4-多用户与认证)
5. [Tauri 桌面外壳](#5-tauri-桌面外壳)
6. [API 设计](#6-api-设计)
7. [请求处理流程](#7-请求处理流程)
8. [协议转换引擎](#8-协议转换引擎)
9. [前端设计](#9-前端设计)
10. [与 CC Switch 的关系](#10-与-cc-switch-的关系)
11. [目录结构](#11-目录结构)
12. [实现状态](#12-实现状态)

---

## 1. 愿景与范围

### 1.1 一句话定义

**AI API 路由网关 — 聚合多 Provider、分发 API Key、追踪用量。本机可跑，服务器可部署。**

### 1.2 核心场景

| 场景 | 说明 |
|---|---|
| **场景 1：本机使用** | 多个 Provider 聚合到一台机器。配合 CC Switch 使用 — CC Switch 管理 Codex 集成和 Provider 发现，OLR 做路由和协议转换 |
| **场景 2：服务器部署** | 团队共享。管理员配置 Provider，给每个成员发独立的 API Key，追踪每人用量 |

### 1.3 核心功能

- **端点管理**：创建多个端点（如 `/u/alice/codex`、`/u/alice/claude`），每个端点绑定一种协议
- **Provider 管理**：配置上游 API 提供商（base_url + api_key + 模型列表）
- **模型可见性**：每个模型指定对哪些端点可见
- **协议转换**：OpenAI Chat / OpenAI Responses / Anthropic Messages 之间自动转换
- **API Key 分发**：为每个端点生成多个 API Key，用于分发和用量归属
- **多用户**：管理员 + 普通用户，数据按 user_id 隔离

### 1.4 不做什么

| 不做 | 原因 |
|---|---|
| 自动管理 Codex/Claude Code 等工具的配置文件 | CC Switch 已经做得很好，互补而非竞争 |
| Codex model catalog JSON 生成 | 同上，留给 CC Switch |
| MCP 服务器同步 / Session 管理 / Skills 仓库 | 不属于路由范围 |
| 故障转移 / 熔断器 | V1 不做 |
| Provider 多 api_type 支持 | V1 单 api_type，需要多协议时创建多个 Provider |

---

## 2. 架构总览

### 2.1 单端口合并架构

当前实现为**单端口架构**：管理 API、代理路由、前端 SPA 全部运行在 **端口 19528**。

```
┌──────────────────────────────────────────────────────────┐
│               http://localhost:19528                      │
│                                                          │
│  /api/admin/*    管理 API（auth required）               │
│  /u/{user}/*     代理路由（API Key 认证）                 │
│  /                前端 SPA                                │
│  /assets/*       前端静态资源                             │
└────────────────────┬──────────────────────────────────────┘
                     │
              ┌──────▼──────┐
              │   SQLite    │
              │  users      │
              │  sessions   │
              │  endpoints  │
              │  providers  │
              │  models     │
              │  endpoint_api_keys │
              │  usage_records     │
              └──────┬──────┘
                     │
                     ▼
              上游 API Provider
              (OpenAI / Anthropic / DeepSeek / ...)
```

### 2.2 配置

- 配置文件：`$CONFIG_DIR/openlocalrouter/config.json`
- 默认监听：`127.0.0.1:19528`
- `AppConfig` 结构：`listen_address`, `admin_port`, `data_dir`, `config_path`

---

## 3. 数据模型

### 3.1 表设计（当前实际）

```sql
CREATE TABLE users (
    id              TEXT PRIMARY KEY,
    username        TEXT NOT NULL UNIQUE,
    password_hash   TEXT NOT NULL,                    -- argon2id
    is_admin        INTEGER NOT NULL DEFAULT 0,
    enabled         INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE sessions (
    token           TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at      TEXT NOT NULL
);

CREATE TABLE endpoints (
    id          TEXT PRIMARY KEY,
    user_id     TEXT NOT NULL REFERENCES users(id),
    name        TEXT NOT NULL,
    listen_path TEXT NOT NULL UNIQUE,
    protocol    TEXT NOT NULL CHECK(protocol IN ('openai_chat','openai_responses','anthropic_messages')),
    enabled     INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE providers (
    id           TEXT PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users(id),
    name         TEXT NOT NULL,
    base_url     TEXT NOT NULL,
    api_key      TEXT NOT NULL DEFAULT '',
    api_type     TEXT NOT NULL DEFAULT 'openai_chat',
    enabled      INTEGER NOT NULL DEFAULT 1,
    extra_config TEXT NOT NULL DEFAULT '{}',
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE models (
    id              TEXT PRIMARY KEY,
    provider_id     TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    slug            TEXT NOT NULL,
    display_name    TEXT NOT NULL,
    context_window  INTEGER NOT NULL DEFAULT 128000,
    extra_config    TEXT NOT NULL DEFAULT '{}',
    UNIQUE(provider_id, slug)
);

CREATE TABLE model_endpoint_visibility (
    model_id    TEXT NOT NULL REFERENCES models(id) ON DELETE CASCADE,
    endpoint_id TEXT NOT NULL REFERENCES endpoints(id) ON DELETE CASCADE,
    PRIMARY KEY (model_id, endpoint_id)
);

CREATE TABLE endpoint_api_keys (
    id              TEXT PRIMARY KEY,
    endpoint_id     TEXT NOT NULL REFERENCES endpoints(id) ON DELETE CASCADE,
    user_id         TEXT NOT NULL REFERENCES users(id),
    name            TEXT NOT NULL DEFAULT '',
    key_value       TEXT NOT NULL DEFAULT '',          -- 明文存储，UI 可复制
    key_hash        TEXT NOT NULL,                     -- SHA-256 保留但查询用 key_value
    key_prefix      TEXT NOT NULL,                     -- 前 12 字符
    enabled         INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    last_used_at    TEXT
);

CREATE TABLE usage_records (
    id              TEXT PRIMARY KEY,
    api_key_id      TEXT NOT NULL REFERENCES endpoint_api_keys(id),
    endpoint_id     TEXT NOT NULL REFERENCES endpoints(id),
    user_id         TEXT NOT NULL REFERENCES users(id),
    model           TEXT NOT NULL,
    input_tokens    INTEGER NOT NULL DEFAULT 0,
    output_tokens   INTEGER NOT NULL DEFAULT 0,
    cache_read_tokens INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### 3.2 API Key 设计

- 格式：`olr_<32 字节随机 base64url>`
- 存储：明文存 `key_value`（本地桌面应用，哈希无额外安全性）
- 认证方式：直接 `key_value` 比对
- 管理界面显示 key_prefix，可复制完整 key_value

---

## 4. 多用户与认证

### 4.1 账号体系

| 角色 | 权限 |
|---|---|
| **管理员** | 管理所有用户、所有端点/Provider/Key |
| **普通用户** | 只能管理自己的端点、Provider、API Key |

默认管理员 `admin`，首次启动自动创建，密码打印到日志。

### 4.2 两种认证

| 认证类型 | 用途 | 方式 |
|---|---|---|
| **Session Token** | 管理 API | `POST /api/admin/login` → Bearer session token（24h 过期） |
| **Endpoint API Key** | 代理请求 | `Authorization: Bearer olr_xxx`，查 `key_value` 比对 |

### 4.3 listen_path 命名

格式：`/u/{username}/{path_prefix}`。创建端点时后端自动拼接，用户只需指定 path_prefix（最后一段）。

不同用户自然隔离：
```
/u/alice/codex    → Alice 的 codex 端点
/u/bob/codex      → Bob 的 codex 端点
```

---

## 5. Tauri 桌面外壳

极简设计：仅系统托盘（打开管理界面 / 退出），不做 Tauri 窗口。

```
┌──────────────────────────┐
│ 🔗 打开管理界面           │  → 系统浏览器 → http://localhost:19528
├──────────────────────────┤
│ ✕  退出                  │  → app.exit(0)
└──────────────────────────┘
```

启动时自动打开浏览器，macOS 隐藏 Dock 图标。

---

## 6. API 设计

### 6.1 管理 API（端口 19528）

所有需认证路由通过 `auth::require_auth` 中间件。

```
# 认证（公开）
POST   /api/admin/login               { username, password } → { token, user }
POST   /api/admin/logout
GET    /api/admin/status              服务状态
GET    /api/admin/server-info         服务器连接信息
GET    /api/admin/presets             Provider 预设列表

# Dashboard
GET    /api/admin/dashboard           统计信息

# 端点管理
GET    /api/admin/endpoints
POST   /api/admin/endpoints           { name, path_prefix, protocol }
GET    /api/admin/endpoints/:id
PUT    /api/admin/endpoints/:id
DELETE /api/admin/endpoints/:id

# 端点 API Key
GET    /api/admin/endpoints/:id/keys
POST   /api/admin/endpoints/:id/keys  { name } → { key, ... } （返回完整 key）
PUT    /api/admin/endpoints/:id/keys/:kid    { name?, enabled? }
DELETE /api/admin/endpoints/:id/keys/:kid

# Provider 管理
GET    /api/admin/providers
POST   /api/admin/providers           { name, base_url, api_key, api_types[] }
GET    /api/admin/providers/:id
PUT    /api/admin/providers/:id
DELETE /api/admin/providers/:id

# 模型（挂载在 Provider 下）
POST   /api/admin/providers/:id/models       { slug, display_name, context_window?, visible_endpoint_ids[]? }
DELETE /api/admin/providers/:id/models/:mid
PUT    /api/admin/providers/:id/models/:mid/visibility   { endpoint_ids[] }

# 用户管理（管理员）
GET    /api/admin/users
POST   /api/admin/users                { username, password, is_admin? }
PUT    /api/admin/users/:id
DELETE /api/admin/users/:id
```

### 6.2 代理 API（同端口，/u/{user}/* 前缀）

每个端点自动注册：

```
GET  {listen_path}/models
GET  {listen_path}/v1/models
POST {listen_path}/chat/completions
POST {listen_path}/v1/chat/completions
POST {listen_path}/responses
POST {listen_path}/v1/responses
POST {listen_path}/v1/messages
```

`/models` 无需认证，其他路由需 `Authorization: Bearer <endpoint_api_key>`。

---

## 7. 请求处理流程

```
POST /u/alice/codex/v1/responses
Authorization: Bearer olr_xxx...
Body: {"model":"gpt-5.5", "input":"hello", ...}

Step 1: 路径匹配
  → 提取 listen_path = "/u/alice/codex"
  → 查 endpoint → 协议: openai_responses, user_id: alice

Step 2: API Key 认证
  → 提取 Bearer token
  → key_value 比对 endpoint_api_keys
  → 校验 enabled=1, endpoint_id 匹配

Step 3: 提取模型名
  → 从 body 解析 model 字段

Step 4: 查找 Provider
  → find_provider_by_model_slug(model_slug, endpoint_id)
  → 跨所有 provider 搜索（通过 model_endpoint_visibility 过滤）

Step 5: 选择协议路径
  → 优先 Provider 原生协议透传
  → 其次选择已实现的转换路径
  → 最后 fallback 到 Provider api_type + 协议转换

Step 6: 发送上游请求
  → URL: Provider base_url + API 路径
  → Headers: 移除 olr_ API Key，注入 Provider api_key
  → Body: 透传或经 transform 转换

Step 7: 返回响应
  → 透传: 直接流式转发 SSE
  → 转换: 逐 chunk 转协议后发送
```

---

## 8. 协议转换引擎

### 8.1 实现状态

`src/router/transform.rs` — 非流式转换：

| 函数 | 状态 |
|---|---|
| `anthropic_to_openai_chat` | 已实现 |
| `openai_chat_to_anthropic` | 已实现 |
| `openai_responses_to_openai_chat` | 已实现 |
| `openai_chat_to_openai_responses` | 已实现 |

`src/router/streaming.rs` — 流式 SSE 转换：

| 函数 | 状态 |
|---|---|
| `openai_sse_to_anthropic` | 已实现（含 thinking/redacted_thinking 支持） |
| `openai_sse_to_openai_responses` | 已实现（Chat SSE → Responses SSE 聚合） |

### 8.2 转换矩阵（当前）

| 端点协议 | Provider 协议 | 流式 | 非流式 |
|---|---|---|---|
| openai_chat | openai_chat | 透传 | 透传 |
| openai_chat | anthropic_messages | openai_sse→anthropic | openai_chat→anthropic |
| openai_responses | openai_responses | 透传 | 透传 |
| openai_responses | openai_chat | 聚合 SSE | responses→chat |
| openai_responses | anthropic_messages | openai_sse→anthropic + 展开 | 支持 |
| anthropic_messages | anthropic_messages | 透传 | 透传 |
| anthropic_messages | openai_chat | 支持 | anthropic→chat |

### 8.3 协议 Fallback

当 Provider 不支持端点声明的协议时，自动 fallback 到 Provider 的 `api_type`：

1. `openai_responses` 端点 → Provider 仅支持 `openai_chat`：使用 responses→chat 转换
2. `openai_responses` 端点 → Provider 仅支持 `anthropic_messages`：使用 anthropic 转换并展开为 responses 格式

### 8.4 特殊处理

- **`developer` role**：OpenAI Responses 的 `developer` role 映射为 `system`（DeepSeek 等不支持 developer 的 Provider）
- **Content-Length**：转发前移除（hyper 自动处理）

### 8.5 源码参考

转换代码从 CC Switch（MIT 协议）移植：

| CC Switch 模块 | 对应 OLR 代码 |
|---|---|
| `proxy/providers/transform.rs` | `src/router/transform.rs` |
| `proxy/providers/streaming.rs` | `src/router/streaming.rs` |
| `proxy/providers/transform_codex_chat.rs` | 合并到 transform/streaming |
| `proxy/providers/transform_codex_anthropic.rs` | 合并到 transform/streaming |

---

## 9. 前端设计

### 9.1 页面

| 页面 | 路由 | 说明 |
|---|---|---|
| 登录 | `/login` | |
| 仪表板 | `/dashboard` | 端点/Provider/模型/Key/用户数量统计 |
| 端点管理 | `/endpoints` | 创建/编辑/删除端点 |
| Provider 管理 | `/providers` | 创建/编辑/删除 Provider，管理模型和可见性 |
| API Key 管理 | `/endpoints/:id/keys` | 按端点管理 API Key，创建/启用/禁用/删除/复制 |
| 用户管理 | `/users` | 管理员：创建/编辑/删除用户 |

### 9.2 技术栈

| 项 | 选择 |
|---|---|
| 框架 | React 18 + TypeScript |
| 构建 | Vite |
| 样式 | Tailwind CSS |
| 路由 | React Router v6 |
| 数据获取 | @tanstack/react-query |
| 图标 | lucide-react |
| 通知 | sonner |
| 状态 | zustand（auth store） |

---

## 10. 与 CC Switch 的关系

**互补而非竞争。** CC Switch 和 OLR 功能不重叠，配合使用：

- **CC Switch**：Codex 集成（model catalog JSON 生成）、Provider 发现和配置、工具配置文件自动管理
- **OLR**：多 Provider 聚合路由、协议转换、API Key 分发和用量管理

推荐工作流：用 CC Switch 发现和配置 Provider → 在 OLR 中注册 Provider 和端点 → CC Switch 将 OLR 作为代理指向 → 团队成员用各自的 OLR Key 访问

### 代码复用

OLR 的协议转换引擎（`transform.rs`、`streaming.rs`）从 CC Switch v3.16.5 移植，CC Switch 以 MIT 协议授权。

---

## 11. 目录结构

```
openlocalrouter/
├── Cargo.toml                  ← workspace 根 + lib crate (openlocalrouter-core)
├── Makefile
├── rustfmt.toml
├── src/                        ← 库 crate
│   ├── lib.rs                  ← pub fn run_backend(), init_logging()
│   ├── main.rs                 ← CLI 入口
│   ├── config.rs               ← AppConfig
│   ├── error.rs                ← AppError
│   ├── auth.rs                 ← 密码、session、API Key 生成
│   ├── db/
│   │   ├── mod.rs              ← Database struct (Arc<Mutex<Connection>>)
│   │   ├── schema.rs           ← SQL migration
│   │   └── dao.rs              ← 所有 CRUD 方法
│   ├── router/
│   │   ├── mod.rs              ← build_proxy_routes
│   │   ├── types.rs            ← ProxyState, ApiProtocol, RouteMatch
│   │   ├── server.rs           ← 动态路由注册（hyper http1）
│   │   ├── handler.rs          ← handle_models / chat / responses / messages
│   │   ├── transform.rs        ← 非流式协议转换
│   │   └── streaming.rs        ← 流式 SSE 转换
│   └── admin/
│       ├── mod.rs              ← serve(), AdminState, Router 组装
│       ├── auth.rs             ← login/logout/require_auth middleware
│       ├── endpoints.rs        ← CRUD handlers
│       ├── providers.rs        ← CRUD + model/visibility handlers
│       ├── api_keys.rs         ← API Key CRUD handlers
│       ├── users.rs            ← 用户管理 handlers
│       ├── dashboard.rs        ← 统计 handler
│       └── presets.rs          ← Provider 预设
├── src-tauri/                  ← Tauri 二进制 crate
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── capabilities/
│   ├── icons/
│   └── src/main.rs             ← 托盘入口
└── frontend/
    ├── package.json
    ├── vite.config.ts
    ├── tailwind.config.js
    └── src/
        ├── App.tsx
        ├── main.tsx
        ├── lib/api.ts          ← API 客户端
        ├── lib/auth.ts         ← zustand auth store
        ├── components/Layout.tsx
        └── pages/
            ├── LoginPage.tsx
            ├── DashboardPage.tsx
            ├── EndpointsPage.tsx
            ├── ProvidersPage.tsx
            ├── ApiKeysPage.tsx
            └── UsersPage.tsx
```

---

## 12. 实现状态

### 已完成

- [x] 项目骨架（workspace + lib + CLI + Tauri）
- [x] 数据库 schema（全部 7 张表）+ migration
- [x] DAO 层（完整 CRUD + 查询方法）
- [x] 用户认证（argon2id 密码 + session token）
- [x] 管理 API（endpoints, providers, models, api_keys, users, dashboard, presets, server-info）
- [x] 代理服务器（动态路由注册 + hyper http1）
- [x] API Key 认证（Bearer token 验证）
- [x] 模型聚合（跨 Provider 可见性过滤）
- [x] 协议转换（6 个方向，流式 + 非流式）
- [x] 协议 fallback 路由
- [x] `developer` → `system` role 映射
- [x] API Key 明文存储 + UI 复制
- [x] 前端 SPA（全部 6 个页面）
- [x] Tauri 托盘外壳
- [x] Provider 预设（常见 Provider 模板）

### 待开发

- [ ] 用量统计写入（usage_records 表已有，handler 未写入）
- [ ] 前端用量图表
- [ ] 用量限制（按 Key 设置 token 上限）
- [ ] Docker 部署支持
- [ ] 操作系统 Keychain（Provider API Key 安全存储）
- [ ] 热重载路由（无需重启即可生效）
- [ ] 测试覆盖
