import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { getUsers, createUser, updateUser, deleteUser } from '../lib/api'
import type { User } from '../lib/api'
import { useAuth } from '../lib/auth'
import { toast } from 'sonner'
import { Plus, Trash2, ToggleLeft, ToggleRight, Shield, UserIcon } from 'lucide-react'

export default function UsersPage() {
  const queryClient = useQueryClient()
  const { user: currentUser } = useAuth()
  const { data: users, isLoading } = useQuery({ queryKey: ['users'], queryFn: getUsers })
  const [showCreate, setShowCreate] = useState(false)
  const [form, setForm] = useState({ username: '', password: '', is_admin: false })

  const createMut = useMutation({
    mutationFn: createUser,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['users'] })
      toast.success('用户已创建')
      setShowCreate(false)
      setForm({ username: '', password: '', is_admin: false })
    },
    onError: (err: Error) => toast.error(err.message),
  })

  const updateMut = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Partial<User> & { password?: string } }) => updateUser(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['users'] })
      toast.success('用户已更新')
    },
    onError: (err: Error) => toast.error(err.message),
  })

  const deleteMut = useMutation({
    mutationFn: deleteUser,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['users'] })
      toast.success('用户已删除')
    },
    onError: (err: Error) => toast.error(err.message),
  })

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-2xl font-semibold text-gray-800">用户管理</h2>
        {currentUser?.is_admin && (
          <button
            onClick={() => setShowCreate(true)}
            className="flex items-center gap-2 px-4 py-2 bg-indigo-600 text-white text-sm font-medium rounded-lg hover:bg-indigo-700 transition-colors"
          >
            <Plus size={16} /> 新建用户
          </button>
        )}
      </div>

      {/* Create Form */}
      {showCreate && (
        <div className="bg-white rounded-xl border border-gray-200 p-5 mb-4">
          <h3 className="text-sm font-semibold text-gray-700 mb-3">新建用户</h3>
          <div className="flex flex-wrap gap-3 mb-3">
            <input
              placeholder="用户名"
              value={form.username}
              onChange={e => setForm({ ...form, username: e.target.value })}
              className="px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
            />
            <input
              type="password"
              placeholder="密码"
              value={form.password}
              onChange={e => setForm({ ...form, password: e.target.value })}
              className="px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
            />
            <label className="flex items-center gap-2 text-sm text-gray-600">
              <input
                type="checkbox"
                checked={form.is_admin}
                onChange={e => setForm({ ...form, is_admin: e.target.checked })}
              />
              管理员
            </label>
          </div>
          <div className="flex gap-2">
            <button
              onClick={() => createMut.mutate(form)}
              disabled={createMut.isPending}
              className="px-4 py-2 bg-indigo-600 text-white text-sm rounded-lg hover:bg-indigo-700 disabled:opacity-50"
            >
              {createMut.isPending ? '创建中…' : '创建'}
            </button>
            <button onClick={() => setShowCreate(false)} className="px-4 py-2 text-gray-600 text-sm hover:bg-gray-100 rounded-lg">取消</button>
          </div>
        </div>
      )}

      {/* List */}
      <div className="bg-white rounded-xl border border-gray-200 overflow-hidden">
        {isLoading ? (
          <div className="p-5 text-sm text-gray-400">加载中…</div>
        ) : (
          <table className="w-full">
            <thead>
              <tr className="border-b border-gray-100">
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">用户</th>
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">角色</th>
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">状态</th>
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">创建时间</th>
                <th className="text-right text-xs font-medium text-gray-400 uppercase px-5 py-3">操作</th>
              </tr>
            </thead>
            <tbody>
              {users?.map((u) => (
                <tr key={u.id} className="border-b border-gray-50 hover:bg-gray-50/50">
                  <td className="px-5 py-3">
                    <div className="flex items-center gap-3">
                      <div className="w-8 h-8 rounded-full bg-gray-100 flex items-center justify-center text-sm font-medium text-gray-600">
                        {u.username[0]?.toUpperCase()}
                      </div>
                      <span className="text-sm font-medium text-gray-800">{u.username}</span>
                      {u.id === currentUser?.id && (
                        <span className="text-xs text-indigo-500">(你)</span>
                      )}
                    </div>
                  </td>
                  <td className="px-5 py-3">
                    <span className={`inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded-full ${
                      u.is_admin ? 'bg-purple-50 text-purple-700' : 'bg-gray-100 text-gray-600'
                    }`}>
                      {u.is_admin ? <Shield size={12} /> : <UserIcon size={12} />}
                      {u.is_admin ? '管理员' : '普通用户'}
                    </span>
                  </td>
                  <td className="px-5 py-3">
                    {currentUser?.is_admin && u.id !== currentUser.id ? (
                      <button onClick={() => updateMut.mutate({ id: u.id, data: { enabled: !u.enabled } })}>
                        {u.enabled ? <ToggleRight size={20} className="text-green-500" /> : <ToggleLeft size={20} />}
                      </button>
                    ) : (
                      u.enabled ? <span className="text-xs text-green-600">启用</span> : <span className="text-xs text-red-500">禁用</span>
                    )}
                  </td>
                  <td className="px-5 py-3 text-sm text-gray-400">{u.created_at}</td>
                  <td className="px-5 py-3">
                    <div className="flex justify-end">
                      {currentUser?.is_admin && u.id !== currentUser.id && (
                        <button
                          onClick={() => { if (confirm('删除用户？')) deleteMut.mutate(u.id) }}
                          className="p-1.5 text-gray-400 hover:text-red-500 hover:bg-red-50 rounded-lg transition-colors"
                        >
                          <Trash2 size={16} />
                        </button>
                      )}
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
