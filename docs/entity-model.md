# 用户资产关系模型

> v0.1.5 | 2026-07-24

---

## 角色

有两种角色 — Admin 和普通用户。所有用户都可以创建 Provider、Endpoint、Key，都可以分享给他人。

| | Admin | 普通用户 |
|---|---|---|
| 用户管理（创建/编辑/启用/禁用） | ✅（可编辑任意用户） | ❌（只能修改自己的密码和用户名） |
| 查看所有 Provider | ✅ | ❌（只看自己创建的） |
| 查看所有 Endpoint | ✅ | ❌（只看自己的 + 被分享的） |
| 查看所有 API Key | ✅ | ❌（只看自己创建的 + 被分配给的） |
| 修改任意 Key | ✅ | ❌（只能修改自己创建的） |
| 查看全量用量 | ✅ | ❌（只看自己的 + 自己分享出去的） |
| 创建/管理自己的资产 | ✅ | ✅ |

Admin 拥有全局可见性和管理权限，可以查看和修改所有用户的数据。普通用户只能管理自己的资产和查看被分享给他们的资源。

**用量数据对 Admin 全量可见。** Admin 可以查看所有用户的用量记录和汇总统计。

---

## 三种资产

每个用户都可以创建：

| 资产 | 说明 |
|---|---|
| **Provider** | 上游 AI 服务商（OpenAI、Anthropic…），含 base_url、api_key、模型列表 |
| **Endpoint** | 对外暴露的调用入口（`/u/alice/codex`），绑定一种协议，指定哪些模型可见 |
| **API Key** | 密钥，关联到一个 Endpoint，可分配给其他用户 |

三者都属于创建者：

```
Alice 的 Provider ──→ Alice 的 Endpoint ──→ Alice 的 Key
```

---

## 资产可见性

### 自己的资产

创建者对自己的 Provider、Endpoint、Key 有完全的读写权限。

### 分享出去的资产

**只读 = 通过 Key 间接获得。**

```
Alice 把 Endpoint1 下的 Key1 分配给 Bob。

Bob 自动获得：
  ✅ 看到 Endpoint1（名称、URL、协议、可用模型列表）  ← 只读
  ✅ 看到 Key1（密钥值、名称）                         ← 只读

Bob 不能：
  ❌ 看到 Alice 的 Provider（base_url、api_key 对 Bob 隐藏）
  ❌ 修改 Endpoint1
  ❌ 在 Endpoint1 下创建新 Key
  ❌ 查看除自己 Key 以外的其他 Key
  ❌ 删除 Key1
```

**Key 也可以不分配给其他人（自己用）**。此时调用者 = 创建者，`assigned_to = created_by`。

**规则**：分配某个 Endpoint 下的 Key 给某人，他就自动能看到这个 Endpoint，但只能读。

---

## 用量归属

一次调用涉及两方：

```
Bob 用 Alice 创建的 Key1 调了一次 → 用量记录

  调用者视角（Bob）：我用了多少 Token
  拥有者视角（Alice）：我发出去的 Key 被用了多少 Token
```

| 维度 | 谁看 | 含义 |
|---|---|---|
| 我是调用者 | Bob | 自己消耗了多少 |
| Key 是我的 | Alice | 我发给别人的 Key 被用了多少 |
| 全局 | Admin | 所有人的用量汇总和明细 |

---

## 关系总结

```
Provider ──owns── Endpoint ──owns── Key ──分配给── 另一个 User
    │                   │              │                  │
    │                   │              │                  │
 只有创建者          只有创建者      创建者可读写       被分配者
 能看到              能管理          被分配者可只读     只读 Endpoint 和 Key
```

**没有单独的"分享 Endpoint"操作。** 给一个人分配 Endpoint 下的 Key，等于自动向他展示了这个 Endpoint（只读）。
