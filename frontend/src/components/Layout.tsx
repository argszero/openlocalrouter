import { NavLink, Outlet, useNavigate } from 'react-router-dom'
import { useAuth } from '../lib/auth'
import { getStatus } from '../lib/api'
import { LayoutDashboard, Globe, Server, Users, BarChart3, LogOut, Languages } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useI18n, t, type MessageKey } from '../lib/i18n'

const navItems = [
  { to: '/dashboard', icon: LayoutDashboard, label: 'nav.dashboard' },
  { to: '/endpoints', icon: Globe, label: 'nav.endpoints' },
  { to: '/providers', icon: Server, label: 'nav.providers' },
  { to: '/usage/my', icon: BarChart3, label: 'nav.usage.my' },
  { to: '/usage/shared', icon: BarChart3, label: 'nav.usage.shared' },
  { to: '/users', icon: Users, label: 'nav.users' },
] as const

export default function Layout() {
  const { user, logout } = useAuth()
  const navigate = useNavigate()
  const { lang, setLang } = useI18n()
  const [version, setVersion] = useState<string>('')

  useEffect(() => {
    getStatus().then(s => setVersion(s.version)).catch(() => {})
  }, [])

  const handleLogout = () => {
    logout()
    navigate('/login')
  }

  return (
    <div className="flex h-screen bg-gray-50">
      {/* Sidebar */}
      <aside className="w-56 bg-white border-r border-gray-200 flex flex-col shrink-0">
        <div className="p-4 border-b border-gray-100">
          <h1 className="text-lg font-semibold text-gray-800">
            OLR
            {version && <span className="ml-1.5 text-xs font-normal text-gray-400 align-middle">v{version}</span>}
          </h1>
          <p className="text-xs text-gray-400 mt-0.5">{t(lang, 'app.console')}</p>
        </div>
        <nav className="flex-1 p-3 space-y-1">
          {navItems.map(({ to, icon: Icon, label }) => (
            <NavLink
              key={to}
              to={to}
              className={({ isActive }) =>
                `flex items-center gap-3 px-3 py-2 rounded-lg text-sm font-medium transition-colors ${
                  isActive
                    ? 'bg-indigo-50 text-indigo-700'
                    : 'text-gray-600 hover:bg-gray-100 hover:text-gray-900'
                }`
              }
            >
              <Icon size={18} />
              {t(lang, label as MessageKey)}
            </NavLink>
          ))}
        </nav>
        <div className="p-3 border-t border-gray-100">
          <div className="flex items-center justify-between px-3 py-1">
            <button
              onClick={() => setLang(lang === 'zh' ? 'en' : 'zh')}
              className="flex items-center gap-2 text-xs text-gray-400 hover:text-indigo-600 transition-colors"
              title={t(lang, 'action.language')}
            >
              <Languages size={14} />
              {lang === 'zh' ? 'EN' : '中文'}
            </button>
          </div>
          <div className="flex items-center gap-3 px-3 py-2">
            <div className="w-8 h-8 rounded-full bg-indigo-100 flex items-center justify-center text-indigo-700 text-sm font-semibold">
              {user?.username?.[0]?.toUpperCase() || '?'}
            </div>
            <div className="flex-1 min-w-0">
              <p className="text-sm font-medium text-gray-700 truncate">{user?.username}</p>
              <p className="text-xs text-gray-400">{user?.is_admin ? t(lang, 'role.admin') : t(lang, 'role.user')}</p>
            </div>
            <button onClick={handleLogout} className="text-gray-400 hover:text-red-500 transition-colors" title={t(lang, 'action.logout')}>
              <LogOut size={16} />
            </button>
          </div>
        </div>
      </aside>

      {/* Main content */}
      <main className="flex-1 overflow-auto">
        <div className="p-6 max-w-5xl">
          <Outlet />
        </div>
      </main>
    </div>
  )
}
