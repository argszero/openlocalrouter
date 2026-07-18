import { useState, useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import { getMyUsageSummary, getMyUsageTrend, getMyUsageTrendBreakdown, getMyUsageRecords } from '../lib/api'
import type { TimeSeriesBreakdown } from '../lib/api'
import { Zap, ArrowUpRight, Cpu, MousePointerClick, Brain, Calendar, ChevronLeft, ChevronRight } from 'lucide-react'

function formatTokens(n: number) { if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M'; if (n >= 1_000) return (n / 1_000).toFixed(1) + 'k'; return String(n) }
function formatDate(s: string) { return s.replace('T', ' ').slice(0, 16) }
function todayStr() { return new Date().toISOString().slice(0, 10) }
function daysAgo(n: number) { const d = new Date(); d.setDate(d.getDate() - n); return d.toISOString().slice(0, 10) }

// ── Color palette ────────────────────────────────────
const COLORS = ['#6366f1','#3b82f6','#10b981','#f59e0b','#ef4444','#8b5cf6','#ec4899','#06b6d4']

// ── SVG Trend Chart ──────────────────────────────────
function TrendChart({ series, height }: { series: { label: string; color: string; points: { ts: string; val: number }[] }[]; height: number }) {
  const allPoints = series.flatMap(s => s.points)
  if (allPoints.length === 0) return <div className="text-sm text-gray-400 py-8 text-center">暂无趋势数据</div>

  const timestamps = [...new Set(allPoints.map(p => p.ts))].sort()
  const maxVal = Math.max(1, ...allPoints.map(p => p.val))
  const pad = { top: 20, right: 16, bottom: 36, left: 48 }
  const w = 800; const h = height
  const plotW = w - pad.left - pad.right
  const plotH = h - pad.top - pad.bottom

  const x = (ts: string) => pad.left + (timestamps.indexOf(ts) / Math.max(1, timestamps.length - 1)) * plotW
  const y = (v: number) => pad.top + plotH - (v / maxVal) * plotH

  // Y-axis ticks
  const yTicks = 4
  const yValues = Array.from({length: yTicks}, (_, i) => Math.round((maxVal / (yTicks - 1)) * i))

  return (
    <svg viewBox={`0 0 ${w} ${h}`} className="w-full" style={{ maxHeight: height }}>
      {/* Y axis */}
      {yValues.map(v => (
        <g key={v}>
          <line x1={pad.left} y1={y(v)} x2={w - pad.right} y2={y(v)} stroke="#f1f5f9" strokeWidth={1} />
          <text x={pad.left - 6} y={y(v) + 4} textAnchor="end" className="text-[10px]" fill="#94a3b8">{formatTokens(v)}</text>
        </g>
      ))}
      {/* X axis labels */}
      {timestamps.filter((_, i) => timestamps.length <= 7 || i % Math.ceil(timestamps.length / 7) === 0).map(ts => (
        <text key={ts} x={x(ts)} y={h - 6} textAnchor="middle" className="text-[10px]" fill="#94a3b8">
          {ts.slice(5)} {/* MM-DD */}
        </text>
      ))}
      {/* Lines */}
      {series.map(s => {
        const sorted = [...s.points].sort((a,b) => a.ts.localeCompare(b.ts))
        if (sorted.length < 2) return null
        const d = sorted.map((p,i) => `${i===0?'M':'L'} ${x(p.ts).toFixed(1)} ${y(p.val).toFixed(1)}`).join(' ')
        return (
          <g key={s.label}>
            <path d={d} fill="none" stroke={s.color} strokeWidth={2} strokeLinecap="round" strokeLinejoin="round" />
            {sorted.map(p => (
              <circle key={p.ts} cx={x(p.ts).toFixed(1)} cy={y(p.val).toFixed(1)} r={3} fill={s.color} />
            ))}
          </g>
        )
      })}
    </svg>
  )
}

// ── Legend ───────────────────────────────────────────
function Legend({ items }: { items: { label: string; color: string }[] }) {
  if (items.length <= 1) return null
  return (
    <div className="flex flex-wrap gap-3 mt-2">
      {items.map(it => (
        <div key={it.label} className="flex items-center gap-1.5 text-xs text-gray-500">
          <span className="w-2.5 h-2.5 rounded-full shrink-0" style={{ backgroundColor: it.color }} />
          <span className="truncate max-w-[140px]">{it.label}</span>
        </div>
      ))}
    </div>
  )
}

export default function UsageMyPage() {
  const [dateFrom, setDateFrom] = useState(daysAgo(7))
  const [dateTo, setDateTo] = useState(todayStr())
  const [page, setPage] = useState(0)
  const [trendMode, setTrendMode] = useState<'total' | 'model' | 'key'>('total')
  const limit = 25

  // Summary + records (existing)
  const { data: summary } = useQuery({ queryKey: ['myUsageSum', dateFrom, dateTo], queryFn: () => getMyUsageSummary('model', dateFrom, dateTo), refetchInterval: 30000 })
  const { data: records, isLoading } = useQuery({ queryKey: ['myUsageRec', page, dateFrom, dateTo], queryFn: () => getMyUsageRecords({ from: dateFrom, to: dateTo, limit, offset: page * limit }), refetchInterval: 30000 })

  // Total trend
  const { data: totalTrend } = useQuery({ queryKey: ['myTrend', dateFrom, dateTo], queryFn: () => getMyUsageTrend({ from: dateFrom, to: dateTo }), refetchInterval: 60000, enabled: trendMode === 'total' })
  // Breakdown trend (model or key)
  const { data: breakdownTrend } = useQuery({ queryKey: ['myTrendBd', trendMode, dateFrom, dateTo], queryFn: () => getMyUsageTrendBreakdown({ group_by: trendMode, from: dateFrom, to: dateTo }), refetchInterval: 60000, enabled: trendMode !== 'total' })

  const totalPages = records ? Math.ceil(records.total / limit) : 0
  const groups = summary?.groups || []

  const stats = useMemo(() => ({
    tokens: groups.reduce((s, g) => s + g.total_input_tokens + g.total_output_tokens, 0),
    input: groups.reduce((s, g) => s + g.total_input_tokens, 0),
    output: groups.reduce((s, g) => s + g.total_output_tokens, 0),
    count: groups.reduce((s, g) => s + g.count, 0),
    models: groups.length,
  }), [groups])

  // ── Build trend chart series ────────────────────────
  const trendSeries = useMemo(() => {
    if (trendMode === 'total' && totalTrend?.points?.length) {
      const pts = totalTrend.points.map(p => ({
        ts: p.timestamp,
        val: p.input_tokens + p.output_tokens,
      }))
      return [{ label: 'Total', color: '#6366f1', points: pts }]
    }
    if (breakdownTrend?.points?.length) {
      // Group by group_key, pick top 5 by total volume
      const map = new Map<string, TimeSeriesBreakdown[]>()
      for (const p of breakdownTrend.points) {
        if (!map.has(p.group_key)) map.set(p.group_key, [])
        map.get(p.group_key)!.push(p)
      }
      const ranked = [...map.entries()]
        .map(([k, pts]) => ({ key: k, total: pts.reduce((s,p) => s + p.total_tokens, 0), pts }))
        .sort((a,b) => b.total - a.total)
        .slice(0, 5)
      return ranked.map((r, i) => ({
        label: r.key,
        color: COLORS[i % COLORS.length],
        points: r.pts.map(p => ({ ts: p.timestamp, val: p.total_tokens })),
      }))
    }
    return []
  }, [trendMode, totalTrend, breakdownTrend])

  return (
    <div>
      <h2 className="text-2xl font-semibold text-gray-800 mb-4">我的用量</h2>
      <div className="flex items-center gap-2 mb-4">
        <Calendar size={14} className="text-gray-400" />
        <input type="date" value={dateFrom} onChange={e => { setDateFrom(e.target.value); setPage(0) }} className="px-2 py-1 border border-gray-300 rounded text-xs" />
        <span className="text-gray-300">{'—'}</span>
        <input type="date" value={dateTo} onChange={e => { setDateTo(e.target.value); setPage(0) }} className="px-2 py-1 border border-gray-300 rounded text-xs" />
        <button onClick={() => { setDateFrom(daysAgo(7)); setDateTo(todayStr()) }} className="px-2 py-1 text-xs border rounded hover:bg-gray-50">7天</button>
        <button onClick={() => { setDateFrom(daysAgo(30)); setDateTo(todayStr()) }} className="px-2 py-1 text-xs border rounded hover:bg-gray-50">30天</button>
      </div>

      {/* Stat cards */}
      <div className="grid grid-cols-2 md:grid-cols-5 gap-3 mb-6">
        <StatC icon={<Zap size={16} />} label="Total" value={formatTokens(stats.tokens)} color="indigo" />
        <StatC icon={<ArrowUpRight size={16} />} label="Input" value={formatTokens(stats.input)} color="blue" />
        <StatC icon={<Cpu size={16} />} label="Output" value={formatTokens(stats.output)} color="emerald" />
        <StatC icon={<MousePointerClick size={16} />} label="Requests" value={String(stats.count)} color="amber" />
        <StatC icon={<Brain size={16} />} label="Models" value={String(stats.models)} color="purple" />
      </div>

      {/* Trend chart */}
      <div className="bg-white rounded-xl border p-4 mb-6">
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-sm font-semibold text-gray-700">Token 趋势</h3>
          <div className="flex gap-1">
            {(['total','model','key'] as const).map(m => (
              <button
                key={m}
                onClick={() => setTrendMode(m)}
                className={`px-3 py-1 text-xs rounded-lg transition-colors ${
                  trendMode === m ? 'bg-indigo-600 text-white' : 'bg-gray-100 text-gray-600 hover:bg-gray-200'
                }`}
              >
                {m === 'total' ? '总计' : m === 'model' ? '按模型' : '按Key'}
              </button>
            ))}
          </div>
        </div>
        <TrendChart series={trendSeries} height={220} />
        <Legend items={trendSeries.map(s => ({ label: s.label, color: s.color }))} />
      </div>

      {/* Model breakdown */}
      <div className="bg-white rounded-xl border p-4 mb-6">
        <h3 className="text-sm font-semibold text-gray-700 mb-2">Token / Model</h3>
        {groups.slice(0, 10).map(g => (
          <div key={g.key} className="flex items-center gap-2 py-1.5 text-xs">
            <span className="flex-1 text-gray-700 truncate">{g.key}</span>
            <span className="text-gray-500">{formatTokens(g.total_input_tokens + g.total_output_tokens)}</span>
            <span className="text-gray-300">{g.count}次</span>
          </div>
        ))}
      </div>

      {/* Records table */}
      <div className="bg-white rounded-xl border overflow-hidden">
        <div className="px-5 py-3 border-b flex items-center justify-between bg-gray-50/50">
          <h3 className="text-sm font-semibold text-gray-700">明细</h3>
          {records && <span className="text-xs text-gray-400">共 {records.total} 条</span>}
        </div>
        {isLoading ? <div className="p-5 text-sm text-gray-400">加载中...</div> : !records?.records?.length ? (
          <div className="p-8 text-center text-sm text-gray-400">暂无数据</div>
        ) : (
          <table className="w-full">
            <thead><tr className="border-b border-gray-100">
              <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-2">时间</th>
              <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-2">模型</th>
              <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-2">Provider</th>
              <th className="text-right text-xs font-medium text-gray-400 uppercase px-5 py-2">Input</th>
              <th className="text-right text-xs font-medium text-gray-400 uppercase px-5 py-2">Output</th>
              <th className="text-right text-xs font-medium text-gray-400 uppercase px-5 py-2">Total</th>
            </tr></thead>
            <tbody>
              {(records.records || []).map(r => (
                <tr key={r.id} className="border-b border-gray-50 hover:bg-gray-50/50">
                  <td className="px-5 py-2 text-sm text-gray-400 whitespace-nowrap">{formatDate(r.created_at)}</td>
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
            <button onClick={() => setPage(p => Math.max(0, p - 1))} disabled={page === 0} className="flex items-center gap-1 px-3 py-1.5 text-sm bg-white border rounded-lg disabled:opacity-30"><ChevronLeft size={14} /> 上一页</button>
            <span className="text-xs text-gray-400">第 {page + 1}/{totalPages} 页</span>
            <button onClick={() => setPage(p => Math.min(totalPages - 1, p + 1))} disabled={page >= totalPages - 1} className="flex items-center gap-1 px-3 py-1.5 text-sm bg-white border rounded-lg disabled:opacity-30">下一页 <ChevronRight size={14} /></button>
          </div>
        )}
      </div>
    </div>
  )
}

function StatC({ icon, label, value, color }: { icon: React.ReactNode; label: string; value: string; color: string }) {
  const c: Record<string, string> = { indigo: 'bg-indigo-50 text-indigo-600', blue: 'bg-blue-50 text-blue-600', emerald: 'bg-emerald-50 text-emerald-600', amber: 'bg-amber-50 text-amber-600', purple: 'bg-purple-50 text-purple-600' }
  return <div className="bg-white rounded-xl border border-gray-200 p-3 flex items-center gap-2"><div className={'w-8 h-8 rounded-lg flex items-center justify-center ' + (c[color] || c.indigo)}>{icon}</div><div><p className="text-base font-bold text-gray-800">{value}</p><p className="text-xs text-gray-400">{label}</p></div></div>
}
