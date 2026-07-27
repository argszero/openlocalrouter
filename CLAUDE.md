# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Product Positioning

OLR is **complementary to CC Switch, not a replacement**. See `docs/design.md §10`.

- **CC Switch**: Codex integration, model catalog JSON generation, provider discovery, tool config management
- **OLR**: Multi-provider aggregation/routing, protocol translation, API key distribution, usage tracking

## Build / Test / Lint Commands

```bash
# Build CLI binary
cargo build --release

# Run dev server (single port 19528)
cargo run

# Build Tauri desktop bundle (macOS .app)
cargo build -p openlocalrouter --release

# Tests (20 tests currently)
cargo test --lib
cargo test --lib -- --test-threads=1   # serial tests (SQLite)

# Lint
cargo clippy -- -D warnings
cargo fmt -- --check
cargo fmt                               # auto-format

# Full check
make check

# Frontend
cd frontend && npm install && npm run build
```

## Architecture

### Crate structure

```
openlocalrouter/              ← Cargo workspace root
├── Cargo.toml                ← [workspace] + lib crate (openlocalrouter-core) + [[bin]]
├── src/                      ← openlocalrouter-core library
│   ├── lib.rs                ← pub fn run_backend(), init_logging()
│   ├── main.rs               ← CLI binary
│   ├── config.rs             ← AppConfig (listen_address, admin_port, data_dir, config_path)
│   ├── error.rs              ← AppError enum
│   ├── auth.rs               ← Password hashing (argon2id), API key generation, session tokens
│   ├── db/                   ← SQLite schema + DAO
│   ├── router/               ← Proxy routes, handlers, protocol transforms
│   └── admin/                ← Admin REST API + SPA serving
├── src-tauri/                ← Tauri binary crate (system tray only)
│   ├── Cargo.toml            ← tauri + tray-icon + opener
│   ├── tauri.conf.json       ← windows: [], tray only
│   └── src/main.rs           ← Tray entry: open admin / quit, spawns backend
└── frontend/                 ← React SPA (Vite, built into dist/)
    └── src/
        ├── App.tsx           ← Routes: login, dashboard, endpoints, providers, keys, users
        ├── lib/api.ts        ← API client (wraps fetch with auth token)
        ├── lib/auth.ts       ← zustand auth store
        └── pages/            ← LoginPage, DashboardPage, EndpointsPage, ProvidersPage, ApiKeysPage, UsersPage
```

### Single-port design

**Everything runs on port 19528** — admin API, proxy routes, and frontend SPA. The old two-server (19527 proxy + 19528 admin) design was consolidated. `AdminState` and proxy routes are merged into one axum `Router` in `src/admin/mod.rs::serve()`.

### Database layer (`src/db/`)

- `schema.rs` — SQL migrations for 7 tables: users, sessions, endpoints, providers, models, model_endpoint_visibility, endpoint_api_keys, usage_records. Includes runtime column migration for `key_value`.
- `dao.rs` — All CRUD as `Database` methods. Key query: `find_provider_by_model_slug(slug, endpoint_id)`.
- `Database` wraps `rusqlite::Connection` in `Arc<tokio::sync::Mutex<…>>`. All access through `with_conn(f)`.

### Proxy server (`src/router/`)

- `mod.rs` — `build_proxy_routes()` creates axum Router for `/u/{user}/*` paths
- `server.rs` — Iterates enabled endpoints, registers routes. Uses `hyper::server::conn::http1` for header casing. No hot-reload yet.
- `handler.rs` — Four handlers: `handle_models` (no auth), `handle_chat_completions`, `handle_responses`, `handle_messages` (all Bearer API Key auth). Each does model lookup → protocol routing → transform → forward.
- `transform.rs` — Non-streaming: `anthropic_to_openai_chat`, `openai_chat_to_anthropic`, `openai_responses_to_openai_chat`, `openai_chat_to_openai_responses`
- `streaming.rs` — Streaming: `openai_sse_to_anthropic` (with thinking support), `openai_sse_to_openai_responses` (Chat SSE → Responses SSE)
- `types.rs` — `ApiProtocol`, `ProxyState`, `RouteMatch`, `ProxyAuth`

### Protocol routing logic

Each handler:
1. Extract model name from body → `find_provider_by_model_slug`
2. If provider api_type == endpoint protocol → **passthrough**
3. Else if transform exists → **convert** then forward
4. Else → **fallback** to provider's native api_type with conversion
5. `developer` role mapped to `system` for providers that reject it (DeepSeek)

### Admin API (`src/admin/`)

- `mod.rs` — `serve()` assembles Router: public routes, auth-gated CRUD, proxy routes, SPA static files
- `auth.rs` — login/logout, `require_auth` middleware (Bearer session token)
- `endpoints.rs` — Endpoint CRUD; `listen_path` auto-generated as `/u/{username}/{path_prefix}`
- `providers.rs` — Provider + model + visibility CRUD
- `api_keys.rs` — API Key CRUD; raw key returned on creation, stored in `key_value` (plaintext)
- `users.rs` — User CRUD (admin only)
- `dashboard.rs` — Aggregate counts
- `presets.rs` — Provider preset templates

### Frontend (`frontend/`)

React 18 + TypeScript + Tailwind + React Router v6 + TanStack Query + zustand + lucide-react + sonner.

Auth via zustand store. API client attaches Bearer session token, redirects to `/login` on 401.

## Key Design Decisions

- **API key storage**: Plaintext in `key_value` column — allows copying previously created keys; hashing is pointless for a local single-user app
- **Single api_type per provider**: Create multiple provider entries for multi-protocol providers
- **Model visibility**: `model_endpoint_visibility` join table; `/models` aggregates across all visible providers
- **Same-model conflict**: Last-insert-wins when same slug appears in multiple providers
- **listen_path**: `/u/{username}/{path_prefix}` — user inputs only `path_prefix` in UI
- **No hot-reload**: Route changes require restart
- **Config**: `$CONFIG_DIR/openlocalrouter/config.json`

## Code Style

- Rust edition 2021, MSRV 1.86.0
- `rustfmt.toml`: max_width=100, tab_spaces=4
- All DB access through `Database::with_conn()` — never lock mutex directly outside `db/mod.rs`
- Error type: `AppError` in `src/error.rs` — `?` works with `rusqlite::Error` and `std::io::Error` via `#[from]`
- Handler responses: `axum::response::IntoResponse` / `Json<serde_json::Value>`

## Reference Code

CC Switch source: `/Users/argszero/scm/github.com/farion1231/cc-switch`

Key files for protocol transforms:
- `proxy/providers/transform.rs` / `streaming.rs` — Chat ↔ Anthropic
- `proxy/providers/transform_codex_chat.rs` / `streaming_codex_chat.rs` — Chat ↔ Responses
- `proxy/providers/transform_codex_anthropic.rs` / `streaming_codex_anthropic.rs` — Responses ↔ Anthropic
- `proxy/sse.rs` — SSE parsing utilities
