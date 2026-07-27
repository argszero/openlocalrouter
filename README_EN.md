<div align="center">

# OpenLocalRouter

### A locally-runnable, team-deployable AI API routing gateway

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-orange.svg)](https://www.rust-lang.org/)

[中文版](README.md) | English

</div>

---

## Why OpenLocalRouter

In the AI era, every developer is using AI coding assistants. But SMBs face a real dilemma:

> **It's impossible — and unwise — to buy every model's API key for every employee.**

Each model's API key ties to a credit card, has monthly limits, and can't be split by employee usage. Worse, once a key leaks, the financial loss is uncontrollable.

**OpenLocalRouter solves exactly this problem.**

## What It Is

OpenLocalRouter is an **AI API routing gateway** that sits between you and AI providers:

```
Employee → API Key (OLR-issued) → OLR Gateway → Upstream Provider (admin-managed keys)
                ↑                              ↑
      One cheap key per person        Real keys invisible to employees
```

Core capabilities:

1. **API Key distribution** — Admins purchase upstream API quotas centrally and issue each employee an intermediary key through OLR. Employee keys can be revoked, rate-limited, and tracked at any time. Upstream keys never leak.
2. **Multi-provider aggregation & routing** — OpenAI, Anthropic, DeepSeek, SiliconFlow, Alibaba Bailian… no matter how many you connect, they appear as a single endpoint. Automatic protocol translation — employees never need to know which provider is underneath.
3. **Unified usage tracking** — Who used how many tokens? Which model costs the most? Crystal clear.

Run it on your machine (personal use) or deploy it on a server for your team.

## Two Use Cases

**Scenario 1 — Personal use**: You have multiple API providers (official, proxy, self-hosted) and want to aggregate them. Works even better with [CC Switch](https://github.com/farion1231/cc-switch) — CC Switch handles Codex integration and provider discovery, OLR handles routing and protocol translation.

**Scenario 2 — Team / Enterprise deployment**: Your company purchases API quotas centrally (across multiple providers), deploys OLR, and issues each employee an independent API key. Employees use it like any regular API. Admins manage upstream keys and track usage centrally. No need to create provider accounts for each employee. No key leakage worries. Financially transparent and controllable.

## Relationship with CC Switch

OpenLocalRouter and [CC Switch](https://github.com/farion1231/cc-switch) are **complementary, not competitive**.

| | CC Switch | OpenLocalRouter |
|---|---|---|
| **Positioning** | AI coding tool config manager | API routing + key distribution |
| **Strengths** | Codex catalog generation, provider discovery, tool config management | Protocol translation, multi-provider aggregation, API key distribution, usage tracking |
| **Multi-user** | Single-user desktop app | Multi-user, Docker-deployable |
| **Recommended combo** | Use CC Switch for provider discovery & config, OLR for routing & key management |

OpenLocalRouter's protocol translation engine draws from CC Switch's implementation (MIT license). Thanks to CC Switch's author [Jason Young](https://github.com/farion1231).

## Core Concepts

### Endpoint

Externally-facing API entry points, auto-generated in `/u/{username}/{path_prefix}` format. Each endpoint binds to one protocol type:

```
/u/alice/codex    → protocol: openai_responses
/u/alice/claude   → protocol: anthropic_messages
/u/alice/general  → protocol: openai_chat
```

### Provider

Upstream API providers. Configure base URL, API key, API type, and model list. Providers are only visible to their creator and are never directly exposed to clients.

### Model Visibility

Each model can be configured as visible to specific endpoints. A request to `/u/alice/codex/models` returns all models visible to that endpoint (aggregated across providers).

### Protocol Translation

When endpoint protocol differs from provider protocol, automatic translation kicks in. Implemented translations:

| Endpoint Protocol | Provider Protocol | Status |
|---|---|---|
| openai_chat | openai_chat | Passthrough |
| openai_chat | anthropic_messages | Bidirectional |
| openai_responses | openai_responses | Passthrough |
| openai_responses | openai_chat | Translation + streaming |
| openai_responses | anthropic_messages | Translation + streaming |
| anthropic_messages | anthropic_messages | Passthrough |
| anthropic_messages | openai_chat | Translation + streaming |

Additionally, when a provider doesn't support the endpoint's declared protocol, OLR automatically falls back to the provider's native protocol.

### Provider Presets

The admin UI includes built-in configuration templates for common providers. Create a provider with one click — base URL and recommended models are pre-filled:

| Preset | Supported Protocols |
|---|---|
| OpenAI | openai_chat, openai_responses |
| Anthropic | anthropic_messages |
| Google Gemini | openai_chat |
| Groq | openai_chat |
| SiliconFlow | openai_chat |
| Alibaba TokenPlan | openai_chat, openai_responses, anthropic_messages |
| Alibaba Bailian | openai_chat, openai_responses, anthropic_messages |
| DeepSeek | openai_chat |
| OpenRouter | openai_chat |
| Ollama | openai_chat |

A "Custom" mode is also available — manually enter any OpenAI/Anthropic-compatible API endpoint.

## Quick Start

```bash
# Build from source
git clone https://github.com/argszero/openlocalrouter.git
cd openlocalrouter
cargo build --release
./target/release/openlocalrouter
```

After startup, visit `http://localhost:19528` and log in with the default admin account (password shown in startup logs).

### Configuring AI Tools

Create an API key for your endpoint in the admin UI, then point your tool's API address and key at OLR:

```bash
# Claude Code example
export ANTHROPIC_BASE_URL=http://localhost:19528/u/alice/claude/v1/messages
export ANTHROPIC_API_KEY=olr_xxxxxxxxxx
claude

# Codex example (use with CC Switch — CC Switch manages catalog, OLR does routing)
# In CC Switch, point provider base_url to OLR
```

### Discovering Available Models

The `/models` endpoint requires no authentication and returns a standard OpenAI-format model list:

```bash
curl http://localhost:19528/u/alice/codex/v1/models
```

## Tech Stack

- **Backend**: Rust (axum + hyper + tokio)
- **Storage**: SQLite (rusqlite)
- **Frontend**: React + TypeScript + Tailwind CSS + React Router + TanStack Query
- **Desktop wrapper**: Tauri 2 (system tray)

## License

MIT
