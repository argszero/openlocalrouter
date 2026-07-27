<div align="center">

# OpenLocalRouter

### 本地可运行、团队可部署的 AI API 路由网关

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-orange.svg)](https://www.rust-lang.org/)

</div>

---

## 为什么需要 OpenLocalRouter

AI 时代，每个开发者都在用 AI 编程助手。但中小企业面临一个现实困境：

> **不可能也不应该给每个员工购买每个模型的 API Key。**

每个模型的 API Key 都绑定信用卡、都有月度限额、都无法按员工拆分用量。更糟的是，一旦 Key 泄漏，财务损失不可控。

**OpenLocalRouter 解决的，正是这个问题。**

## 这是什么

OpenLocalRouter 是一个 **AI API 路由网关**，在你和 AI Provider 之间架起一座桥：

```
员工 → API Key (OLR 分发) → OLR 网关 → 上游 Provider (API Key 由管理员统一管理)
                ↑                              ↑
        每人一个廉价 Key                真实 Key 对员工透明
```

核心能力：

1. **API Key 分发** — 管理员统一购买上游 API 额度，通过 OLR 给每个员工发一个中间 Key。员工的 Key 可以随时撤销、限额、追踪用量。上游 Key 永不泄漏。
2. **多 Provider 聚合路由** — OpenAI、Anthropic、DeepSeek、硅基流动、阿里百炼……不管接了多少家，对外都统一成一个端点。自动协议转换，员工不用关心底层是哪家 Provider。
3. **统一用量追踪** — 谁用了多少 Token？哪个模型最费钱？一目了然。

你可以把它跑在本机（个人使用），也可以部署在服务器上供团队使用。

## 两种使用场景

**场景 1 — 本机使用**：你有多个 API Provider（官方、中转站、自建），想把它们聚合到一起。配合 [CC Switch](https://github.com/farion1231/cc-switch) 使用效果更佳 — CC Switch 负责 Codex 集成和 Provider 发现，OLR 负责路由和转换。

**场景 2 — 团队/企业部署**：公司统一购买了一套 API 额度（多个 Provider），部署 OLR 给每个员工分配独立的 API Key。员工像用普通 API 一样使用，管理员统一管理上游 Key 和追踪用量。无需给每个员工开 Provider 账号，不用担心 Key 泄漏，财务透明可控。

## 与 CC Switch 的关系

OpenLocalRouter 和 [CC Switch](https://github.com/farion1231/cc-switch) **互补而非竞争**。

| | CC Switch | OpenLocalRouter |
|---|---|---|
| **定位** | AI 编码工具配置管家 | API 路由 + Key 分发管理 |
| **擅长** | Codex catalog 生成、Provider 发现、工具配置文件管理 | 协议转换、多 Provider 聚合、API Key 分发、用量追踪 |
| **多用户** | 单用户桌面应用 | 多用户，支持 Docker 部署 |
| **推荐配合** | 用 CC Switch 发现和配置 Provider，用 OLR 做路由和 Key 管理 |

OpenLocalRouter 的协议转换引擎借鉴了 CC Switch（MIT 协议）的实现。感谢 CC Switch 的作者 [Jason Young](https://github.com/farion1231)。

## 核心概念

### 端点（Endpoint）

对外暴露的 API 入口，自动以 `/u/{username}/{path_prefix}` 格式生成。每个端点绑定一种协议类型：

```
/u/alice/codex    → 协议: openai_responses
/u/alice/claude   → 协议: anthropic_messages
/u/alice/general  → 协议: openai_chat
```

### Provider

上游 API 提供商。配置 base URL、API Key、API 类型以及模型列表。Provider 只对创建者可见，不直接暴露给客户端。

### 模型可见性

每个模型可以指定对哪些端点可见。请求 `/u/alice/codex/models` 时返回所有对该端点可见的模型（跨 Provider 聚合）。

### 协议转换

端点协议与 Provider 协议不一致时自动转换。已实现：

| 端点协议 | Provider 协议 | 状态 |
|---|---|---|
| openai_chat | openai_chat | 透传 |
| openai_chat | anthropic_messages | 双向转换 |
| openai_responses | openai_responses | 透传 |
| openai_responses | openai_chat | 转换 + 流式聚合 |
| openai_responses | anthropic_messages | 转换 + 流式 |
| anthropic_messages | anthropic_messages | 透传 |
| anthropic_messages | openai_chat | 转换 + 流式 |

此外，当 Provider 不支持端点声明的协议时，会自动 fallback 到 Provider 原生协议。

### Provider 预设

管理界面内置了常用 Provider 的配置模板，创建 Provider 时可一键填入 base URL 和推荐模型：

| 预设 | 支持协议 |
|---|---|
| OpenAI | openai_chat, openai_responses |
| Anthropic | anthropic_messages |
| Google Gemini | openai_chat |
| Groq | openai_chat |
| 硅基流动 (SiliconFlow) | openai_chat |
| 阿里云 TokenPlan | openai_chat, openai_responses, anthropic_messages |
| 阿里云百炼 (Alibaba Bailian) | openai_chat, openai_responses, anthropic_messages |
| DeepSeek | openai_chat |
| OpenRouter | openai_chat |
| Ollama | openai_chat |

也支持"自定义"模式，手动填写任意 OpenAI/Anthropic 兼容的 API 地址。

## 快速开始

```bash
# 从源码构建
git clone https://github.com/argszero/openlocalrouter.git
cd openlocalrouter
cargo build --release
./target/release/openlocalrouter
```

启动后访问 `http://localhost:19528`，用默认管理员账号登录（密码在启动日志中）。

### 配置 AI 工具

在管理界面为端点创建 API Key，然后把工具的 API 地址和 Key 指向 OLR：

```bash
# Claude Code 示例
export ANTHROPIC_BASE_URL=http://localhost:19528/u/alice/claude/v1/messages
export ANTHROPIC_API_KEY=olr_xxxxxxxxxx
claude

# Codex 示例（配合 CC Switch 使用，CC Switch 管理 catalog，OLR 做路由）
# 在 CC Switch 中将 Provider 的 base_url 指向 OLR
```

### 发现可用模型

端点 `/models` 不要求认证，返回标准 OpenAI 格式的模型列表：

```bash
curl http://localhost:19528/u/alice/codex/v1/models
```

## 技术栈

- **后端**：Rust（axum + hyper + tokio）
- **存储**：SQLite（rusqlite）
- **前端**：React + TypeScript + Tailwind CSS + React Router + TanStack Query
- **桌面包**：Tauri 2（系统托盘）

## License

MIT
