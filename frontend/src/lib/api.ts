import { useAuth } from './auth'

const BASE = '/api/admin'

async function request<T>(path: string, options?: RequestInit): Promise<T> {
  const token = useAuth.getState().token
  const res = await fetch(`${BASE}${path}`, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...options?.headers,
    },
  })
  if (res.status === 401) {
    useAuth.getState().logout()
    window.location.href = '/login'
    throw new Error('Unauthorized')
  }
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: { message: res.statusText } }))
    throw new Error(err.error?.message || res.statusText)
  }
  return res.json()
}

// ── Auth ────────────────────────────────────────────
export const login = (username: string, password: string) =>
  request<{ token: string; user: { id: string; username: string; is_admin: boolean } }>('/login', {
    method: 'POST',
    body: JSON.stringify({ username, password }),
  })

// ── Status ──────────────────────────────────────────
export const getStatus = () =>
  request<{ status: string; timestamp: string; version: string }>('/status')

// ── Dashboard ───────────────────────────────────────
export const getDashboard = () =>
  request<{
    my_providers: number; my_endpoints: number; my_keys: number
    keys_assigned_to_others: number; keys_assigned_to_me: number
    shared_endpoints: number; today_my_tokens: number; today_shared_tokens: number
    recent_trend: { date: string; tokens: number }[]
  }>('/dashboard')

// ── Endpoints ───────────────────────────────────────
export interface Endpoint {
  id: string; user_id: string; name: string; listen_path: string; protocol: string
  enabled: boolean; created_at: string; updated_at: string
  is_mine?: boolean; shared_by?: string | null
}
export interface ServerInfo {
  listen_address: string; proxy_port: number; proxy_base_url: string
}
export const getServerInfo = () => request<ServerInfo>('/server-info')
export const getEndpoints = () => request<Endpoint[]>('/endpoints')
export const createEndpoint = (data: { name: string; path_prefix: string; protocol: string }) =>
  request<Endpoint>('/endpoints', { method: 'POST', body: JSON.stringify(data) })
