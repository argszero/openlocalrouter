import { useState } from 'react'
import { useParams } from 'react-router-dom'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { getApiKeys, getEndpoints, getUsers, createApiKey, updateApiKey, deleteApiKey, getKeyUsage } from '../lib/api'
import { useAuth } from '../lib/auth'
import { toast } from 'sonner'
import { Plus, Trash2, Copy, Check, ToggleLeft, ToggleRight, ArrowLeft } from 'lucide-react'
import { Link } from 'react-router-dom'

export default function ApiKeysPage() {
  const { id } = useParams<{ id: string }>()
  const queryClient = useQueryClient()
  const { user } = useAuth()
  const { data: endpoints } = useQuery({ queryKey: ['endpoints'], queryFn: getEndpoints })
  const { data: users } = useQuery({ queryKey: ['users'], queryFn: getUsers })
  const { data: keys, isLoading } = useQuery({ queryKey: ['apiKeys', id], queryFn: () => getApiKeys(id!), enabled: !!id })
  const endpoint = endpoints?.find(e => e.id === id)
  const [showCreate, setShowCreate] = useState(false)
  const [name, setName] = useState('')
  const [assignedTo, setAssignedTo] = useState('')
  const [newKey, setNewKey] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)
  const [usageMap, setUsageMap] = useState<Record<string, number>>({})

  // Load monthly usage for each key
  useQuery({
    queryKey: ['keyUsageMonthly', id],
    queryFn: async () => {
      if (!id) return null
      const from = new Date()
      from.setDate(1)
      from.setHours(0, 0, 0, 0)
      const result = await getKeyUsage(id, {
        from: from.toISOString(),
        limit: 1000,
      })
      const map: Record<string, number> = {}
      for (const r of result.records) {
        map[r.api_key_id] = (map[r.api_key_id] || 0) + r.input_tokens + r.output_tokens
      }
      setUsageMap(map)
      return result
    },
    enabled: !!id,
    refetchInterval: 30_000,
  })

  const formatTokens = (n: number) => {
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
    if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`
    return String(n)
  }

  const createMut = useMutation({
    mutationFn: ({ name, assignedTo }: { name: string; assignedTo?: string }) =>
      createApiKey(id!, name, assignedTo || undefined),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['apiKeys', id] })
      toast.success('API Key 已创建')
      setNewKey(data.key)
      setName('')
      setAssignedTo('')
      setShowCreate(false)
    },
    onError: (err: Error) => toast.error(err.message),
  })

  const updateMut = useMutation({
    mutationFn: ({ keyId, data }: { keyId: string; data: { name?: string; enabled?: boolean; assigned_to?: string } }) =>
      updateApiKey(id!, keyId, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['apiKeys', id] })
      toast.success('已更新')
    },
    onError: (err: Error) => toast.error(err.message),
  })

  const deleteMut = useMutation({
    mutationFn: (keyId: string) => deleteApiKey(id!, keyId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['apiKeys', id] })
      toast.success('API Key 已删除')
    },
    onError: (err: Error) => toast.error(err.message),
  })

  const handleCopy = async (text: string) => {
    await navigator.clipboard.writeText(text)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
    toast.success('已复制到剪贴板')
  }

  const usernameById = (userId: string) => {
    const u = users?.find(u => u.id === userId)
    return u ? u.username : userId
  }

  return (
    <div>
      <div className="flex items-center gap-3 mb-6">
        <Link to="/endpoints" className="text-gray-400 hover:text-gray-600"><ArrowLeft size={20} /></Link>
        <div>
          <h2 className="text-2xl font-semibold text-gray-800">API Key 管理</h2>
          {endpoint && <p className="text-sm text-gray-400 mt-0.5">{endpoint.name} — {endpoint.listen_path}</p>}
        </div>
      </div>

      {/* New Key Display */}
      {newKey && (
        <div className="bg-green-50 border border-green-200 rounded-xl p-5 mb-4">
          <p className="text-sm font-medium text-green-800 mb-2">✅ API Key 已创建！请立即复制，之后将无法再次查看：</p>
          <div className="flex items-center gap-2">
            <code className="flex-1 px-3 py-2 bg-white border border-green-200 rounded text-sm font-mono text-green-900 break-all">
              {newKey}
            </code>
            <button
              onClick={() => handleCopy(newKey)}
              className="p-2 text-green-600 hover:bg-green-100 rounded-lg transition-colors"
            >
              {copied ? <Check size={18} /> : <Copy size={18} />}
            </button>
          </div>
        </div>
      )}

      <div className="flex items-center justify-between mb-4">
        <button
          onClick={() => setShowCreate(true)}
          className="flex items-center gap-2 px-4 py-2 bg-indigo-600 text-white text-sm font-medium rounded-lg hover:bg-indigo-700 transition-colors"
        >
          <Plus size={16} /> 生成 API Key
        </button>
      </div>

      {/* Create Form */}
      {showCreate && (
        <div className="bg-white rounded-xl border border-gray-200 p-4 mb-4">
          <div className="flex flex-col gap-3">
            <div className="flex items-center gap-3">
              <input
                placeholder='Key 名称 (如 "Chat App")'
                value={name}
                onChange={e => setName(e.target.value)}
                className="flex-1 px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
              />
              <select
                value={assignedTo}
                onChange={e => setAssignedTo(e.target.value)}
                className="px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500 min-w-[140px]"
              >
                <option value="">分配给自己</option>
                {users?.filter(u => u.enabled).map(u => (
                  <option key={u.id} value={u.id}>{u.username}</option>
                ))}
              </select>
              <button
                onClick={() => createMut.mutate({ name, assignedTo: assignedTo || undefined })}
                disabled={createMut.isPending || !name}
                className="px-4 py-2 bg-indigo-600 text-white text-sm rounded-lg hover:bg-indigo-700 disabled:opacity-50"
              >
                {createMut.isPending ? '生成中…' : '生成'}
              </button>
              <button onClick={() => { setShowCreate(false); setAssignedTo('') }} className="px-4 py-2 text-gray-600 text-sm hover:bg-gray-100 rounded-lg">取消</button>
            </div>
            <p className="text-xs text-gray-400">选择用户后，该 Key 将分配给对应用户使用。"分配给自己" 即自己使用。</p>
          </div>
        </div>
      )}

      {/* Key List */}
      <div className="bg-white rounded-xl border border-gray-200 overflow-hidden">
        {isLoading ? (
          <div className="p-5 text-sm text-gray-400">加载中…</div>
        ) : !keys?.length ? (
          <div className="p-5 text-sm text-gray-400">暂无 API Key</div>
        ) : (
          <table className="w-full">
            <thead>
              <tr className="border-b border-gray-100">
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">名称</th>
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">Key</th>
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">分配给</th>
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">状态</th>
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">创建时间</th>
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">最后使用</th>
                <th className="text-center text-xs font-medium text-gray-400 uppercase px-5 py-3">本月用量</th>
                <th className="text-right text-xs font-medium text-gray-400 uppercase px-5 py-3">操作</th>
              </tr>
            </thead>
            <tbody>
              {keys.map((k) => {
                const isOwner = user?.is_admin || user?.id === k.created_by
                return (
                <tr key={k.id} className="border-b border-gray-50 hover:bg-gray-50/50">
                  <td className="px-5 py-3 text-sm font-medium text-gray-800">{k.name}</td>
                  <td className="px-5 py-3">
                    <div className="flex items-center gap-2">
                      <code className="text-sm text-gray-400 font-mono">{k.key_prefix}</code>
                      <button
                        onClick={() => handleCopy(k.key_value)}
                        className="p-1 text-gray-400 hover:text-indigo-500 hover:bg-indigo-50 rounded transition-colors"
                        title="复制完整 Key"
                      >
                        {copied ? <Check size={14} /> : <Copy size={14} />}
                      </button>
                    </div>
                  </td>
                  <td className="px-5 py-3 text-sm text-gray-500">
                    {isOwner ? (
                      <select
                        value={k.assigned_to}
                        onChange={e => updateMut.mutate({ keyId: k.id, data: { assigned_to: e.target.value } })}
                        className="px-2 py-1 border border-gray-200 rounded text-sm bg-white focus:outline-none focus:ring-1 focus:ring-indigo-400 max-w-[140px]"
                      >
                        <option value={k.created_by}>{usernameById(k.created_by)}（自己）</option>
                        {users?.filter(u => u.enabled && u.id !== k.created_by).map(u => (
                          <option key={u.id} value={u.id}>{u.username}</option>
                        ))}
                      </select>
                    ) : (
                      <span className="text-sm text-gray-500">{usernameById(k.assigned_to)}</span>
                    )}
                  </td>
                  <td className="px-5 py-3">
                    {isOwner ? (
                      <button onClick={() => updateMut.mutate({ keyId: k.id, data: { enabled: !k.enabled } })}>
                        {k.enabled ? <ToggleRight size={20} className="text-green-500" /> : <ToggleLeft size={20} />}
                      </button>
                    ) : (
                      k.enabled ? <ToggleRight size={20} className="text-green-300" /> : <ToggleLeft size={20} className="text-gray-300" />
                    )}
                  </td>
                  <td className="px-5 py-3 text-sm text-gray-400">{k.created_at}</td>
                  <td className="px-5 py-3 text-sm text-gray-400">{k.last_used_at || '—'}</td>
                  <td className="px-5 py-3 text-center text-sm text-gray-500">
                    {formatTokens(usageMap[k.id] || 0)}
                  </td>
                  <td className="px-5 py-3">
                    <div className="flex justify-end">
                      {isOwner && (
                        <button
                          onClick={() => { if (confirm('删除此 Key？')) deleteMut.mutate(k.id) }}
                          className="p-1.5 text-gray-400 hover:text-red-500 hover:bg-red-50 rounded-lg transition-colors"
                        >
                          <Trash2 size={16} />
                        </button>
                      )}
                    </div>
                  </td>
                </tr>
                )
              })}
            </tbody>
          </table>
        )}
      </div>
    </div>
  )
}
