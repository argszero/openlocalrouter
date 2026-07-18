import { useState, useRef } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  getProviders, getEndpoints, getPresets,
  createProvider, updateProvider, deleteProvider,
  deleteModel, setModelVisibility,
  type Provider, type Endpoint, type Model, type ProviderPreset,
} from '../lib/api'
import { toast } from 'sonner'
import {
  Plus, Trash2, ToggleLeft, ToggleRight,
  Edit3, Check, Store, Globe, Server, X, Zap, Brain,
} from 'lucide-react'

const PROTOCOL_LABELS: Record<string, { label: string; desc: string }> = {
  openai_chat: { label: 'OpenAI Chat', desc: 'Chat Completions API — 兼容性最广' },
  openai_responses: { label: 'OpenAI Responses', desc: 'Responses API — 原生支持多模态' },
  anthropic_messages: { label: 'Anthropic Messages', desc: 'Messages API — Claude 原生协议' },
}

const CATEGORY_LABELS: Record<string, string> = {
  official: '官方',
  cloud: '云平台',
  custom: '自定义',
}

// ── Pending Model (local state during creation) ──────────

interface PendingModel {
  key: string
  slug: string
  display_name: string
  context_window: number
  model_slug?: string
}

// ── Page ──────────────────────────────────────────────

export default function ProvidersPage() {
  const queryClient = useQueryClient()
  const { data: providers, isLoading } = useQuery({ queryKey: ['providers'], queryFn: getProviders })
  const { data: endpoints } = useQuery({ queryKey: ['endpoints'], queryFn: getEndpoints })
  const [showWizard, setShowWizard] = useState(false)
  const [editingId, setEditingId] = useState<string | null>(null)

  const deleteMut = useMutation({
    mutationFn: deleteProvider,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['providers'] })
      toast.success('Provider 已删除')
    },
    onError: (err: Error) => toast.error(err.message),
  })

  const updateMut = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Partial<Provider> }) => updateProvider(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['providers'] })
      toast.success('Provider 已更新')
    },
    onError: (err: Error) => toast.error(err.message),
  })

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-2xl font-semibold text-gray-800">Provider 管理</h2>
        <button
          onClick={() => setShowWizard(true)}
          className="flex items-center gap-2 px-4 py-2 bg-indigo-600 text-white text-sm font-medium rounded-lg hover:bg-indigo-700 transition-colors"
        >
          <Plus size={16} /> 添加 Provider
        </button>
      </div>

      {/* Create Wizard */}
      {showWizard && (
        <CreateProviderWizard
          onClose={() => setShowWizard(false)}
          onCreated={() => {
            queryClient.invalidateQueries({ queryKey: ['providers'] })
            setShowWizard(false)
          }}
        />
      )}

      {/* Provider List */}
      <div className="space-y-3">
        {isLoading ? (
          <div className="text-sm text-gray-400">加载中…</div>
        ) : !providers?.length ? (
          <div className="bg-white rounded-xl border border-gray-200 p-12 text-center">
            <Server size={40} className="mx-auto mb-3 text-gray-300" />
            <p className="text-gray-500 font-medium mb-1">还没有 Provider</p>
            <p className="text-sm text-gray-400 mb-4">添加上游 AI 服务商，如 OpenAI、Anthropic 等</p>
            <button
              onClick={() => setShowWizard(true)}
              className="inline-flex items-center gap-2 px-4 py-2 bg-indigo-600 text-white text-sm font-medium rounded-lg hover:bg-indigo-700 transition-colors"
            >
              <Store size={16} /> 添加 Provider
            </button>
          </div>
        ) : (
          providers.map((p) => (
            <div key={p.id} className="bg-white rounded-xl border border-gray-200 overflow-hidden">
              <div className="flex items-center justify-between p-4">
                <div className="flex items-center gap-3">
                  <div className="w-8 h-8 rounded-lg bg-gray-100 flex items-center justify-center text-sm">
                    <Server size={16} className="text-gray-500" />
                  </div>
                  <div>
                    <div className="flex items-center gap-2">
                      <p className="text-sm font-medium text-gray-800">{p.name}</p>
                      <div className="flex flex-wrap gap-1">
                        {(p.api_types || (p.api_type ? p.api_type.split(',').filter(Boolean) : [])).map((t: string) => (
                          <span key={t} className="inline-flex items-center px-2 py-0.5 text-xs rounded-full bg-indigo-50 text-indigo-700 border border-indigo-200 font-medium">
                            {PROTOCOL_LABELS[t]?.label || t}
                          </span>
                        ))}
                      </div>
                    </div>
                    <p className="text-xs text-gray-400 font-mono">{p.base_url}</p>
                    {p.api_urls && Object.keys(p.api_urls).length > 0 && (
                      <div className="mt-1 space-y-0.5">
                        {Object.entries(p.api_urls).map(([proto, url]) => (
                          <p key={proto} className="text-xs text-gray-400 font-mono">
                            <span className="text-indigo-500">{PROTOCOL_LABELS[proto]?.label || proto}:</span> {url}
                          </p>
                        ))}
                      </div>
                    )}
                    <span className="text-xs text-gray-400">
                      <ModelCount providerId={p.id} />
                    </span>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <button
                    onClick={() => updateMut.mutate({ id: p.id, data: { enabled: !p.enabled } })}
                  >
                    {p.enabled ? <ToggleRight size={20} className="text-green-500" /> : <ToggleLeft size={20} />}
                  </button>
                  <button
                    onClick={() => editingId === p.id ? setEditingId(null) : setEditingId(p.id)}
                    className={`p-1.5 rounded-lg transition-colors ${
                      editingId === p.id ? 'bg-indigo-100 text-indigo-600' : 'text-gray-400 hover:bg-gray-100'
                    }`}
                    title="编辑"
                  >
                    <Edit3 size={16} />
                  </button>
                  <button
                    onClick={() => { if (confirm('确定删除此 Provider？关联的模型也会被删除。')) deleteMut.mutate(p.id) }}
                    className="p-1.5 text-gray-400 hover:text-red-500 hover:bg-red-50 rounded-lg transition-colors"
                    title="删除"
                  >
                    <Trash2 size={16} />
                  </button>
                </div>
              </div>

              {/* Edit Form */}
              {editingId === p.id && (
                <EditProviderForm
                  provider={p}
                  endpoints={endpoints || []}
                  onClose={() => setEditingId(null)}
                  onUpdated={() => {
                    queryClient.invalidateQueries({ queryKey: ['providers'] })
                    setEditingId(null)
                  }}
                />
              )}
            </div>
          ))
        )}
      </div>
    </div>
  )
}

