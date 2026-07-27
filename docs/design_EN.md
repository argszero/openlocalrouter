# OpenLocalRouter Design Document

> **Version**: 0.1.5 | **Updated**: 2026-07-24 (translated)

[中文版](design.md) | English

---

## Table of Contents

1. [Vision & Scope](#1-vision--scope)
2. [Architecture Overview](#2-architecture-overview)
3. [Data Model](#3-data-model)
4. [Multi-User & Authentication](#4-multi-user--authentication)
5. [Tauri Desktop Shell](#5-tauri-desktop-shell)
6. [API Design](#6-api-design)
7. [Request Processing Flow](#7-request-processing-flow)
8. [Protocol Translation Engine](#8-protocol-translation-engine)
9. [Frontend Design](#9-frontend-design)
10. [Relationship with CC Switch](#10-relationship-with-cc-switch)
11. [Directory Structure](#11-directory-structure)
12. [Implementation Status](#12-implementation-status)

---

## 1. Vision & Scope

### 1.1 One-Line Definition

**AI API Routing Gateway — aggregate multiple providers, distribute API keys, track usage. Runs locally, deployable on servers.**

### 1.2 Core Scenarios

| Scenario | Description |
|---|---|
| **Scenario 1: Local Use** | Aggregate multiple providers on a single machine. Use alongside CC Switch — CC Switch manages Codex integration and provider discovery, OLR handles routing and protocol translation |
| **Scenario 2: Server Deployment** | Team sharing. Admin configures providers, issues individual API keys to each member, tracks per-member usage |

### 1.3 Core Features

- **Endpoint Management**: Create multiple endpoints (e.g. `/u/alice/codex`, `/u/alice/claude`), each bound to a protocol
- **Provider Management**: Configure upstream API providers (base_url + api_key + model list)
- **Model Visibility**: Specify which models are visible to which endpoints
- **Protocol Translation**: Auto-convert between OpenAI Chat / OpenAI Responses / Anthropic Messages
- **API Key Distribution**: Generate multiple API keys per endpoint for distribution and usage attribution
- **Multi-User**: Admin + regular users, data isolated by user_id

### 1.4 Out of Scope

| Not Doing | Reason |
|---|---|
| Auto-manage Codex/Claude Code tool config files | CC Switch already does this well; complementary, not competitive |
| Codex model catalog JSON generation | Same — leave to CC Switch |
| MCP server sync / Session management / Skills repository | Out of routing scope |
| Failover / Circuit breaker | Not in V1 |
| Multi-api_type per Provider | V1: single api_type per provider; create multiple providers for multi-protocol |

---

## 2. Architecture Overview

### 2.1 Single-Port Consolidated Architecture

The current implementation uses a **single-port architecture**: admin API, proxy routes, and frontend SPA all run on **port 19528**.

```
┌──────────────────────────────────────────────────────────┐
│               http://localhost:19528                      │
│                                                          │
│  /api/admin/*     Admin API (auth required)              │
│  /u/{user}/*      Proxy routes (API Key auth)            │
│  /                 Frontend SPA                           │
│  /assets/*        Frontend static assets                 │
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
              Upstream API Provider
              (OpenAI / Anthropic / DeepSeek / ...)
```

### 2.2 Configuration

- Config file: `$CONFIG_DIR/openlocalrouter/config.json`
- Default listen: `127.0.0.1:19528`
- `AppConfig` struct: `listen_address`, `admin_port`, `data_dir`, `config_path`

---

## 3. Data Model

### 3.1 Table Design (Current)

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
    key_value       TEXT NOT NULL DEFAULT '',          -- Plaintext storage, copyable in UI
    key_hash        TEXT NOT NULL,                     -- SHA-256 retained but query uses key_value
    key_prefix      TEXT NOT NULL,                     -- First 12 characters
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

### 3.2 API Key Design

- Format: `olr_<32 bytes random base64url>`
- Storage: Plaintext in `key_value` (local desktop app; hashing adds no extra security)
- Authentication: Direct `key_value` comparison
- Admin UI shows `key_prefix`; full `key_value` is copyable

---

## 4. Multi-User & Authentication

### 4.1 Account System

| Role | Permissions |
|---|---|
| **Admin** | Manage all users, all endpoints/providers/keys |
| **Regular User** | Only manage own endpoints, providers, API keys |

Default admin `admin`, auto-created on first launch; password printed to log.

### 4.2 Two Authentication Types

| Auth Type | Use | Method |
|---|---|---|
| **Session Token** | Admin API | `POST /api/admin/login` → Bearer session token (24h expiry) |
| **Endpoint API Key** | Proxy requests | `Authorization: Bearer olr_xxx`, matched against `key_value` |

### 4.3 listen_path Naming

Format: `/u/{username}/{path_prefix}`. Backend auto-prepends username when creating endpoints; user only specifies `path_prefix` (last segment).

Natural per-user isolation:
```
/u/alice/codex    → Alice's codex endpoint
/u/bob/codex      → Bob's codex endpoint
```

---

## 5. Tauri Desktop Shell

Minimalist design: system tray only (open admin panel / quit), no Tauri window.

```
┌──────────────────────────┐
│ 🔗 Open Admin Panel       │  → system browser → http://localhost:19528
├──────────────────────────┤
│ ✕  Quit                  │  → app.exit(0)
└──────────────────────────┘
```

Auto-opens browser on startup, hides Dock icon on macOS.

---

## 6. API Design

### 6.1 Admin API (Port 19528)

All auth-required routes use `auth::require_auth` middleware.

```
# Auth (public)
POST   /api/admin/login               { username, password } → { token, user }
POST   /api/admin/logout
GET    /api/admin/status              Service status
GET    /api/admin/server-info         Server connection info
GET    /api/admin/presets             Provider preset list

# Dashboard
GET    /api/admin/dashboard           Statistics

# Endpoint Management
GET    /api/admin/endpoints
POST   /api/admin/endpoints           { name, path_prefix, protocol }
GET    /api/admin/endpoints/:id
PUT    /api/admin/endpoints/:id
DELETE /api/admin/endpoints/:id

# Endpoint API Keys
GET    /api/admin/endpoints/:id/keys
POST   /api/admin/endpoints/:id/keys  { name } → { key, ... } (returns full key)
PUT    /api/admin/endpoints/:id/keys/:kid    { name?, enabled? }
DELETE /api/admin/endpoints/:id/keys/:kid

# Provider Management
GET    /api/admin/providers
POST   /api/admin/providers           { name, base_url, api_key, api_types[] }
GET    /api/admin/providers/:id
PUT    /api/admin/providers/:id
DELETE /api/admin/providers/:id

# Models (nested under Providers)
POST   /api/admin/providers/:id/models       { slug, display_name, context_window?, visible_endpoint_ids[]? }
DELETE /api/admin/providers/:id/models/:mid
PUT    /api/admin/providers/:id/models/:mid/visibility   { endpoint_ids[] }

# User Management (Admin only)
GET    /api/admin/users
POST   /api/admin/users                { username, password, is_admin? }
PUT    /api/admin/users/:id
DELETE /api/admin/users/:id
```

### 6.2 Proxy API (Same Port, `/u/{user}/*` Prefix)

Auto-registered per endpoint:

```
GET  {listen_path}/models
GET  {listen_path}/v1/models
POST {listen_path}/chat/completions
POST {listen_path}/v1/chat/completions
POST {listen_path}/responses
POST {listen_path}/v1/responses
POST {listen_path}/v1/messages
```

`/models` no auth required; all other routes require `Authorization: Bearer <endpoint_api_key>`.

---

## 7. Request Processing Flow

```
POST /u/alice/codex/v1/responses
Authorization: Bearer olr_xxx...
Body: {"model":"gpt-5.5", "input":"hello", ...}

Step 1: Path Matching
  → Extract listen_path = "/u/alice/codex"
  → Look up endpoint → protocol: openai_responses, user_id: alice

Step 2: API Key Auth
  → Extract Bearer token
  → key_value comparison against endpoint_api_keys
  → Verify enabled=1, endpoint_id match

Step 3: Extract Model Name
  → Parse model field from body

Step 4: Find Provider
  → find_provider_by_model_slug(model_slug, endpoint_id)
  → Search across all providers (filtered by model_endpoint_visibility)

Step 5: Choose Protocol Path
  → Prefer Provider's native protocol (passthrough)
  → Then choose an implemented conversion path
  → Finally fallback to Provider api_type + protocol conversion

Step 6: Send Upstream Request
  → URL: Provider base_url + API path
  → Headers: Remove olr_ API key, inject Provider api_key
  → Body: Passthrough or transformed

Step 7: Return Response
  → Passthrough: Direct SSE stream forwarding
  → Conversion: Transform chunk-by-chunk then send
```

---

## 8. Protocol Translation Engine

### 8.1 Implementation Status

`src/router/transform.rs` — Non-streaming conversions:

| Function | Status |
|---|---|
| `anthropic_to_openai_chat` | Implemented |
| `openai_chat_to_anthropic` | Implemented |
| `openai_responses_to_openai_chat` | Implemented |
| `openai_chat_to_openai_responses` | Implemented |

`src/router/streaming.rs` — Streaming SSE conversions:

| Function | Status |
|---|---|
| `openai_sse_to_anthropic` | Implemented (with thinking/redacted_thinking support) |
| `openai_sse_to_openai_responses` | Implemented (Chat SSE → Responses SSE aggregation) |

### 8.2 Conversion Matrix (Current)

| Endpoint Protocol | Provider Protocol | Streaming | Non-streaming |
|---|---|---|---|
| openai_chat | openai_chat | Passthrough | Passthrough |
| openai_chat | anthropic_messages | openai_sse→anthropic | openai_chat→anthropic |
| openai_responses | openai_responses | Passthrough | Passthrough |
| openai_responses | openai_chat | Aggregate SSE | responses→chat |
| openai_responses | anthropic_messages | openai_sse→anthropic + expand | Supported |
| anthropic_messages | anthropic_messages | Passthrough | Passthrough |
| anthropic_messages | openai_chat | Supported | anthropic→chat |

### 8.3 Protocol Fallback

When a provider doesn't support the endpoint's declared protocol, auto-fallback to provider's `api_type`:

1. `openai_responses` endpoint → Provider only supports `openai_chat`: use responses→chat conversion
2. `openai_responses` endpoint → Provider only supports `anthropic_messages`: use anthropic conversion and expand to responses format

### 8.4 Special Handling

- **`developer` role**: OpenAI Responses `developer` role mapped to `system` (for providers like DeepSeek that don't support developer)
- **Content-Length**: Removed before forwarding (hyper handles it automatically)

### 8.5 Source Reference

Conversion code ported from CC Switch (MIT license):

| CC Switch Module | Corresponding OLR Code |
|---|---|
| `proxy/providers/transform.rs` | `src/router/transform.rs` |
| `proxy/providers/streaming.rs` | `src/router/streaming.rs` |
| `proxy/providers/transform_codex_chat.rs` | Merged into transform/streaming |
| `proxy/providers/transform_codex_anthropic.rs` | Merged into transform/streaming |

---

## 9. Frontend Design

### 9.1 Pages

| Page | Route | Description |
|---|---|---|
| Login | `/login` | |
| Dashboard | `/dashboard` | Endpoint/Provider/Model/Key/User count statistics |
| Endpoint Management | `/endpoints` | Create/edit/delete endpoints |
| Provider Management | `/providers` | Create/edit/delete providers, manage models and visibility |
| API Key Management | `/endpoints/:id/keys` | Per-endpoint key management: create/enable/disable/delete/copy |
| User Management | `/users` | Admin: create/edit/delete users |

### 9.2 Tech Stack

| Item | Choice |
|---|---|
| Framework | React 18 + TypeScript |
| Build | Vite |
| Styling | Tailwind CSS |
| Routing | React Router v6 |
| Data Fetching | @tanstack/react-query |
| Icons | lucide-react |
| Notifications | sonner |
| State | zustand (auth store) |

---

## 10. Relationship with CC Switch

**Complementary, not competitive.** CC Switch and OLR have non-overlapping functionality and are designed to work together:

- **CC Switch**: Codex integration (model catalog JSON generation), provider discovery and configuration, automatic tool config file management
- **OLR**: Multi-provider aggregation and routing, protocol translation, API key distribution and usage management

Recommended workflow: Use CC Switch to discover and configure providers → Register providers and endpoints in OLR → CC Switch points to OLR as proxy → Team members access via their own OLR keys

### Code Reuse

OLR's protocol translation engine (`transform.rs`, `streaming.rs`) was ported from CC Switch v3.16.5, which is licensed under MIT.

---

## 11. Directory Structure

```
openlocalrouter/
├── Cargo.toml                  ← Workspace root + lib crate (openlocalrouter-core)
├── Makefile
├── rustfmt.toml
├── src/                        ← Library crate
│   ├── lib.rs                  ← pub fn run_backend(), init_logging()
│   ├── main.rs                 ← CLI entry point
│   ├── config.rs               ← AppConfig
│   ├── error.rs                ← AppError
│   ├── auth.rs                 ← Password, session, API key generation
│   ├── db/
│   │   ├── mod.rs              ← Database struct (Arc<Mutex<Connection>>)
│   │   ├── schema.rs           ← SQL migration
│   │   └── dao.rs              ← All CRUD methods
│   ├── router/
│   │   ├── mod.rs              ← build_proxy_routes
│   │   ├── types.rs            ← ProxyState, ApiProtocol, RouteMatch
│   │   ├── server.rs           ← Dynamic route registration (hyper http1)
│   │   ├── handler.rs          ← handle_models / chat / responses / messages
│   │   ├── transform.rs        ← Non-streaming protocol conversion
│   │   └── streaming.rs        ← Streaming SSE conversion
│   └── admin/
│       ├── mod.rs              ← serve(), AdminState, Router assembly
│       ├── auth.rs             ← login/logout/require_auth middleware
│       ├── endpoints.rs        ← CRUD handlers
│       ├── providers.rs        ← CRUD + model/visibility handlers
│       ├── api_keys.rs         ← API Key CRUD handlers
│       ├── users.rs            ← User management handlers
│       ├── dashboard.rs        ← Statistics handler
│       └── presets.rs          ← Provider presets
├── src-tauri/                  ← Tauri binary crate
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── capabilities/
│   ├── icons/
│   └── src/main.rs             ← Tray entry point
└── frontend/
    ├── package.json
    ├── vite.config.ts
    ├── tailwind.config.js
    └── src/
        ├── App.tsx
        ├── main.tsx
        ├── lib/api.ts          ← API client
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

## 12. Implementation Status

### Completed

- [x] Project skeleton (workspace + lib + CLI + Tauri)
- [x] Database schema (all 7 tables) + migration
- [x] DAO layer (full CRUD + query methods)
- [x] User authentication (argon2id password + session token)
- [x] Admin API (endpoints, providers, models, api_keys, users, dashboard, presets, server-info)
- [x] Proxy server (dynamic route registration + hyper http1)
- [x] API Key authentication (Bearer token verification)
- [x] Model aggregation (cross-provider visibility filtering)
- [x] Protocol translation (6 directions, streaming + non-streaming)
- [x] Protocol fallback routing
- [x] `developer` → `system` role mapping
- [x] API Key plaintext storage + UI copy
- [x] Frontend SPA (all 6 pages)
- [x] Tauri tray shell
- [x] Provider presets (common provider templates)

### To Do

- [ ] Usage tracking write (usage_records table exists, handler not writing yet)
- [ ] Frontend usage charts
- [ ] Usage limits (per-key token caps)
- [ ] Docker deployment support
- [ ] OS Keychain (Provider API Key secure storage)
- [ ] Hot-reload routes (no restart needed)
- [ ] Test coverage
