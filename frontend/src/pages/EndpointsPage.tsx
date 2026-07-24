import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { getEndpoints, getServerInfo, createEndpoint, updateEndpoint, deleteEndpoint } from '../lib/api'
import type { Endpoint } from '../lib/api'
import { useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import { Plus, Trash2, Key, ToggleLeft, ToggleRight, Copy, Check } from 'lucide-react'

const PROTOCOLS = ['openai_chat', 'openai_responses', 'anthropic_messages']

export default function EndpointsPage() {
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  const { data, isLoading } = useQuery({ queryKey: ['endpoints'], queryFn: getEndpoints })
  const { data: serverInfo } = useQuery({ queryKey: ['serverInfo'], queryFn: getServerInfo })
  const [showCreate, setShowCreate] = useState(false)
  const [form, setForm] = useState({ name: '', path_prefix: '', protocol: 'openai_chat' })
  const [copiedUrl, setCopiedUrl] = useState<string | null>(null)
  const [editingId, setEditingId] = useState<string | null>(null)
  const [editName, setEditName] = useState('')
  const [editPathPrefix, setEditPathPrefix] = useState('')
  const [editProtocol, setEditProtocol] = useState('openai_chat')

  const createMut = useMutation({
    mutationFn: createEndpoint,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['endpoints'] })
      toast.success('端点已创建')
      setShowCreate(false)
      setForm({ name: '', path_prefix: '', protocol: 'openai_chat' })
    },
    onError: (err: Error) => toast.error(err.message),
  })

  const updateMut = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Partial<Endpoint> & { path_prefix?: string } }) => updateEndpoint(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['endpoints'] })
      toast.success('端点已更新')
    },
    onError: (err: Error) => toast.error(err.message),
  })

  const deleteMut = useMutation({
    mutationFn: deleteEndpoint,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['endpoints'] })
      toast.success('端点已删除')
    },
    onError: (err: Error) => toast.error(err.message),
  })

  const proxyBaseUrl = serverInfo?.proxy_base_url || ''

  const getProxyUrl = (ep: Endpoint) => {
    if (proxyBaseUrl) return `${proxyBaseUrl}${ep.listen_path}`
    return ep.listen_path
  }

  const handleCopy = async (text: string) => {
    await navigator.clipboard.writeText(text)
    setCopiedUrl(text)
    setTimeout(() => setCopiedUrl(null), 2000)
    toast.success('已复制到剪贴板')
  }

  const startEdit = (ep: Endpoint) => {
    setEditingId(ep.id)
    setEditName(ep.name)
    setEditPathPrefix(ep.listen_path.replace(/^\/u\/[^/]+\//, ''))
    setEditProtocol(ep.protocol)
  }

  const saveEdit = (id: string) => {
    if (!editName.trim()) { toast.error('名称不能为空'); return }
    updateMut.mutate({
      id,
      data: {
        name: editName.trim(),
        path_prefix: editPathPrefix.trim(),
        protocol: editProtocol,
      },
    })
    setEditingId(null)
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-2xl font-semibold text-gray-800">端点管理</h2>
        <button
          onClick={() => setShowCreate(true)}
          className="flex items-center gap-2 px-4 py-2 bg-indigo-600 text-white text-sm font-medium rounded-lg hover:bg-indigo-700 transition-colors"
        >
          <Plus size={16} /> 新建端点
        </button>
      </div>

      {/* Create Form */}
      {showCreate && (
        <div className="bg-white rounded-xl border border-gray-200 p-5 mb-4">
          <h3 className="text-sm font-semibold text-gray-700 mb-3">新建端点</h3>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-3 mb-3">
            <div>
              <input
                placeholder="名称"
                value={form.name}
                onChange={e => setForm({ ...form, name: e.target.value })}
                className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
              />
            </div>
            <div>
              <div className="flex items-center gap-2">
                <span className="text-xs text-gray-400 font-mono whitespace-nowrap">/u/me/</span>
                <input
                  placeholder="路径前缀 (如 default)"
                  value={form.path_prefix}
                  onChange={e => setForm({ ...form, path_prefix: e.target.value })}
                  className="flex-1 px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
                />
              </div>
            </div>
            <select
              value={form.protocol}
              onChange={e => setForm({ ...form, protocol: e.target.value })}
              className="px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
            >
              {PROTOCOLS.map(p => <option key={p} value={p}>{p}</option>)}
            </select>
          </div>
          {form.path_prefix && (
            <p className="text-xs text-gray-400 mb-3">
              完整路径: <code className="text-indigo-500 bg-indigo-50 px-1.5 py-0.5 rounded">{`/u/me/${form.path_prefix.replace(/^\/+|\/+$/g, '')}` || '...'}</code>
            </p>
          )}
          <div className="flex gap-2">
            <button
              onClick={() => createMut.mutate(form)}
              disabled={createMut.isPending}
              className="px-4 py-2 bg-indigo-600 text-white text-sm rounded-lg hover:bg-indigo-700 disabled:opacity-50"
            >
              {createMut.isPending ? '创建中…' : '创建'}
            </button>
            <button onClick={() => setShowCreate(false)} className="px-4 py-2 text-gray-600 text-sm hover:bg-gray-100 rounded-lg">
              取消
            </button>
          </div>
        </div>
      )}

      {/* List */}
      <div className="bg-white rounded-xl border border-gray-200 overflow-hidden">
        {isLoading ? (
          <div className="p-5 text-sm text-gray-400">加载中…</div>
        ) : !data?.length ? (
          <div className="p-5 text-sm text-gray-400">暂无端点，点击「新建端点」开始</div>
        ) : (
          <table className="w-full">
            <thead>
              <tr className="border-b border-gray-100">
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">名称</th>
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">路径</th>
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">协议</th>
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">连接地址</th>
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">状态</th>
                <th className="text-right text-xs font-medium text-gray-400 uppercase px-5 py-3">操作</th>
              </tr>
            </thead>
            <tbody>
              {data.map((ep) => (
                <tr key={ep.id} className="border-b border-gray-50 hover:bg-gray-50/50">
                  <td className="px-5 py-3 text-sm font-medium text-gray-800">
                    {editingId === ep.id ? (
                      <input
                        value={editName}
                        onChange={e => setEditName(e.target.value)}
                        onKeyDown={e => { if (e.key === 'Enter') saveEdit(ep.id); if (e.key === 'Escape') setEditingId(null) }}
                        className="w-full px-2 py-1 border border-indigo-300 rounded text-sm focus:outline-none focus:ring-1 focus:ring-indigo-400"
                        autoFocus
                      />
                    ) : (
                      <span className="cursor-pointer hover:text-indigo-600" onClick={() => startEdit(ep)} title="点击编辑">
                        {ep.name}
                      </span>
                    )}
                  </td>
                  <td className="px-5 py-3">
                    {editingId === ep.id ? (
                      <div className="flex items-center gap-1">
                        <span className="text-xs text-gray-400 font-mono">/u/me/</span>
                        <input
                          value={editPathPrefix}
                          onChange={e => setEditPathPrefix(e.target.value)}
                          onKeyDown={e => { if (e.key === 'Enter') saveEdit(ep.id); if (e.key === 'Escape') setEditingId(null) }}
                          className="flex-1 px-2 py-1 border border-indigo-300 rounded text-sm focus:outline-none focus:ring-1 focus:ring-indigo-400"
                        />
                      </div>
                    ) : (
                      <code className="text-sm text-gray-500 bg-gray-50 px-1.5 py-0.5 rounded">{ep.listen_path}</code>
                    )}
                  </td>
                  <td className="px-5 py-3">
                    {editingId === ep.id ? (
                      <div className="flex items-center gap-1">
                        <select
                          value={editProtocol}
                          onChange={e => setEditProtocol(e.target.value)}
                          className="px-2 py-1 border border-indigo-300 rounded text-xs focus:outline-none focus:ring-1 focus:ring-indigo-400"
                        >
                          {PROTOCOLS.map(p => <option key={p} value={p}>{p}</option>)}
                        </select>
                        <button
                          onClick={() => saveEdit(ep.id)}
                          className="px-2 py-1 bg-indigo-600 text-white text-xs rounded hover:bg-indigo-700"
                        >
                          保存
                        </button>
                        <button
                          onClick={() => setEditingId(null)}
                          className="px-2 py-1 text-gray-500 text-xs hover:bg-gray-100 rounded"
                        >
                          取消
                        </button>
                      </div>
                    ) : (
                      <span className="inline-flex px-2 py-0.5 text-xs rounded-full bg-gray-100 text-gray-600 font-mono">
                        {ep.protocol}
                      </span>
                    )}
                  </td>
                  <td className="px-5 py-3">
                    <div className="flex items-center gap-1.5">
                      <code className="text-xs text-gray-400 bg-gray-50 px-1.5 py-0.5 rounded truncate max-w-[220px]">
                        {getProxyUrl(ep)}
                      </code>
                      <button
                        onClick={() => handleCopy(getProxyUrl(ep))}
                        className="p-1 text-gray-400 hover:text-indigo-500 hover:bg-indigo-50 rounded transition-colors flex-shrink-0"
                        title="复制连接地址"
                      >
                        {copiedUrl === getProxyUrl(ep) ? <Check size={14} className="text-green-500" /> : <Copy size={14} />}
                      </button>
                    </div>
                  </td>
                  <td className="px-5 py-3">
                    <button
                      onClick={() => updateMut.mutate({ id: ep.id, data: { enabled: !ep.enabled } })}
                      className="text-gray-400 hover:text-gray-600"
                    >
                      {ep.enabled ? <ToggleRight size={20} className="text-green-500" /> : <ToggleLeft size={20} />}
                    </button>
                  </td>
                  <td className="px-5 py-3">
                    <div className="flex items-center justify-end gap-2">
                      <button
                        onClick={() => navigate(`/endpoints/${ep.id}/keys`)}
                        className="p-1.5 text-gray-400 hover:text-indigo-500 hover:bg-indigo-50 rounded-lg transition-colors"
                        title="API Keys"
                      >
                        <Key size={16} />
                      </button>
                      <button
                        onClick={() => {
                          if (confirm('确定删除此端点？')) deleteMut.mutate(ep.id)
                        }}
                        className="p-1.5 text-gray-400 hover:text-red-500 hover:bg-red-50 rounded-lg transition-colors"
                        title="删除"
                      >
                        <Trash2 size={16} />
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  )
}
