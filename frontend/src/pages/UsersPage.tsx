import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { getUsers, createUser, updateUser, deleteUser } from '../lib/api'
import type { User } from '../lib/api'
import { useAuth } from '../lib/auth'
import { toast } from 'sonner'
import { Plus, Trash2, ToggleLeft, ToggleRight, Shield, UserIcon, Key, Pencil, Check, X } from 'lucide-react'
import { useI18n, t } from '../lib/i18n'

export default function UsersPage() {
  const queryClient = useQueryClient()
  const { user: currentUser } = useAuth()
  const { data: users, isLoading } = useQuery({ queryKey: ['users'], queryFn: getUsers })
  const [showCreate, setShowCreate] = useState(false)
  const [showPasswordChange, setShowPasswordChange] = useState(false)
  const [passwordForm, setPasswordForm] = useState({ newPassword: '', confirmPassword: '' })
  const [form, setForm] = useState({ username: '', password: '', is_admin: false })
  const [editingUser, setEditingUser] = useState<string | null>(null)
  const [editingUsername, setEditingUsername] = useState('')
  const [resettingPw, setResettingPw] = useState<string | null>(null)
  const [newUserPw, setNewUserPw] = useState('')
  const { lang } = useI18n()

  const createMut = useMutation({
    mutationFn: createUser,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['users'] })
      toast.success(t(lang, 'users.created'))
      setShowCreate(false)
      setForm({ username: '', password: '', is_admin: false })
    },
    onError: (err: Error) => toast.error(err.message),
  })

  const updateMut = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Partial<User> & { password?: string } }) => updateUser(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['users'] })
      toast.success(t(lang, 'users.updated'))
    },
    onError: (err: Error) => toast.error(err.message),
  })

  const deleteMut = useMutation({
    mutationFn: deleteUser,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['users'] })
      toast.success(t(lang, 'users.deleted'))
    },
    onError: (err: Error) => toast.error(err.message),
  })

  const passwordMut = useMutation({
    mutationFn: ({ id, password }: { id: string; password: string }) => updateUser(id, { password }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['users'] })
      toast.success(t(lang, 'users.password_changed'))
      setShowPasswordChange(false)
      setPasswordForm({ newPassword: '', confirmPassword: '' })
    },
    onError: (err: Error) => toast.error(err.message),
  })

  const handleChangePassword = () => {
    if (!passwordForm.newPassword) { toast.error(t(lang, 'users.enter_new_password')); return }
    if (passwordForm.newPassword !== passwordForm.confirmPassword) { toast.error(t(lang, 'users.password_mismatch')); return }
    if (!currentUser) return
    passwordMut.mutate({ id: currentUser.id, password: passwordForm.newPassword })
  }

  const handleSaveUsername = (id: string) => {
    if (!editingUsername.trim()) { toast.error(t(lang, 'users.username_required')); return }
    updateMut.mutate({ id, data: { username: editingUsername.trim() } })
    setEditingUser(null)
  }

  const handleResetPassword = (id: string) => {
    if (!newUserPw) { toast.error(t(lang, 'users.enter_new_password')); return }
    updateMut.mutate({ id, data: { password: newUserPw } })
    setResettingPw(null)
    setNewUserPw('')
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-2xl font-semibold text-gray-800">{t(lang, 'users.title')}</h2>
        {currentUser?.is_admin && (
          <button
            onClick={() => setShowCreate(true)}
            className="flex items-center gap-2 px-4 py-2 bg-indigo-600 text-white text-sm font-medium rounded-lg hover:bg-indigo-700 transition-colors"
          >
            <Plus size={16} /> {t(lang, 'users.new_user')}
          </button>
        )}
      </div>

      {/* Create Form */}
      {showCreate && (
        <div className="bg-white rounded-xl border border-gray-200 p-5 mb-4">
          <h3 className="text-sm font-semibold text-gray-700 mb-3">{t(lang, 'users.new_user')}</h3>
          <div className="flex flex-wrap gap-3 mb-3">
            <input
              placeholder={t(lang, 'users.username')}
              value={form.username}
              onChange={e => setForm({ ...form, username: e.target.value })}
              className="px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
            />
            <input
              type="password"
              placeholder={t(lang, 'users.password')}
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
              {t(lang, 'users.admin')}
            </label>
          </div>
          <div className="flex gap-2">
            <button
              onClick={() => createMut.mutate(form)}
              disabled={createMut.isPending}
              className="px-4 py-2 bg-indigo-600 text-white text-sm rounded-lg hover:bg-indigo-700 disabled:opacity-50"
            >
              {createMut.isPending ? t(lang, 'users.creating') : t(lang, 'users.create')}
            </button>
            <button onClick={() => setShowCreate(false)} className="px-4 py-2 text-gray-600 text-sm hover:bg-gray-100 rounded-lg">{t(lang, 'common.cancel')}</button>
          </div>
        </div>
      )}

      {/* List */}
      <div className="bg-white rounded-xl border border-gray-200 overflow-hidden">
        {isLoading ? (
          <div className="p-5 text-sm text-gray-400">{t(lang, 'common.loading')}</div>
        ) : (
          <table className="w-full">
            <thead>
              <tr className="border-b border-gray-100">
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">{t(lang, 'users.username')}</th>
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">{t(lang, 'users.role')}</th>
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">{t(lang, 'common.status')}</th>
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">{t(lang, 'keys.created_at')}</th>
                <th className="text-right text-xs font-medium text-gray-400 uppercase px-5 py-3">{t(lang, 'common.actions')}</th>
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
                      {editingUser === u.id ? (
                        <div className="flex items-center gap-1">
                          <input
                            value={editingUsername}
                            onChange={e => setEditingUsername(e.target.value)}
                            onKeyDown={e => { if (e.key === 'Enter') handleSaveUsername(u.id); if (e.key === 'Escape') setEditingUser(null) }}
                            className="px-2 py-1 border border-indigo-300 rounded text-sm focus:outline-none focus:ring-1 focus:ring-indigo-400 w-32"
                            autoFocus
                          />
                          <button onClick={() => handleSaveUsername(u.id)} className="p-1 text-green-600 hover:bg-green-50 rounded"><Check size={14} /></button>
                          <button onClick={() => setEditingUser(null)} className="p-1 text-gray-400 hover:bg-gray-100 rounded"><X size={14} /></button>
                        </div>
                      ) : (
                        <span className="text-sm font-medium text-gray-800">{u.username}</span>
                      )}
                      {u.id === currentUser?.id && (
                        <span className="text-xs text-indigo-500">{t(lang, 'users.you')}</span>
                      )}
                      {currentUser?.is_admin && u.id !== currentUser?.id && editingUser !== u.id && (
                        <button
                          onClick={() => { setEditingUser(u.id); setEditingUsername(u.username) }}
                          className="p-0.5 text-gray-400 hover:text-indigo-500 hover:bg-indigo-50 rounded"
                          title={t(lang, 'users.edit_username')}
                        >
                          <Pencil size={12} />
                        </button>
                      )}
                    </div>
                  </td>
                  <td className="px-5 py-3">
                    <span className={`inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded-full ${
                      u.is_admin ? 'bg-purple-50 text-purple-700' : 'bg-gray-100 text-gray-600'
                    }`}>
                      {u.is_admin ? <Shield size={12} /> : <UserIcon size={12} />}
                      {u.is_admin ? t(lang, 'users.admin_role') : t(lang, 'users.user_role')}
                    </span>
                  </td>
                  <td className="px-5 py-3">
                    {currentUser?.is_admin && u.id !== currentUser.id ? (
                      <button onClick={() => updateMut.mutate({ id: u.id, data: { enabled: !u.enabled } })}>
                        {u.enabled ? <ToggleRight size={20} className="text-green-500" /> : <ToggleLeft size={20} />}
                      </button>
                    ) : (
                      u.enabled ? <span className="text-xs text-green-600">{t(lang, 'users.enabled')}</span> : <span className="text-xs text-red-500">{t(lang, 'users.disabled')}</span>
                    )}
                  </td>
                  <td className="px-5 py-3 text-sm text-gray-400">{u.created_at}</td>
                  <td className="px-5 py-3">
                    <div className="flex justify-end items-center gap-1">
                      {currentUser?.is_admin && u.id !== currentUser.id && (
                        <>
                          {resettingPw === u.id ? (
                            <div className="flex items-center gap-1">
                              <input
                                type="password"
                                placeholder={t(lang, 'users.new_password')}
                                value={newUserPw}
                                onChange={e => setNewUserPw(e.target.value)}
                                onKeyDown={e => { if (e.key === 'Enter') handleResetPassword(u.id); if (e.key === 'Escape') { setResettingPw(null); setNewUserPw('') } }}
                                className="w-24 px-2 py-1 border border-indigo-300 rounded text-xs focus:outline-none focus:ring-1 focus:ring-indigo-400"
                                autoFocus
                              />
                              <button onClick={() => handleResetPassword(u.id)} className="p-1 text-green-600 hover:bg-green-50 rounded"><Check size={14} /></button>
                              <button onClick={() => { setResettingPw(null); setNewUserPw('') }} className="p-1 text-gray-400 hover:bg-gray-100 rounded"><X size={14} /></button>
                            </div>
                          ) : (
                            <button
                              onClick={() => setResettingPw(u.id)}
                              className="p-1.5 text-gray-400 hover:text-indigo-500 hover:bg-indigo-50 rounded-lg transition-colors"
                              title={t(lang, 'users.reset_password')}
                            >
                              <Key size={16} />
                            </button>
                          )}
                          <button
                            onClick={() => { if (confirm(t(lang, 'users.delete_confirm'))) deleteMut.mutate(u.id) }}
                            className="p-1.5 text-gray-400 hover:text-red-500 hover:bg-red-50 rounded-lg transition-colors"
                          >
                            <Trash2 size={16} />
                          </button>
                        </>
                      )}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {/* Change Password */}
      <div className="mt-6 bg-white rounded-xl border border-gray-200 p-5">
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-semibold text-gray-700">{t(lang, 'users.change_password')}</h3>
          {!showPasswordChange && (
            <button
              onClick={() => setShowPasswordChange(true)}
              className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-indigo-600 bg-indigo-50 rounded-lg hover:bg-indigo-100 transition-colors"
            >
              <Key size={14} /> {t(lang, 'users.change_password')}
            </button>
          )}
        </div>
        {showPasswordChange && (
          <div className="mt-3 flex flex-wrap gap-3">
            <input
              type="password"
              placeholder={t(lang, 'users.new_password')}
              value={passwordForm.newPassword}
              onChange={e => setPasswordForm({ ...passwordForm, newPassword: e.target.value })}
              className="px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
            />
            <input
              type="password"
              placeholder={t(lang, 'users.confirm_password')}
              value={passwordForm.confirmPassword}
              onChange={e => setPasswordForm({ ...passwordForm, confirmPassword: e.target.value })}
              className="px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
            />
            <button
              onClick={handleChangePassword}
              disabled={passwordMut.isPending}
              className="px-4 py-2 bg-indigo-600 text-white text-sm rounded-lg hover:bg-indigo-700 disabled:opacity-50"
            >
              {passwordMut.isPending ? t(lang, 'users.changing') : t(lang, 'users.confirm_change')}
            </button>
            <button
              onClick={() => { setShowPasswordChange(false); setPasswordForm({ newPassword: '', confirmPassword: '' }) }}
              className="px-4 py-2 text-gray-600 text-sm hover:bg-gray-100 rounded-lg"
            >
              {t(lang, 'common.cancel')}
            </button>
          </div>
        )}
      </div>
    </div>
  )
}