export const updateEndpoint = (id: string, data: Partial<Endpoint> & { path_prefix?: string }) =>
  request<Endpoint>(`/endpoints/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const deleteEndpoint = (id: string) =>
  request(`/endpoints/${id}`, { method: 'DELETE' })

// ── Providers ────────────────────────────────────────
export interface Provider {
  id: string; name: string; base_url: string; api_key: string
  api_type: string; api_types: string[]
  api_urls?: Record<string, string>
  enabled: boolean; extra_config: string; created_at: string; updated_at: string
}
export interface Model {
  id: string; provider_id: string; slug: string; display_name: string
  context_window: number; extra_config: string
  visible_endpoint_ids?: string[]
}
export const getProviders = () => request<Provider[]>('/providers')
export const createProvider = (data: { name: string; base_url: string; api_key: string; api_types: string[] }) =>
  request<Provider>('/providers', { method: 'POST', body: JSON.stringify(data) })
export const updateProvider = (id: string, data: Partial<Provider>) =>
  request<Provider>(`/providers/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const deleteProvider = (id: string) =>
  request(`/providers/${id}`, { method: 'DELETE' })
export const createModel = (providerId: string, data: { slug: string; display_name: string; context_window?: number; model_slug?: string; visible_endpoint_ids?: string[] }) =>
  request<Model>(`/providers/${providerId}/models`, { method: 'POST', body: JSON.stringify(data) })
export const deleteModel = (providerId: string, modelId: string) =>
  request(`/providers/${providerId}/models/${modelId}`, { method: 'DELETE' })
export const setModelVisibility = (providerId: string, modelId: string, endpointIds: string[]) =>
  request(`/providers/${providerId}/models/${modelId}/visibility`, { method: 'PUT', body: JSON.stringify({ endpoint_ids: endpointIds }) })

// ── API Keys ─────────────────────────────────────────
export interface ApiKey {
  id: string; endpoint_id: string; name: string; key_value: string; key_prefix: string
  created_by: string; assigned_to: string
  enabled: boolean; created_at: string; last_used_at: string | null
}
export const getApiKeys = (endpointId: string) => request<ApiKey[]>(`/endpoints/${endpointId}/keys`)
export const createApiKey = (endpointId: string, name: string, assigned_to?: string) =>
  request<{ key: string } & ApiKey>(`/endpoints/${endpointId}/keys`, {
    method: 'POST',
    body: JSON.stringify({ name, assigned_to }),
  })
export const updateApiKey = (endpointId: string, keyId: string, data: { name?: string; enabled?: boolean; assigned_to?: string }) =>
  request<ApiKey>(`/endpoints/${endpointId}/keys/${keyId}`, { method: 'PUT', body: JSON.stringify(data) })
export const deleteApiKey = (endpointId: string, keyId: string) =>
  request(`/endpoints/${endpointId}/keys/${keyId}`, { method: 'DELETE' })

// ── Users ────────────────────────────────────────────
export interface User {
  id: string; username: string; is_admin: boolean; enabled: boolean
  created_at: string; updated_at: string
}
export const getUsers = () => request<User[]>('/users')
export const createUser = (data: { username: string; password: string; is_admin?: boolean }) =>
  request<User>('/users', { method: 'POST', body: JSON.stringify(data) })
export const updateUser = (id: string, data: { username?: string; password?: string; is_admin?: boolean; enabled?: boolean }) =>
  request<User>(`/users/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const deleteUser = (id: string) =>
  request(`/users/${id}`, { method: 'DELETE' })

// ── Presets ──────────────────────────────────────────
export interface ProviderPreset {
  name: string; base_url: string; api_types: string[]; icon: string
  api_urls?: Record<string, string>
  category: string; description: string
  models_hint?: { slug: string; display_name: string; context_window?: number }[]
}
export const getPresets = () => request<ProviderPreset[]>('/presets')

// ── My Usage ─────────────────────────────────────────
export interface UsageRecord {
  id: string; api_key_id: string; key_owner_id: string
  endpoint_id: string; user_id: string
  provider_id: string; provider_name: string; model: string
  input_tokens: number; output_tokens: number; cache_read_tokens: number
  created_at: string
}
export interface UsageAggregate {
  key: string; key_name?: string
  total_input_tokens: number; total_output_tokens: number; count: number
}
export interface TimeSeriesPoint {
  timestamp: string; input_tokens: number; output_tokens: number; count: number
}
export interface TimeSeriesBreakdown {
  timestamp: string; group_key: string; total_tokens: number; count: number
}
export interface SharedSummary {
  today_tokens: number; yesterday_tokens: number; trend_pct: number
  active_keys: number; total_keys: number; active_users: number
}
export const getMyUsageSummary = (groupBy: string, from?: string, to?: string) => {
  const qs = new URLSearchParams({ group_by: groupBy })
  if (from) qs.set('from', from)
  if (to) qs.set('to', to)
  return request<{ groups: UsageAggregate[] }>(`/usage/my/summary?${qs}`)
}
export const getMyUsageTrend = (params: { granularity?: string; from?: string; to?: string }) => {
  const qs = new URLSearchParams()
  if (params.granularity) qs.set('granularity', params.granularity)
  if (params.from) qs.set('from', params.from)
  if (params.to) qs.set('to', params.to)
  return request<{ points: TimeSeriesPoint[] }>(`/usage/my/trend?${qs}`)
}
export const getMyUsageTrendBreakdown = (params: { group_by?: string; from?: string; to?: string }) => {
  const qs = new URLSearchParams()
  if (params.group_by) qs.set('group_by', params.group_by)
  if (params.from) qs.set('from', params.from)
  if (params.to) qs.set('to', params.to)
  return request<{ points: TimeSeriesBreakdown[] }>(`/usage/my/trend-breakdown?${qs}`)
}
export const getMyUsageRecords = (params: { from?: string; to?: string; limit?: number; offset?: number }) => {
  const qs = new URLSearchParams()
  if (params.from) qs.set('from', params.from)
  if (params.to) qs.set('to', params.to)
  if (params.limit) qs.set('limit', String(params.limit))
  if (params.offset) qs.set('offset', String(params.offset))
  return request<{ records: UsageRecord[]; total: number }>(`/usage/my/records?${qs}`)
}

// ── Shared Usage ─────────────────────────────────────
export const getSharedSummary = (from?: string, to?: string) => {
  const qs = new URLSearchParams({ group_by: 'key' })
  if (from) qs.set('from', from)
  if (to) qs.set('to', to)
  return request<SharedSummary>(`/usage/shared/summary?${qs}`)
}
export const getSharedTrend = (params: { granularity?: string; from?: string; to?: string }) => {
  const qs = new URLSearchParams()
  if (params.granularity) qs.set('granularity', params.granularity)
  if (params.from) qs.set('from', params.from)
  if (params.to) qs.set('to', params.to)
  return request<{ points: TimeSeriesPoint[] }>(`/usage/shared/trend?${qs}`)
}
export const getSharedTop = (rankBy: string, from?: string, to?: string) => {
  const qs = new URLSearchParams({ rank_by: rankBy })
  if (from) qs.set('from', from)
  if (to) qs.set('to', to)
  return request<{ groups: UsageAggregate[] }>(`/usage/shared/top?${qs}`)
}
export const getSharedKeys = () =>
  request<{ keys: { id: string; name: string; assigned_to: string; last_used_at: string | null; created_by: string }[] }>(`/usage/shared/keys`)
export const getSharedRecords = (params: { from?: string; to?: string; api_key_id?: string; model?: string; limit?: number; offset?: number }) => {
  const qs = new URLSearchParams()
  if (params.from) qs.set('from', params.from)
  if (params.to) qs.set('to', params.to)
  if (params.api_key_id) qs.set('api_key_id', params.api_key_id)
  if (params.model) qs.set('model', params.model)
  if (params.limit) qs.set('limit', String(params.limit))
  if (params.offset) qs.set('offset', String(params.offset))
  return request<{ records: UsageRecord[]; total: number }>(`/usage/shared/records?${qs}`)
}

// ── Legacy usage (keep for existing consumers) ────────
export const getUsageRecords = (params: { limit?: number; offset?: number; from?: string; to?: string }) => {
  const qs = new URLSearchParams()
  if (params.from) qs.set('from', params.from)
  if (params.to) qs.set('to', params.to)
  if (params.limit) qs.set('limit', String(params.limit))
  if (params.offset) qs.set('offset', String(params.offset))
  return request<{ records: UsageRecord[]; total: number }>(`/usage?${qs}`)
}
export const getUsageSummary = (groupBy: string, from?: string, to?: string) => {
  const qs = new URLSearchParams({ group_by: groupBy })
  if (from) qs.set('from', from)
  if (to) qs.set('to', to)
  return request<{ groups: UsageAggregate[] }>(`/usage/summary?${qs}`)
}
export const getKeyUsage = (keyId: string, params: { from?: string; to?: string; limit?: number; offset?: number }) => {
  const qs = new URLSearchParams()
  if (params.from) qs.set('from', params.from)
  if (params.to) qs.set('to', params.to)
  if (params.limit) qs.set('limit', String(params.limit))
  if (params.offset) qs.set('offset', String(params.offset))
  return request<{ records: UsageRecord[]; total: number }>(`/keys/${keyId}/usage?${qs}`)
}
