import { useQuery } from '@tanstack/react-query'
import { getDashboard } from '../lib/api'
import { Globe, Server, Key, Zap, Users, Share2, Gift, TrendingUp } from 'lucide-react'
import { useAuth } from '../lib/auth'

function formatTokens(n: number) {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`
  return String(n)
}

export default function DashboardPage() {
  const { user } = useAuth()
  const { data, isLoading } = useQuery({ queryKey: ['dashboard'], queryFn: getDashboard, refetchInterval: 30_000 })

  if (isLoading) return <div className="text-gray-400 text-sm">加载中…</div>

  const s = data || { my_providers: 0, my_endpoints: 0, my_keys: 0, keys_assigned_to_others: 0, keys_assigned_to_me: 0, shared_endpoints: 0, today_my_tokens: 0, today_shared_tokens: 0 }

  return (
    <div>
      <h2 className="text-2xl font-semibold text-gray-800 mb-6">仪表板</h2>

      <div className="grid grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
        <Card icon={<Server size={20} />} label="我的 Provider" value={String(s.my_providers)} color="indigo" />
        <Card icon={<Globe size={20} />} label="我的 Endpoint" value={String(s.my_endpoints)} color="blue" />
        <Card icon={<Key size={20} />} label="我的 Key" value={String(s.my_keys)} color="amber" />
        <Card icon={<Share2 size={20} />} label="分享端点" value={String(s.shared_endpoints)} color="purple" />
        <Card icon={<Zap size={20} />} label="今日自己消耗" value={formatTokens(s.today_my_tokens)} color="emerald" />
        <Card icon={<TrendingUp size={20} />} label="今日分享消耗" value={formatTokens(s.today_shared_tokens)} color="rose" />
        <Card icon={<Gift size={20} />} label="收到 Key" value={String(s.keys_assigned_to_me)} color="cyan" />
        <Card icon={<Users size={20} />} label="发出 Key" value={String(s.keys_assigned_to_others)} color="orange" />
      </div>

      <div className="p-4 bg-white rounded-xl border border-gray-200">
        <h3 className="text-sm font-medium text-gray-700 mb-2">服务状态</h3>
        <div className="flex items-center gap-2">
          <span className="w-2 h-2 rounded-full bg-green-500" />
          <span className="text-sm text-gray-500">运行中 — 端口 19528</span>
          <span className="text-xs text-gray-300 ml-2">{user?.username} · {user?.is_admin ? 'Admin' : 'User'}</span>
        </div>
      </div>
    </div>
  )
}

function Card({ icon, label, value, color }: { icon: React.ReactNode; label: string; value: string; color: string }) {
  const c: Record<string, string> = {
    indigo: 'bg-indigo-50 text-indigo-600', blue: 'bg-blue-50 text-blue-600',
    emerald: 'bg-emerald-50 text-emerald-600', amber: 'bg-amber-50 text-amber-600',
    purple: 'bg-purple-50 text-purple-600', rose: 'bg-rose-50 text-rose-600',
    cyan: 'bg-cyan-50 text-cyan-600', orange: 'bg-orange-50 text-orange-600',
  }
  return (
    <div className="bg-white rounded-xl border border-gray-200 p-5">
      <div className={`w-10 h-10 rounded-lg ${c[color] || c.indigo} flex items-center justify-center mb-3`}>
        {icon}
      </div>
      <p className="text-2xl font-bold text-gray-800">{value}</p>
      <p className="text-sm text-gray-400 mt-0.5">{label}</p>
    </div>
  )
}
