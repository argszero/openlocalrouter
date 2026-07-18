import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { getUsageRecords, getUsageSummary } from '../lib/api'
import type { UsageRecord } from '../lib/api'

function formatTokens(n: number) {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`
  return String(n)
}

function formatDate(s: string) {
  return s.replace('T', ' ').slice(0, 19)
}

export default function UsagePage() {
  const [groupBy, setGroupBy] = useState<'key' | 'model' | 'day'>('model')
  const [page, setPage] = useState(0)
  const limit = 30

  const { data: records, isLoading } = useQuery({
    queryKey: ['usageRecords', page],
    queryFn: () => getUsageRecords({ limit, offset: page * limit }),
    refetchInterval: 30_000,
  })

  const { data: summary } = useQuery({
    queryKey: ['usageSummary', groupBy],
    queryFn: () => getUsageSummary(groupBy),
    refetchInterval: 30_000,
  })

  const totalPages = records ? Math.ceil(records.total / limit) : 0

  // Bar chart: max bar width relative
  const maxTokens = summary?.groups[0]?.total_input_tokens ?? 1

  return (
    <div>
      <h2 className="text-2xl font-semibold text-gray-800 mb-6">用量概览</h2>

      {/* Group selector */}
      <div className="flex items-center gap-2 mb-4">
        <span className="text-sm text-gray-500">分组：</span>
        {(['key', 'model', 'day'] as const).map(g => (
          <button
            key={g}
            onClick={() => setGroupBy(g)}
            className={`px-3 py-1.5 text-sm rounded-lg transition-colors ${
              groupBy === g
                ? 'bg-indigo-600 text-white'
                : 'bg-white text-gray-600 border border-gray-200 hover:bg-gray-50'
            }`}
          >
            {g === 'key' ? '按 Key' : g === 'model' ? '按模型' : '按天'}
          </button>
        ))}
      </div>

      {/* Bar chart */}
      <div className="bg-white rounded-xl border border-gray-200 p-5 mb-6">
        <h3 className="text-sm font-medium text-gray-700 mb-4">Token 消耗分布</h3>
        {!summary?.groups?.length ? (
          <p className="text-sm text-gray-400">暂无数据</p>
        ) : (
          <div className="space-y-3">
            {summary.groups.slice(0, 15).map(g => (
              <div key={g.key} className="flex items-center gap-3">
                <span className="w-24 text-xs text-gray-600 truncate shrink-0" title={g.key}>
                  {g.key}
                </span>
                <div className="flex-1 flex items-center gap-2">
                  <div
                    className="h-5 bg-indigo-500 rounded-sm min-w-[4px]"
                    style={{ width: `${Math.max(2, (g.total_input_tokens / maxTokens) * 60)}%` }}
                  />
                  <span className="text-xs text-gray-400 shrink-0">{formatTokens(g.total_input_tokens)}</span>
                </div>
                <span className="text-xs text-gray-300 shrink-0">{g.count} 次</span>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Records table */}
      <div className="bg-white rounded-xl border border-gray-200 overflow-hidden">
        <div className="px-5 py-3 border-b border-gray-100 flex items-center justify-between">
          <h3 className="text-sm font-medium text-gray-700">明细记录</h3>
          {records && <span className="text-xs text-gray-400">共 {records.total} 条</span>}
        </div>
        {isLoading ? (
          <div className="p-5 text-sm text-gray-400">加载中…</div>
        ) : !records?.records?.length ? (
          <div className="p-5 text-sm text-gray-400">暂无用量数据</div>
        ) : (
          <table className="w-full">
            <thead>
              <tr className="border-b border-gray-100">
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">模型</th>
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">Input</th>
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">Output</th>
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">Cache</th>
                <th className="text-left text-xs font-medium text-gray-400 uppercase px-5 py-3">时间</th>
              </tr>
            </thead>
            <tbody>
              {records.records.map((r: UsageRecord) => (
                <tr key={r.id} className="border-b border-gray-50 hover:bg-gray-50/50">
                  <td className="px-5 py-3 text-sm font-medium text-gray-800">{r.model}</td>
                  <td className="px-5 py-3 text-sm text-gray-600">{formatTokens(r.input_tokens)}</td>
                  <td className="px-5 py-3 text-sm text-gray-600">{formatTokens(r.output_tokens)}</td>
                  <td className="px-5 py-3 text-sm text-gray-400">{r.cache_read_tokens > 0 ? formatTokens(r.cache_read_tokens) : '—'}</td>
                  <td className="px-5 py-3 text-sm text-gray-400">{formatDate(r.created_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {totalPages > 1 && (
          <div className="px-5 py-3 border-t border-gray-100 flex items-center justify-between">
            <button
              onClick={() => setPage(p => Math.max(0, p - 1))}
              disabled={page === 0}
              className="px-3 py-1.5 text-sm text-gray-600 bg-gray-100 rounded-lg disabled:opacity-30"
            >
              上一页
            </button>
            <span className="text-xs text-gray-400">{page + 1} / {totalPages}</span>
            <button
              onClick={() => setPage(p => Math.min(totalPages - 1, p + 1))}
              disabled={page >= totalPages - 1}
              className="px-3 py-1.5 text-sm text-gray-600 bg-gray-100 rounded-lg disabled:opacity-30"
            >
              下一页
            </button>
          </div>
        )}
      </div>
    </div>
  )
}