// ── Model Count (lazy) ────────────────────────────────

function ModelCount({ providerId }: { providerId: string }) {
  const { data } = useQuery({
    queryKey: ['providerModelCount', providerId],
    queryFn: async () => {
      const res = await fetch(`/api/admin/providers/${providerId}/models`, {
        headers: { Authorization: `Bearer ${localStorage.getItem('token')}` },
      })
      const json = await res.json()
      return json.count as number
    },
    staleTime: 30_000,
  })

  if (data === undefined) return null
  return <>{data} 个模型</>
}

// ── Create Wizard ──────────────────────────────────────

function CreateProviderWizard({ onClose, onCreated }: { onClose: () => void; onCreated: () => void }) {
  const { data: presets } = useQuery({ queryKey: ['presets'], queryFn: getPresets })
  const [step, setStep] = useState<'preset' | 'config'>('preset')
  const [selectedPreset, setSelectedPreset] = useState<ProviderPreset | null>(null)
  const [form, setForm] = useState<{
    name: string; base_url: string; api_key: string; api_types: string[]
    api_urls: Record<string, string>
  }>({
    name: '', base_url: '', api_key: '', api_types: ['openai_chat'], api_urls: {},
  })

  // Model config — local state, batched on submit
  const [pendingModels, setPendingModels] = useState<PendingModel[]>([])
  const [showAddModel, setShowAddModel] = useState(false)
  const [newModelSlug, setNewModelSlug] = useState('')
  const [newModelDisplay, setNewModelDisplay] = useState('')
  const [newModelCtx, setNewModelCtx] = useState(128000)
  const [newModelUpstream, setNewModelUpstream] = useState('')
  const [fetchingModels, setFetchingModels] = useState(false)
  const [creating, setCreating] = useState(false)
  const [editingModelKey, setEditingModelKey] = useState<string | null>(null)
  const [editSlug, setEditSlug] = useState('')
  const [editDisplay, setEditDisplay] = useState('')
  const [editCtx, setEditCtx] = useState(128000)
  const [editUpstream, setEditUpstream] = useState('')

  const pendingModelsRef = useRef(pendingModels)
  pendingModelsRef.current = pendingModels

  const handlePresetSelect = (preset: ProviderPreset) => {
    setSelectedPreset(preset)
    if (preset.name === '自定义') {
      setForm({ name: '', base_url: '', api_key: '', api_types: ['openai_chat'], api_urls: {} })
      setPendingModels([])
    } else {
      setForm({
        name: preset.name,
        base_url: preset.base_url,
        api_key: '',
        api_types: [...preset.api_types],
        api_urls: preset.api_urls ? { ...preset.api_urls } : {},
      })
      // All preset hints default to selected
      if (preset.models_hint) {
        setPendingModels(preset.models_hint.map((h, i) => ({
          key: `hint-${i}`,
          slug: h.slug,
          display_name: h.display_name,
          context_window: h.context_window || 128000,
        })))
      } else {
        setPendingModels([])
      }
    }
    setStep('config')
  }

  const handleSubmit = async () => {
    setCreating(true)
    try {
      const provider = await createProvider({
        name: form.name,
        base_url: form.base_url,
        api_key: form.api_key,
        api_types: form.api_types,
        ...(Object.keys(form.api_urls).length > 0 ? { api_urls: form.api_urls } : {}),
      } as any)

      const models = pendingModelsRef.current
      let modelErrors = 0
      for (const m of models) {
        try {
          await fetch(`/api/admin/providers/${provider.id}/models`, {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
              Authorization: `Bearer ${localStorage.getItem('token')}`,
            },
            body: JSON.stringify({
              slug: m.slug,
              display_name: m.display_name,
              context_window: m.context_window,
              ...(m.model_slug && m.model_slug !== m.slug ? { model_slug: m.model_slug } : {}),
            }),
          })
        } catch { modelErrors++ }
      }

      if (modelErrors > 0) {
        toast.warning(`Provider 已创建，但 ${modelErrors} 个模型添加失败`)
      } else if (models.length > 0) {
        toast.success(`Provider 已创建，已添加 ${models.length} 个模型`)
      } else {
        toast.success('Provider 已创建')
      }
      onCreated()
    } catch (err: any) {
      toast.error(err.message)
    } finally {
      setCreating(false)
    }
  }

  const addModel = () => {
    const s = newModelSlug.trim()
    if (!s) return
    if (pendingModels.some(m => m.slug === s)) {
      toast.error('模型 slug 已存在')
      return
    }
    setPendingModels(prev => [...prev, {
      key: `m-${Date.now()}`,
      slug: s,
      display_name: newModelDisplay.trim() || s,
      context_window: newModelCtx,
      model_slug: newModelUpstream.trim() || undefined,
    }])
    setNewModelSlug(''); setNewModelDisplay(''); setNewModelCtx(128000); setNewModelUpstream('')
    setShowAddModel(false)
  }

  const removeModel = (key: string) => {
    setPendingModels(prev => prev.filter(m => m.key !== key))
  }

  const updatePendingModel = (key: string) => {
    const s = editSlug.trim()
    if (!s) return
    setPendingModels(prev => prev.map(m => m.key === key ? {
      ...m,
      slug: s,
      display_name: editDisplay.trim() || s,
      context_window: editCtx,
      model_slug: editUpstream.trim() || undefined,
    } : m))
    setEditingModelKey(null)
  }

  const handleFetchUpstream = async () => {
    const base = form.base_url || Object.values(form.api_urls)[0] || ''
    if (!base) { toast.error('未配置 Base URL'); return }
    setFetchingModels(true)
    try {
      const url = base.endsWith('/models') ? base : `${base.replace(/\/+$/, '')}/models`
      const headers: Record<string, string> = { 'Content-Type': 'application/json' }
      if (form.api_key) headers['Authorization'] = `Bearer ${form.api_key}`
      const resp = await fetch(url, { headers })
      if (!resp.ok) { toast.error(`请求 /models 失败: ${resp.status}`); return }
      const data = await resp.json()
      const modelList = data.data || data.models || data || []
      const existing = new Set(pendingModelsRef.current.map(m => m.slug))
      const toAdd: PendingModel[] = []
      for (const m of modelList) {
        const s = m.id || m.name || m.model || ''
        if (!s || existing.has(s)) continue
        toAdd.push({
          key: `up-${Date.now()}-${toAdd.length}`,
          slug: s,
          display_name: m.name || m.display_name || s,
          context_window: m.context_window || 128000,
        })
        existing.add(s)
      }
      setPendingModels(prev => [...prev, ...toAdd])
      toast.success(`从上游获取了 ${toAdd.length} 个模型`)
    } catch (err: any) { toast.error(`获取模型列表失败: ${err.message}`) }
    finally { setFetchingModels(false) }
  }

  const categories = [...new Set((presets || []).map(p => p.category))]
  const baseUrlForFetch = form.base_url || Object.values(form.api_urls)[0] || ''

  return (
    <div className="bg-white rounded-xl border border-gray-200 overflow-hidden mb-4">
      {/* Header */}
      <div className="flex items-center justify-between px-5 py-4 border-b border-gray-100 bg-gray-50/50">
        <div className="flex items-center gap-3">
          <Store size={20} className="text-indigo-500" />
          <div>
            <h3 className="text-sm font-semibold text-gray-800">
              添加 Provider
              {selectedPreset && <span className="text-gray-400 font-normal"> — {selectedPreset.name}</span>}
            </h3>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {step === 'config' && (
            <button onClick={() => setStep('preset')} className="text-sm text-gray-500 hover:text-gray-700">
              返回选择
            </button>
          )}
          <button onClick={onClose} className="p-1.5 text-gray-400 hover:text-gray-600 rounded-lg hover:bg-gray-100">
            <X size={18} />
          </button>
        </div>
      </div>

      {/* Step 1: Preset Selection */}
      {step === 'preset' && (
        <div className="p-5">
          <p className="text-sm text-gray-500 mb-4">选择一个预设快速配置，或选"自定义"手动填写</p>
          {categories.map(cat => (
            <div key={cat} className="mb-4">
              <p className="text-xs font-medium text-gray-400 uppercase mb-2">
                {CATEGORY_LABELS[cat] || cat}
              </p>
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
                {(presets || []).filter(p => p.category === cat).map(p => (
                  <button
                    key={p.name}
                    onClick={() => handlePresetSelect(p)}
                    className="text-left p-4 rounded-xl border border-gray-200 hover:border-indigo-300 hover:bg-indigo-50/30 transition-all group"
                  >
                    <div className="flex items-center gap-3 mb-2">
                      <span className="text-2xl">{p.icon}</span>
                      <span className="font-medium text-gray-800 text-sm">{p.name}</span>
                    </div>
                    <p className="text-xs text-gray-400 leading-relaxed">{p.description}</p>
                    {p.models_hint && p.models_hint.length > 0 && (
                      <div className="mt-2 flex flex-wrap gap-1">
                        {p.models_hint.slice(0, 4).map(m => (
                          <span key={m.slug} className="text-xs px-1.5 py-0.5 bg-gray-100 rounded text-gray-500 font-mono">
                            {m.slug}
                          </span>
                        ))}
                        {p.models_hint.length > 4 && (
                          <span className="text-xs text-gray-400">+{p.models_hint.length - 4}</span>
                        )}
                      </div>
                    )}
                  </button>
                ))}
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Step 2: Config (Protocol + Model) */}
      {step === 'config' && selectedPreset && (
        <div className="p-5">
          {/* Protocol hint */}
          <div className="flex items-center gap-2 p-3 mb-4 bg-indigo-50 rounded-lg border border-indigo-100">
            <Zap size={16} className="text-indigo-500 shrink-0" />
            <div>
              <span className="text-xs font-medium text-indigo-700">
                已选中 {form.api_types.length} 个协议
              </span>
              <span className="text-xs text-indigo-500 ml-1">
                — {form.api_types.map(t => PROTOCOL_LABELS[t]?.label).join('、')}
              </span>
            </div>
          </div>

          <div className="space-y-6">
            {/* Name & API Key */}
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1">名称</label>
                <input
                  value={form.name}
                  onChange={e => setForm({ ...form, name: e.target.value })}
                  className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
                  placeholder="如 OpenAI、Anthropic"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1">API Key</label>
                <input
                  type="password"
                  value={form.api_key}
                  onChange={e => setForm({ ...form, api_key: e.target.value })}
                  className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
                  placeholder="sk-..."
                />
              </div>
            </div>

            {/* ── Protocol Config ── */}
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-2">协议与 URL 配置</label>
              <p className="text-xs text-gray-400 mb-3">选择需要的协议类型，并为每种协议配置对应的 Base URL</p>
              <div className="space-y-3">
                {Object.entries(PROTOCOL_LABELS).map(([key, { label, desc }]) => {
                  const checked = form.api_types.includes(key)
                  return (
                    <div
                      key={key}
                      className={`p-4 rounded-lg border transition-all ${
                        checked
                          ? 'border-indigo-300 bg-indigo-50/50'
                          : 'border-gray-200 bg-gray-50/30'
                      }`}
                    >
                      <div className="flex items-start gap-3">
                        <button
                          onClick={() => {
                            const next = checked
                              ? form.api_types.filter(t => t !== key)
                              : [...form.api_types, key]
                            if (next.length > 0) setForm({ ...form, api_types: next })
                          }}
                          className={`mt-0.5 w-5 h-5 rounded border-2 flex items-center justify-center shrink-0 transition-colors ${
                            checked
                              ? 'bg-indigo-600 border-indigo-600 text-white'
                              : 'border-gray-300 hover:border-gray-400'
                          }`}
                        >
                          {checked && <Check size={12} strokeWidth={3} />}
                        </button>
                        <div className="flex-1 min-w-0">
                          <p className={`text-sm font-medium ${checked ? 'text-gray-800' : 'text-gray-500'}`}>
                            {label}
                          </p>
                          <p className="text-xs text-gray-400 mt-0.5">{desc}</p>
                        </div>
                        {checked && (
                          <div className="flex-1 min-w-[300px]">
                            <input
                              value={form.api_urls[key] || ''}
                              onChange={e => {
                                const urls = { ...form.api_urls }
                                if (e.target.value) { urls[key] = e.target.value }
                                else { delete urls[key] }
                                setForm({ ...form, api_urls: urls })
                              }}
                              className="w-full px-3 py-1.5 border border-gray-300 rounded-lg text-xs font-mono focus:outline-none focus:ring-2 focus:ring-indigo-500"
                              placeholder={`https://api.example.com/${key === 'anthropic_messages' ? '' : 'v1'}`}
                            />
                          </div>
                        )}
                      </div>
                    </div>
                  )
                })}
              </div>
            </div>

            {/* ── Model Config ── */}
            <div className="border-t border-gray-100 pt-4">
              <div className="flex items-center justify-between mb-3">
                <div className="flex items-center gap-2">
                  <Brain size={14} className="text-indigo-400 shrink-0" />
                  <p className="text-sm font-medium text-gray-700">模型</p>
                  <span className="text-xs text-gray-400">({pendingModels.length})</span>
                </div>
                <div className="flex items-center gap-2">
                  <button
                    onClick={handleFetchUpstream}
                    disabled={fetchingModels || !baseUrlForFetch}
                    className="flex items-center gap-1 text-xs text-gray-500 hover:text-indigo-600 font-medium disabled:opacity-50"
                    title={!baseUrlForFetch ? '需要配置 Base URL' : '从上游 /models 获取'}
                  >
                    <Globe size={12} />
                    {fetchingModels ? '获取中…' : '从上游获取'}
                  </button>
                  <button
                    onClick={() => setShowAddModel(true)}
                    className="flex items-center gap-1 text-xs text-indigo-600 hover:text-indigo-700 font-medium"
                  >
                    <Plus size={12} /> 手动添加
                  </button>
                </div>
              </div>

              {/* Manual add form */}
              {showAddModel && (
                <div className="mb-3 p-4 bg-white rounded-lg border border-gray-200 space-y-3">
                  <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                    <div>
                      <label className="block text-xs font-medium text-gray-500 mb-1">模型 ID (slug)</label>
                      <input
                        placeholder="如 gpt-4o"
                        value={newModelSlug}
                        onChange={e => setNewModelSlug(e.target.value)}
                        className="w-full px-2 py-1.5 border border-gray-300 rounded text-sm font-mono focus:outline-none focus:ring-2 focus:ring-indigo-500"
                        onKeyDown={e => e.key === 'Enter' && addModel()}
                      />
                    </div>
                    <div>
                      <label className="block text-xs font-medium text-gray-500 mb-1">显示名称</label>
                      <input
                        placeholder="如 GPT-4o"
                        value={newModelDisplay}
                        onChange={e => setNewModelDisplay(e.target.value)}
                        className="w-full px-2 py-1.5 border border-gray-300 rounded text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
                        onKeyDown={e => e.key === 'Enter' && addModel()}
                      />
                    </div>
                    <div>
                      <label className="block text-xs font-medium text-gray-500 mb-1">
                        上游请求模型名 <span className="text-gray-400 font-normal">（可选）</span>
                      </label>
                      <input
                        value={newModelUpstream}
                        onChange={e => setNewModelUpstream(e.target.value)}
                        className="w-full px-2 py-1.5 border border-gray-300 rounded text-sm font-mono text-gray-500 focus:outline-none focus:ring-2 focus:ring-indigo-500"
                        placeholder="默认同 slug"
                        onKeyDown={e => e.key === 'Enter' && addModel()}
                      />
                    </div>
                    <div>
                      <label className="block text-xs font-medium text-gray-500 mb-1">Context Window</label>
                      <input
                        type="number"
                        value={newModelCtx}
                        onChange={e => setNewModelCtx(Number(e.target.value))}
                        className="w-full px-2 py-1.5 border border-gray-300 rounded text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
                        onKeyDown={e => e.key === 'Enter' && addModel()}
                      />
                    </div>
                  </div>
                  <div className="flex gap-2">
                    <button
                      onClick={addModel}
                      disabled={!newModelSlug.trim()}
                      className="px-4 py-1.5 bg-indigo-600 text-white text-xs rounded hover:bg-indigo-700 disabled:opacity-50"
                    >
                      添加模型
                    </button>
                    <button
                      onClick={() => {
                        setShowAddModel(false)
                        setNewModelSlug(''); setNewModelDisplay(''); setNewModelUpstream('')
                      }}
                      className="px-4 py-1.5 text-gray-600 text-xs rounded hover:bg-gray-100"
                    >
                      取消
                    </button>
                  </div>
                </div>
              )}

              {/* Model list */}
              {pendingModels.length === 0 ? (
                <p className="text-xs text-gray-400 py-2">
                  尚未添加模型。点击「从上游获取」自动拉取，或「手动添加」。
                </p>
              ) : (
                <div className="space-y-1.5">
                  {pendingModels.map((m) => (
                    <div key={m.key} className="flex items-center justify-between p-2.5 bg-white rounded-lg border border-gray-100">
                      <div className="flex items-center gap-3 min-w-0">
                        <Brain size={14} className="text-gray-300 shrink-0" />
                        {editingModelKey === m.key ? (
                          <div className="space-y-2 w-full">
                            <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
                              <div>
                                <label className="block text-xs text-gray-400 mb-0.5">slug</label>
                                <input
                                  value={editSlug}
                                  onChange={e => setEditSlug(e.target.value)}
                                  className="w-full px-1.5 py-0.5 border border-gray-300 rounded text-xs font-mono"
                                />
                              </div>
                              <div>
                                <label className="block text-xs text-gray-400 mb-0.5">显示名称</label>
                                <input
                                  value={editDisplay}
                                  onChange={e => setEditDisplay(e.target.value)}
                                  className="w-full px-1.5 py-0.5 border border-gray-300 rounded text-xs"
                                />
                              </div>
                              <div>
                                <label className="block text-xs text-gray-400 mb-0.5">上游请求模型名</label>
                                <input
                                  value={editUpstream}
                                  onChange={e => setEditUpstream(e.target.value)}
                                  className="w-full px-1.5 py-0.5 border border-gray-300 rounded text-xs font-mono"
                                  placeholder="默认同 slug"
                                />
                              </div>
                              <div>
                                <label className="block text-xs text-gray-400 mb-0.5">Context Window</label>
                                <input
                                  type="number"
                                  value={editCtx}
                                  onChange={e => setEditCtx(Number(e.target.value))}
                                  className="w-full px-1.5 py-0.5 border border-gray-300 rounded text-xs"
                                />
                              </div>
                            </div>
                            <div className="flex gap-2">
                              <button
                                onClick={() => updatePendingModel(m.key)}
                                className="px-2 py-0.5 bg-indigo-600 text-white text-xs rounded hover:bg-indigo-700"
                              >
                                保存
                              </button>
                              <button
                                onClick={() => setEditingModelKey(null)}
                                className="px-2 py-0.5 text-gray-500 text-xs rounded hover:bg-gray-100"
                              >
                                取消
                              </button>
                            </div>
                          </div>
                        ) : (
                          <div className="min-w-0">
                            <div className="flex items-center gap-2">
                              <p className="text-sm font-medium text-gray-700 font-mono">{m.slug}</p>
                              <button
                                onClick={() => {
                                  setEditingModelKey(m.key)
                                  setEditSlug(m.slug)
                                  setEditDisplay(m.display_name)
                                  setEditCtx(m.context_window)
                                  setEditUpstream(m.model_slug || '')
                                }}
                                className="text-gray-300 hover:text-gray-500"
                                title="编辑模型"
                              >
                                <Edit3 size={12} />
                              </button>
                            </div>
                            <p className="text-xs text-gray-400">
                              {m.display_name}{m.model_slug ? ` → ${m.model_slug}` : ''} · {m.context_window.toLocaleString()} tokens
                            </p>
                          </div>
                        )}
                      </div>
                      <button
                        onClick={() => removeModel(m.key)}
                        className="p-1 text-gray-400 hover:text-red-500 rounded transition-colors"
                      >
                        <Trash2 size={14} />
                      </button>
                    </div>
                  ))}
                </div>
              )}
            </div>

            {/* Action buttons */}
            <div className="flex gap-2 pt-2">
              <button
                onClick={handleSubmit}
                disabled={creating || !form.name || (!form.base_url && Object.keys(form.api_urls).length === 0)}
                className="flex items-center gap-2 px-4 py-2 bg-indigo-600 text-white text-sm font-medium rounded-lg hover:bg-indigo-700 disabled:opacity-50 transition-colors"
              >
                {creating ? '创建中…' : <><Check size={16} /> 创建 Provider</>}
              </button>
              <button onClick={onClose} className="px-4 py-2 text-gray-600 text-sm hover:bg-gray-100 rounded-lg">
                取消
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

// ── Edit Provider Form ─────────────────────────────────

function EditProviderForm({ provider, endpoints, onClose, onUpdated }: {
  provider: Provider
  endpoints: Endpoint[]
  onClose: () => void
  onUpdated: () => void
}) {
  const [form, setForm] = useState({
    name: provider.name,
    base_url: provider.base_url,
    api_key: '',
    api_types: [...(provider.api_types || (provider.api_type ? provider.api_type.split(',').filter(Boolean) : ['openai_chat']))],
    api_urls: { ...(provider.api_urls || {}) } as Record<string, string>,
  })

  // Model management state
  const [showAddModel, setShowAddModel] = useState(false)
  const [newModelSlug, setNewModelSlug] = useState('')
  const [newModelDisplay, setNewModelDisplay] = useState('')
  const [newModelCtx, setNewModelCtx] = useState(128000)
  const [newModelUpstream, setNewModelUpstream] = useState('')
  const [fetchingModels, setFetchingModels] = useState(false)
  const [editingModelId, setEditingModelId] = useState<string | null>(null)
  const [editSlug, setEditSlug] = useState('')
  const [editDisplay, setEditDisplay] = useState('')
  const [editCtx, setEditCtx] = useState(128000)
  const [editUpstream, setEditUpstream] = useState('')

  // Fetch models from API
  const { data: providerDetail, refetch: refetchModels } = useQuery({
    queryKey: ['providerDetail', provider.id],
    queryFn: async () => {
      const res = await fetch(`/api/admin/providers/${provider.id}`, {
        headers: { Authorization: `Bearer ${localStorage.getItem('token')}` },
      })
      return res.json() as Promise<Provider & { models: Model[] }>
    },
  })
  const models = providerDetail?.models || []

  const updateMut = useMutation({
    mutationFn: (data: Partial<Provider>) => updateProvider(provider.id, data),
    onSuccess: () => {
      toast.success('Provider 已更新')
      onUpdated()
    },
    onError: (err: Error) => toast.error(err.message),
  })

  const addModel = async () => {
    const s = newModelSlug.trim()
    if (!s) return
    try {
      const res = await fetch(`/api/admin/providers/${provider.id}/models`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${localStorage.getItem('token')}`,
        },
        body: JSON.stringify({
          slug: s,
          display_name: newModelDisplay.trim() || s,
          context_window: newModelCtx,
          ...(newModelUpstream.trim() && newModelUpstream.trim() !== s ? { model_slug: newModelUpstream.trim() } : {}),
        }),
      })
      if (!res.ok) throw new Error((await res.json()).error?.message || 'Failed')
      setNewModelSlug(''); setNewModelDisplay(''); setNewModelCtx(128000); setNewModelUpstream('')
      setShowAddModel(false)
      refetchModels()
      onUpdated()
      toast.success(`模型 "${s}" 已添加`)
    } catch (err: any) { toast.error(err.message) }
  }

  const handleRemoveModel = async (modelId: string, modelSlug: string) => {
    if (!confirm(`删除模型 ${modelSlug}？`)) return
    try {
      await deleteModel(provider.id, modelId)
      refetchModels()
      onUpdated()
      toast.success(`模型 "${modelSlug}" 已删除`)
    } catch (err: any) { toast.error(err.message) }
  }

  const handleUpdateModel = async (m: Model) => {
    try {
      // delete + recreate pattern
      await deleteModel(provider.id, m.id)
      const res = await fetch(`/api/admin/providers/${provider.id}/models`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${localStorage.getItem('token')}`,
        },
        body: JSON.stringify({
          slug: editSlug.trim() || m.slug,
          display_name: editDisplay.trim() || m.display_name,
          context_window: editCtx || m.context_window,
          model_slug: editUpstream.trim() || undefined,
          visible_endpoint_ids: m.visible_endpoint_ids || [],
        }),
      })
      if (!res.ok) throw new Error((await res.json()).error?.message || 'Failed')
      setEditingModelId(null)
      refetchModels()
      onUpdated()
      toast.success('模型已更新')
    } catch (err: any) { toast.error(err.message) }
  }

  const handleFetchUpstream = async () => {
    const base = form.base_url || Object.values(form.api_urls)[0] || provider.base_url
    if (!base) { toast.error('未配置 Base URL'); return }
    setFetchingModels(true)
    try {
      const url = base.endsWith('/models') ? base : `${base.replace(/\/+$/, '')}/models`
      const headers: Record<string, string> = { 'Content-Type': 'application/json' }
      if (form.api_key || provider.api_key) {
        // Use current API key from form or existing provider
      }
      const resp = await fetch(url, { headers })
      if (!resp.ok) { toast.error(`请求 /models 失败: ${resp.status}`); return }
      const data = await resp.json()
      const modelList = data.data || data.models || data || []
      const existing = new Set(models.map(m => m.slug))
      let added = 0
      for (const item of modelList) {
        const s = item.id || item.name || item.model || ''
        if (!s || existing.has(s)) continue
        try {
          const r = await fetch(`/api/admin/providers/${provider.id}/models`, {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
              Authorization: `Bearer ${localStorage.getItem('token')}`,
            },
            body: JSON.stringify({
              slug: s,
              display_name: item.name || item.display_name || s,
              context_window: item.context_window || 128000,
            }),
          })
          if (r.ok) { added++; existing.add(s) }
        } catch { /* skip */ }
      }
      if (added > 0) {
        refetchModels()
        onUpdated()
      }
      toast.success(`从上游获取了 ${added} 个模型`)
    } catch (err: any) { toast.error(`获取模型列表失败: ${err.message}`) }
    finally { setFetchingModels(false) }
  }

  return (
    <div className="border-t border-gray-100 p-4 bg-gray-50/30 space-y-6">
      {/* Provider edit fields */}
      <div>
        <h4 className="text-sm font-medium text-gray-700 mb-3">编辑 Provider</h4>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-4">
          <div>
            <label className="block text-xs font-medium text-gray-500 mb-1">名称</label>
            <input
              value={form.name}
              onChange={e => setForm({ ...form, name: e.target.value })}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
            />
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-500 mb-1">Base URL</label>
            <input
              value={form.base_url}
              onChange={e => setForm({ ...form, base_url: e.target.value })}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm font-mono focus:outline-none focus:ring-2 focus:ring-indigo-500"
            />
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-500 mb-1">API Key (留空不修改)</label>
            <input
              type="password"
              value={form.api_key}
              onChange={e => setForm({ ...form, api_key: e.target.value })}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
              placeholder="留空则保持原值"
            />
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-500 mb-1">协议类型（可多选）</label>
            <div className="flex flex-wrap gap-2">
              {Object.entries(PROTOCOL_LABELS).map(([k, v]) => {
                const checked = form.api_types.includes(k)
                return (
                  <label
                    key={k}
                    className={`inline-flex items-center gap-2 px-3 py-1.5 text-xs rounded-lg border cursor-pointer transition-colors ${
                      checked
                        ? 'bg-indigo-50 border-indigo-300 text-indigo-700'
                        : 'bg-white border-gray-200 text-gray-600 hover:border-gray-300'
                    }`}
                  >
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={() => {
                        const next = checked
                          ? form.api_types.filter(t => t !== k)
                          : [...form.api_types, k]
                        if (next.length > 0) setForm({ ...form, api_types: next })
                      }}
                      className="sr-only"
                    />
                    <span className={`w-3.5 h-3.5 rounded border-2 flex items-center justify-center ${
                      checked ? 'bg-indigo-600 border-indigo-600' : 'border-gray-300'
                    }`}>
                      {checked && <Check size={10} strokeWidth={3} className="text-white" />}
                    </span>
                    {v.label}
                    {(form.api_urls[k]) && (
                      <span className="text-gray-400 font-mono">— {form.api_urls[k]}</span>
                    )}
                  </label>
                )
              })}
            </div>
            {form.api_types.length > 0 && (
              <div className="space-y-2 mt-3">
                {form.api_types.map(proto => (
                  <div key={proto} className="flex items-center gap-2">
                    <span className="text-xs font-medium text-indigo-600 w-28 shrink-0">
                      {PROTOCOL_LABELS[proto]?.label}
                    </span>
                    <input
                      value={form.api_urls[proto] || ''}
                      onChange={e => {
                        const urls = { ...form.api_urls }
                        if (e.target.value) { urls[proto] = e.target.value }
                        else { delete urls[proto] }
                        setForm({ ...form, api_urls: urls })
                      }}
                      className="flex-1 px-2 py-1 border border-gray-300 rounded text-xs font-mono focus:outline-none focus:ring-2 focus:ring-indigo-500"
                      placeholder={provider.base_url || `https://api.example.com/`}
                    />
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => updateMut.mutate({
              name: form.name,
              base_url: form.base_url,
              api_types: form.api_types,
              api_urls: Object.keys(form.api_urls).length > 0 ? form.api_urls : undefined,
              ...(form.api_key ? { api_key: form.api_key } : {}),
            })}
            disabled={updateMut.isPending}
            className="px-4 py-2 bg-indigo-600 text-white text-sm rounded-lg hover:bg-indigo-700 disabled:opacity-50"
          >
            {updateMut.isPending ? '保存中…' : '保存'}
          </button>
          <button onClick={onClose} className="px-4 py-2 text-gray-600 text-sm hover:bg-gray-100 rounded-lg">取消</button>
        </div>
      </div>

      {/* ── Model Config ── */}
      <div className="border-t border-gray-200 pt-4">
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2">
            <Brain size={14} className="text-indigo-400 shrink-0" />
            <p className="text-sm font-medium text-gray-700">模型</p>
            <span className="text-xs text-gray-400">({models.length})</span>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={handleFetchUpstream}
              disabled={fetchingModels}
              className="flex items-center gap-1 text-xs text-gray-500 hover:text-indigo-600 font-medium disabled:opacity-50"
            >
              <Globe size={12} />
              {fetchingModels ? '获取中…' : '从上游获取'}
            </button>
            <button
              onClick={() => setShowAddModel(true)}
              className="flex items-center gap-1 text-xs text-indigo-600 hover:text-indigo-700 font-medium"
            >
              <Plus size={12} /> 手动添加
            </button>
          </div>
        </div>

        {/* Manual add form */}
        {showAddModel && (
          <div className="mb-3 p-4 bg-white rounded-lg border border-gray-200 space-y-3">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              <div>
                <label className="block text-xs font-medium text-gray-500 mb-1">模型 ID (slug)</label>
                <input
                  placeholder="如 gpt-4o"
                  value={newModelSlug}
                  onChange={e => setNewModelSlug(e.target.value)}
                  className="w-full px-2 py-1.5 border border-gray-300 rounded text-sm font-mono focus:outline-none focus:ring-2 focus:ring-indigo-500"
                  onKeyDown={e => e.key === 'Enter' && addModel()}
                />
              </div>
              <div>
                <label className="block text-xs font-medium text-gray-500 mb-1">显示名称</label>
                <input
                  placeholder="如 GPT-4o"
                  value={newModelDisplay}
                  onChange={e => setNewModelDisplay(e.target.value)}
                  className="w-full px-2 py-1.5 border border-gray-300 rounded text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
                  onKeyDown={e => e.key === 'Enter' && addModel()}
                />
              </div>
              <div>
                <label className="block text-xs font-medium text-gray-500 mb-1">
                  上游请求模型名 <span className="text-gray-400 font-normal">（可选）</span>
                </label>
                <input
                  value={newModelUpstream}
                  onChange={e => setNewModelUpstream(e.target.value)}
                  className="w-full px-2 py-1.5 border border-gray-300 rounded text-sm font-mono text-gray-500 focus:outline-none focus:ring-2 focus:ring-indigo-500"
                  placeholder="默认同 slug"
                  onKeyDown={e => e.key === 'Enter' && addModel()}
                />
              </div>
              <div>
                <label className="block text-xs font-medium text-gray-500 mb-1">Context Window</label>
                <input
                  type="number"
                  value={newModelCtx}
                  onChange={e => setNewModelCtx(Number(e.target.value))}
                  className="w-full px-2 py-1.5 border border-gray-300 rounded text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
                  onKeyDown={e => e.key === 'Enter' && addModel()}
                />
              </div>
            </div>
            <div className="flex gap-2">
              <button
                onClick={addModel}
                disabled={!newModelSlug.trim()}
                className="px-4 py-1.5 bg-indigo-600 text-white text-xs rounded hover:bg-indigo-700 disabled:opacity-50"
              >
                添加模型
              </button>
              <button
                onClick={() => {
                  setShowAddModel(false)
                  setNewModelSlug(''); setNewModelDisplay(''); setNewModelUpstream('')
                }}
                className="px-4 py-1.5 text-gray-600 text-xs rounded hover:bg-gray-100"
              >
                取消
              </button>
            </div>
          </div>
        )}

        {/* Model list */}
        {models.length === 0 ? (
          <p className="text-xs text-gray-400 py-2">
            尚无模型。点击「从上游获取」自动拉取，或「手动添加」。
          </p>
        ) : (
          <div className="space-y-1.5">
            {models.map((m) => (
              <div key={m.id} className="flex items-center justify-between p-2.5 bg-white rounded-lg border border-gray-100">
                <div className="flex items-center gap-3 min-w-0">
                  <Brain size={14} className="text-gray-300 shrink-0" />
                  <div className="min-w-0">
                    {editingModelId === m.id ? (
                      <div className="space-y-2 w-full">
                        <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
                          <div>
                            <label className="block text-xs text-gray-400 mb-0.5">slug</label>
                            <input
                              value={editSlug}
                              onChange={e => setEditSlug(e.target.value)}
                              className="w-full px-1.5 py-0.5 border border-gray-300 rounded text-xs font-mono"
                            />
                          </div>
                          <div>
                            <label className="block text-xs text-gray-400 mb-0.5">显示名称</label>
                            <input
                              value={editDisplay}
                              onChange={e => setEditDisplay(e.target.value)}
                              className="w-full px-1.5 py-0.5 border border-gray-300 rounded text-xs"
                            />
                          </div>
                          <div>
                            <label className="block text-xs text-gray-400 mb-0.5">上游请求模型名</label>
                            <input
                              value={editUpstream}
                              onChange={e => setEditUpstream(e.target.value)}
                              className="w-full px-1.5 py-0.5 border border-gray-300 rounded text-xs font-mono"
                              placeholder="默认同 slug"
                            />
                          </div>
                          <div>
                            <label className="block text-xs text-gray-400 mb-0.5">Context Window</label>
                            <input
                              type="number"
                              value={editCtx}
                              onChange={e => setEditCtx(Number(e.target.value))}
                              className="w-full px-1.5 py-0.5 border border-gray-300 rounded text-xs"
                            />
                          </div>
                        </div>
                        <div className="flex gap-2">
                          <button
                            onClick={() => handleUpdateModel(m)}
                            className="px-2 py-0.5 bg-indigo-600 text-white text-xs rounded hover:bg-indigo-700"
                          >
                            保存
                          </button>
                          <button onClick={() => setEditingModelId(null)} className="px-2 py-0.5 text-gray-500 text-xs rounded hover:bg-gray-100">
                            取消
                          </button>
                        </div>
                      </div>
                    ) : (
                      <div>
                        <div className="flex items-center gap-2">
                          <p className="text-sm font-medium text-gray-700 font-mono">{m.slug}</p>
                          <button
                            onClick={() => {
                              setEditingModelId(m.id)
                              setEditSlug(m.slug)
                              setEditDisplay(m.display_name)
                              setEditCtx(m.context_window)
                              setEditUpstream('')
                            }}
                            className="text-gray-300 hover:text-gray-500"
                            title="编辑模型"
                          >
                            <Edit3 size={12} />
                          </button>
                        </div>
                        <p className="text-xs text-gray-400">{m.display_name} · {m.context_window.toLocaleString()} tokens</p>
                      </div>
                    )}
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  {endpoints.length > 0 && (
                    <div className="flex items-center gap-1">
                      {endpoints.map((ep) => {
                        const visible = (m.visible_endpoint_ids || []).includes(ep.id)
                        return (
                          <button
                            key={ep.id}
                            onClick={async () => {
                              const ids = visible
                                ? (m.visible_endpoint_ids || []).filter(id => id !== ep.id)
                                : [...(m.visible_endpoint_ids || []), ep.id]
                              try {
                                await setModelVisibility(provider.id, m.id, ids)
                                refetchModels()
                                onUpdated()
                              } catch (err: any) { toast.error(err.message) }
                            }}
                            className={`px-2 py-0.5 text-xs rounded-full border transition-colors ${
                              visible
                                ? 'bg-green-50 border-green-200 text-green-700 hover:bg-green-100'
                                : 'bg-gray-50 border-gray-200 text-gray-400 hover:bg-gray-100'
                            }`}
                            title={`${visible ? '在' : '不在'} ${ep.name} 中可见`}
                          >
                            {ep.name}
                          </button>
                        )
                      })}
                    </div>
                  )}
                  <button
                    onClick={() => handleRemoveModel(m.id, m.slug)}
                    className="p-1 text-gray-400 hover:text-red-500 rounded transition-colors"
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
