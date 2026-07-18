# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

### Added
- Full admin REST API (endpoints, providers, models, API keys, users, dashboard, presets, server-info)
- HTTP proxy server with dynamic endpoint routing (hyper http1)
- API Key authentication for proxy requests (Bearer token, plaintext comparison)
- Multi-user support with user_id data isolation
- Protocol conversion engine: bidirectional transforms for OpenAI Chat, OpenAI Responses, and Anthropic Messages
- Streaming SSE protocol conversion (OpenAI Chat SSE → Anthropic SSE, Chat SSE → Responses SSE)
- Protocol fallback routing when provider doesn't support endpoint's declared protocol
- `developer` role → `system` mapping for providers that don't support the developer role (e.g. DeepSeek)
- API Key copy-to-clipboard in management UI
- Provider presets for common API providers
- Server info endpoint for connection details
- Frontend SPA (React + TypeScript + Tailwind CSS): login, dashboard, endpoints, providers, API keys, users
- Tauri 2 desktop shell with system tray
- Session-based admin authentication (argon2id password hashing)
- `usage_records` table for future per-key usage tracking

### Changed
- Admin API and proxy merged to single port (19528) — was previously two-server architecture
- API Keys stored in plaintext (`key_value` column) instead of hash-only for local app usability
- Endpoint `listen_path` auto-generated from `path_prefix` + `username` pattern

## Fixed
- `Content-Length` header removed when forwarding requests (hyper handles it)
