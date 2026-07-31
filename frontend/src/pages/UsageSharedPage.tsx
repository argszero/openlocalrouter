import { useState, useCallback } from 'react'
import { useQuery } from '@tanstack/react-query'
import { getSharedSummary, getSharedTop, getSharedKeys, getSharedRecords } from '../lib/api'
import { Zap, Key, TrendingUp, MousePointerClick, Calendar, ChevronLeft, ChevronRight, Users, Filter, XCircle } from 'lucide-react'
import { useI18n, t, tf } from '../lib/i18n'

function formatTokens(n: number) { if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M'; if (n >= 1_000) return (n / 1_000).toFixed(1) + 'k'; return String(n) }
function formatDate(s: string) { return s.replace('T', ' ').slice(0, 16) }
function todayStr() { return new Date().toISOString().slice(0, 10) }
function daysAgo(n: number) { const d = new Date(); d.setDate(d.getDate() - n); return d.toISOString().slice(0, 10) }

type ColumnFilter = { model: string; user: string; provider: string; key: string }

export default function UsageSharedPage() {
  const [dateFrom, setDateFrom] = useState(daysAgo(7))
  const [dateTo, setDateTo] = useState(todayStr())
  const [page, setPage] = useState(0)
  const [filters, setFilters] = useState<ColumnFilter>({ model: '', user: '', provider: '', key: '' })
  const [showFilters, setShowFilters] = useState(false)
  const { lang } = useI18n()
  const limit = 25

  const { data: summary } = useQuery({ queryKey: ['sharedSummary', dateFrom, dateTo], queryFn: () => getSharedSummary(dateFrom, dateTo), refetchInterval: 30000 })
  const { data: topCustomers } = useQuery({ queryKey: ['sharedTopCust', dateFrom, dateTo], queryFn: () => getSharedTop('key', dateFrom, dateTo), refetchInterval: 30000 })
  const { data: topModels } = useQuery({ queryKey: ['sharedTopModel', dateFrom, dateTo], queryFn: () => getSharedTop('model', dateFrom, dateTo), refetchInterval: 30000 })
  const { data: keysData } = useQuery({ queryKey: ['sharedKeys'], queryFn: getSharedKeys, refetchInterval: 30000 })
  const { data: records, isLoading } = useQuery({
    queryKey: ['sharedRecords', page, dateFrom, dateTo, filters],
    queryFn: () => getSharedRecords({
      from: dateFrom, to: dateTo,
      model: filters.model || undefined,
      user_id: filters.user || undefined,
      provider: filters.provider || undefined,
      key_name: filters.key || undefined,
      limit, offset: page * limit,
    }),
    refetchInterval: 30000,
  })

  const hasActiveFilters = Object.values(filters).some(v => v !== '')

  const setFilter = useCallback((col: keyof ColumnFilter, value: string) => {
    setFilters(f => ({ ...f, [col]: value }))
    setPage(0)
  }, [])

  const clearFilters = useCallback(() => {
    setFilters({ model: '', user: '', provider: '', key: '' })
    setPage(0)
  }, [])

  const totalPages = records ? Math.ceil(records.total / limit) : 0
  const s = summary || { today_tokens: 0, yesterday_tokens: 0, trend_pct: 0, active_keys: 0, total_keys: 0, active_users: 0 }

  return (
    <div>
      <h2 className="text-2xl font-semibold text-gray-800 mb-4">{t(lang, 'usageshared.title')}</h2>
      <div className="flex items-center gap-2 mb-4">
        <Calendar size={14} className="text-gray-400" />
        <input type="date" value={dateFrom} onChange={e => { setDateFrom(e.target.value); setPage(0) }} className="px-2 py-1 border border-gray-300 rounded text-xs" />
        <span className="text-gray-300">{'—'}</span>
        <input type="date" value={dateTo} onChange={e => { setDateTo(e.target.value); setPage(0) }} className="px-2 py-1 border border-gray-300 rounded text-xs" />
        <button onClick={() => { setDateFrom(daysAgo(7)); setDateTo(todayStr()) }} className="px-2 py-1 text-xs border rounded hover:bg-gray-50">{t(lang, 'usage.days7')}</button>
        <button onClick={() => { setDateFrom(daysAgo(30)); setDateTo(todayStr()) }} className="px-2 py-1 text-xs border rounded hover:bg-gray-50">{t(lang, 'usage.days30')}</button>
      </div>

      <div className="grid grid-cols-2 md:grid-cols-6 gap-3 mb-6">
        <StatC icon={<Zap size={16} />} label={t(lang, 'usageshared.today_consumption')} value={formatTokens(s.today_tokens)} color="indigo" />
        <StatC icon={<TrendingUp size={16} />} label={t(lang, 'usageshared.vs_yesterday')} value={(s.trend_pct >= 0 ? '\u2191' : '\u2193') + Math.abs(s.trend_pct).toFixed(1) + '%'} color={s.trend_pct >= 0 ? 'emerald' : 'rose'} />
        <StatC icon={<Key size={16} />} label={t(lang, 'usageshared.active_keys')} value={s.active_keys + '/' + s.total_keys} color="amber" />
        <StatC icon={<Users size={16} />} label={t(lang, 'usageshared.active_users')} value={String(s.active_users)} color="blue" />
        <StatC icon={<MousePointerClick size={16} />} label={t(lang, 'usageshared.total_keys')} value={String(s.total_keys)} color="purple" />
        <StatC icon={<Users size={16} />} label="" value="" color="cyan" />
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-6">
        <div className="bg-white rounded-xl border p-4">
          <h3 className="text-sm font-semibold text-gray-700 mb-2">{t(lang, 'usageshared.top_customers')}</h3>
          {(topCustomers?.groups || []).slice(0, 8).map((g, i) => (
            <div key={g.key} className="flex items-center gap-2 py-1.5 text-xs">
              <span className="w-5 text-gray-400 text-right">{i + 1}</span>
              <span className="flex-1 text-gray-700 truncate">{g.key}</span>
              <span className="text-gray-500">{formatTokens(g.total_input_tokens + g.total_output_tokens)}</span>
              <span className="text-gray-300">{tf(lang, 'usage.count_suffix', { n: g.count })}</span>
            </div>
          ))}
        </div>
        <div className="bg-white rounded-xl border p-4">
          <h3 className="text-sm font-semibold text-gray-700 mb-2">{t(lang, 'usageshared.top_models')}</h3>
          {(topModels?.groups || []).slice(0, 8).map((g, i) => (
            <div key={g.key} className="flex items-center gap-2 py-1.5 text-xs">
              <span className="w-5 text-gray-400 text-right">{i + 1}</span>
              <span className="flex-1 text-gray-700 truncate">{g.key}</span>
              <span className="text-gray-500">{formatTokens(g.total_input_tokens + g.total_output_tokens)}</span>
              <span className="text-gray-300">{tf(lang, 'usage.count_suffix', { n: g.count })}</span>
            </div>
          ))}
        </div>
      </div>

      <div className="bg-white rounded-xl border overflow-hidden mb-6">
        <div className="px-5 py-3 border-b bg-gray-50/50">
          <h3 className="text-sm font-semibold text-gray-700">{t(lang, 'usageshared.key_status')}</h3>
        </div>
        <table className="w-full">
          <thead><tr className="border-b border-gray-100">
            <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-2">{t(lang, 'keys.name_col')}</th>
            <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-2">{t(lang, 'keys.assigned_to')}</th>
            <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-2">{t(lang, 'keys.last_used')}</th>
          </tr></thead>
          <tbody>
            {(keysData?.keys || []).map(k => (
              <tr key={k.id} className="border-b border-gray-50 hover:bg-gray-50/50">
                <td className="px-5 py-2 text-sm text-gray-700">{k.name}</td>
                <td className="px-5 py-2 text-sm text-gray-600">{k.assigned_to}</td>
                <td className="px-5 py-2 text-sm text-gray-400">{k.last_used_at ? formatDate(k.last_used_at) : t(lang, 'usageshared.never_used')}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="bg-white rounded-xl border overflow-hidden">
        <div className="px-5 py-3 border-b flex items-center justify-between bg-gray-50/50">
          <div className="flex items-center gap-2">
            <h3 className="text-sm font-semibold text-gray-700">{t(lang, 'usageshared.consumption_detail')}</h3>
            {records && <span className="text-xs text-gray-400">{tf(lang, 'usage.total_records', { n: records.total })}</span>}
          </div>
          <button
            onClick={() => { setShowFilters(!showFilters); if (!showFilters) clearFilters() }}
            className={`flex items-center gap-1 px-2 py-1 text-xs border rounded transition-colors ${
              showFilters ? 'bg-indigo-50 border-indigo-300 text-indigo-700' : 'hover:bg-gray-50 text-gray-500'
            }`}
          >
            <Filter size={12} />
            {t(lang, 'usage.filter')}
            {hasActiveFilters && <span className="w-1.5 h-1.5 rounded-full bg-indigo-500" />}
          </button>
        </div>

        {showFilters && (
          <div className="px-5 py-2 border-b bg-gray-50/30">
            <FilterRow filters={filters} setFilter={setFilter} clearFilters={clearFilters} />
          </div>
        )}

        {isLoading ? <div className="p-5 text-sm text-gray-400">{t(lang, 'common.loading')}</div> : !records?.records?.length ? (
          <div className="p-8 text-center text-sm text-gray-400">{t(lang, 'usage.no_data')}</div>
        ) : (
          <table className="w-full">
            <thead><tr className="border-b border-gray-100">
              <ThCell>{t(lang, 'usage.time')}</ThCell>
              <ThCell>{t(lang, 'usageshared.user')}</ThCell>
              <ThCell>Key</ThCell>
              <ThCell>{t(lang, 'usage.model')}</ThCell>
              <ThCell>Provider</ThCell>
              <ThCell align="right">Input</ThCell>
              <ThCell align="right">Output</ThCell>
              <ThCell align="right">Total</ThCell>
            </tr></thead>
            <tbody>
              {(records.records || []).map(r => (
                <tr key={r.id} className="border-b border-gray-50 hover:bg-gray-50/50">
                  <td className="px-5 py-2 text-sm text-gray-400 whitespace-nowrap">{formatDate(r.created_at)}</td>
                  <td className="px-5 py-2 text-sm text-gray-700" title={r.user_id}>{r.user_name || r.user_id || '—'}</td>
                  <td className="px-5 py-2 text-sm text-gray-600" title={r.api_key_id}>{r.key_name || r.api_key_id.slice(0, 12) + '...' || '—'}</td>
                  <td className="px-5 py-2 text-sm font-mono text-gray-700">{r.model}</td>
                  <td className="px-5 py-2 text-sm text-gray-500">{r.provider_name}</td>
                  <td className="px-5 py-2 text-sm text-gray-600 text-right">{formatTokens(r.input_tokens)}</td>
                  <td className="px-5 py-2 text-sm text-gray-600 text-right">{formatTokens(r.output_tokens)}</td>
                  <td className="px-5 py-2 text-sm font-medium text-gray-700 text-right">{formatTokens(r.input_tokens + r.output_tokens)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {totalPages > 1 && (
          <div className="px-5 py-3 border-t flex items-center justify-between bg-gray-50/30">
            <button onClick={() => setPage(p => Math.max(0, p - 1))} disabled={page === 0} className="flex items-center gap-1 px-3 py-1.5 text-sm bg-white border rounded-lg disabled:opacity-30"><ChevronLeft size={14} /> {t(lang, 'usage.prev')}</button>
            <span className="text-xs text-gray-400">{tf(lang, 'usage.page_info', { cur: page + 1, total: totalPages })}</span>
            <button onClick={() => setPage(p => Math.min(totalPages - 1, p + 1))} disabled={page >= totalPages - 1} className="flex items-center gap-1 px-3 py-1.5 text-sm bg-white border rounded-lg disabled:opacity-30">{t(lang, 'usage.next')} <ChevronRight size={14} /></button>
          </div>
        )}
      </div>
    </div>
  )
}

function ThCell({ children, align }: { children: React.ReactNode; align?: 'left' | 'right' }) {
  return (
    <th className={`text-xs font-medium text-gray-400 uppercase px-5 py-2 ${align === 'right' ? 'text-right' : 'text-left'}`}>
      {children}
    </th>
  )
}

function FilterRow({ filters, setFilter, clearFilters }: {
  filters: ColumnFilter
  setFilter: (col: keyof ColumnFilter, value: string) => void
  clearFilters: () => void
}) {
  const { lang } = useI18n()
  return (
    <div className="flex items-center gap-2 flex-wrap">
      <FilterInput label={t(lang, 'usageshared.user')} value={filters.user} onChange={v => setFilter('user', v)} />
      <FilterInput label="Key" value={filters.key} onChange={v => setFilter('key', v)} />
      <FilterInput label={t(lang, 'usage.model')} value={filters.model} onChange={v => setFilter('model', v)} />
      <FilterInput label="Provider" value={filters.provider} onChange={v => setFilter('provider', v)} />
      <button
        onClick={clearFilters}
        className="flex items-center gap-1 px-2 py-1 text-xs text-gray-400 hover:text-gray-600 transition-colors"
      >
        <XCircle size={12} />
        {t(lang, 'usage.clear')}
      </button>
    </div>
  )
}

function FilterInput({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }) {
  const { lang } = useI18n()
  return (
    <div className="flex items-center gap-1">
      <label className="text-xs text-gray-400 whitespace-nowrap">{label}</label>
      <input
        type="text"
        value={value}
        onChange={e => onChange(e.target.value)}
        placeholder={t(lang, 'usage.filter_placeholder')}
        className="w-24 px-1.5 py-0.5 text-xs border border-gray-200 rounded focus:outline-none focus:border-indigo-400 focus:ring-1 focus:ring-indigo-200"
      />
    </div>
  )
}

function StatC({ icon, label, value, color }: { icon: React.ReactNode; label: string; value: string; color: string }) {
  const c: Record<string, string> = { indigo: 'bg-indigo-50 text-indigo-600', blue: 'bg-blue-50 text-blue-600', emerald: 'bg-emerald-50 text-emerald-600', amber: 'bg-amber-50 text-amber-600', rose: 'bg-rose-50 text-rose-600', purple: 'bg-purple-50 text-purple-600', cyan: 'bg-cyan-50 text-cyan-600' }
  return <div className="bg-white rounded-xl border border-gray-200 p-3 flex items-center gap-2"><div className={'w-8 h-8 rounded-lg flex items-center justify-center ' + (c[color] || c.indigo)}>{icon}</div><div><p className="text-base font-bold text-gray-800">{value}</p><p className="text-xs text-gray-400">{label}</p></div></div>
}
