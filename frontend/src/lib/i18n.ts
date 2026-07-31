import { create } from 'zustand'

export type Lang = 'zh' | 'en'

const messages = {
  zh: {
    // Layout / nav
    'nav.dashboard': '仪表板',
    'nav.endpoints': '端点',
    'nav.providers': 'Provider',
    'nav.usage.my': '我的用量',
    'nav.usage.shared': '分享用量',
    'nav.users': '用户',
    'app.console': '管理控制台',
    'role.admin': '管理员',
    'role.user': '用户',
    'action.logout': '退出',
    'action.language': '切换语言',
    // Common
    'common.loading': '加载中…',
    'common.cancel': '取消',
    'common.save': '保存',
    'common.delete': '删除',
    'common.status': '状态',
    'common.actions': '操作',
    'common.copied': '已复制到剪贴板',
    'common.name_required': '名称不能为空',
    // Dashboard
    'dash.title': '仪表板',
    'dash.my_providers': '我的 Provider',
    'dash.my_endpoints': '我的 Endpoint',
    'dash.my_keys': '我的 Key',
    'dash.shared_endpoints': '分享端点',
    'dash.today_my_tokens': '今日自己消耗',
    'dash.today_shared_tokens': '今日分享消耗',
    'dash.keys_received': '收到 Key',
    'dash.keys_sent': '发出 Key',
    'dash.service_status': '服务状态',
    'dash.running_port': '运行中 — 端口 19528',
    // Endpoints
    'endpoints.title': '端点管理',
    'endpoints.new': '新建端点',
    'endpoints.name': '名称',
    'endpoints.path_prefix': '路径前缀 (如 default)',
    'endpoints.full_path': '完整路径:',
    'endpoints.creating': '创建中…',
    'endpoints.create': '创建',
    'endpoints.empty': '暂无端点，点击「新建端点」开始',
    'endpoints.path': '路径',
    'endpoints.protocol': '协议',
    'endpoints.base_url': '连接地址',
    'endpoints.created': '端点已创建',
    'endpoints.updated': '端点已更新',
    'endpoints.deleted': '端点已删除',
    'endpoints.click_edit': '点击编辑',
    'endpoints.copy_url': '复制连接地址',
    'endpoints.delete_confirm': '确定删除此端点？',
    // API Keys
    'keys.title': 'API Key 管理',
    'keys.generate': '生成 API Key',
    'keys.created': 'API Key 已创建',
    'keys.updated': '已更新',
    'keys.deleted': 'API Key 已删除',
    'keys.name_placeholder': 'Key 名称 (如 "Chat App")',
    'keys.assign_self': '分配给自己',
    'keys.generating': '生成中…',
    'keys.create': '生成',
    'keys.assign_hint': '选择用户后，该 Key 将分配给对应用户使用。"分配给自己" 即自己使用。',
    'keys.created_notice': '✅ API Key 已创建！请立即复制，之后将无法再次查看：',
    'keys.empty': '暂无 API Key',
    'keys.name_col': '名称',
    'keys.assigned_to': '分配给',
    'keys.created_at': '创建时间',
    'keys.last_used': '最后使用',
    'keys.monthly_usage': '本月用量',
    'keys.click_edit_name': '点击编辑名称',
    'keys.copy_full': '复制完整 Key',
    'keys.self': '（自己）',
    'keys.delete_confirm': '删除此 Key？',
    // Login
    'login.success': '登录成功',
    'login.failed': '登录失败',
    'login.subtitle': '登录管理控制台',
    'login.username': '用户名',
    'login.password': '密码',
    'login.logging_in': '登录中…',
    'login.login': '登录',
  },
  en: {
    // Layout / nav
    'nav.dashboard': 'Dashboard',
    'nav.endpoints': 'Endpoints',
    'nav.providers': 'Providers',
    'nav.usage.my': 'My Usage',
    'nav.usage.shared': 'Shared Usage',
    'nav.users': 'Users',
    'app.console': 'Admin Console',
    'role.admin': 'Admin',
    'role.user': 'User',
    'action.logout': 'Logout',
    'action.language': 'Switch language',
    // Common
    'common.loading': 'Loading…',
    'common.cancel': 'Cancel',
    'common.save': 'Save',
    'common.delete': 'Delete',
    'common.status': 'Status',
    'common.actions': 'Actions',
    'common.copied': 'Copied to clipboard',
    'common.name_required': 'Name cannot be empty',
    // Dashboard
    'dash.title': 'Dashboard',
    'dash.my_providers': 'My Providers',
    'dash.my_endpoints': 'My Endpoints',
    'dash.my_keys': 'My Keys',
    'dash.shared_endpoints': 'Shared Endpoints',
    'dash.today_my_tokens': 'My Tokens Today',
    'dash.today_shared_tokens': 'Shared Tokens Today',
    'dash.keys_received': 'Keys Received',
    'dash.keys_sent': 'Keys Shared',
    'dash.service_status': 'Service Status',
    'dash.running_port': 'Running — port 19528',
    // Endpoints
    'endpoints.title': 'Endpoints',
    'endpoints.new': 'New Endpoint',
    'endpoints.name': 'Name',
    'endpoints.path_prefix': 'Path prefix (e.g. default)',
    'endpoints.full_path': 'Full path:',
    'endpoints.creating': 'Creating…',
    'endpoints.create': 'Create',
    'endpoints.empty': 'No endpoints yet. Click "New Endpoint" to start.',
    'endpoints.path': 'Path',
    'endpoints.protocol': 'Protocol',
    'endpoints.base_url': 'Base URL',
    'endpoints.created': 'Endpoint created',
    'endpoints.updated': 'Endpoint updated',
    'endpoints.deleted': 'Endpoint deleted',
    'endpoints.click_edit': 'Click to edit',
    'endpoints.copy_url': 'Copy URL',
    'endpoints.delete_confirm': 'Delete this endpoint?',
    // API Keys
    'keys.title': 'API Keys',
    'keys.generate': 'Generate API Key',
    'keys.created': 'API Key created',
    'keys.updated': 'Updated',
    'keys.deleted': 'API Key deleted',
    'keys.name_placeholder': 'Key name (e.g. "Chat App")',
    'keys.assign_self': 'Assign to self',
    'keys.generating': 'Generating…',
    'keys.create': 'Generate',
    'keys.assign_hint': 'After selecting a user, the key is assigned to that user. "Assign to self" means your own use.',
    'keys.created_notice': '✅ API Key created! Copy it now — it will not be shown again:',
    'keys.empty': 'No API keys',
    'keys.name_col': 'Name',
    'keys.assigned_to': 'Assigned to',
    'keys.created_at': 'Created',
    'keys.last_used': 'Last used',
    'keys.monthly_usage': 'Monthly usage',
    'keys.click_edit_name': 'Click to edit name',
    'keys.copy_full': 'Copy full key',
    'keys.self': '(self)',
    'keys.delete_confirm': 'Delete this key?',
    // Login
    'login.success': 'Login successful',
    'login.failed': 'Login failed',
    'login.subtitle': 'Sign in to admin console',
    'login.username': 'Username',
    'login.password': 'Password',
    'login.logging_in': 'Logging in…',
    'login.login': 'Login',
  },
} as const

export type MessageKey = keyof (typeof messages)['zh']

export function t(lang: Lang, key: MessageKey): string {
  return messages[lang][key] ?? messages.zh[key] ?? key
}

interface I18nState {
  lang: Lang
  setLang: (lang: Lang) => void
}

const initialLang: Lang = localStorage.getItem('lang') === 'en' ? 'en' : 'zh'

export const useI18n = create<I18nState>((set) => ({
  lang: initialLang,
  setLang: (lang) => {
    localStorage.setItem('lang', lang)
    document.documentElement.lang = lang
    set({ lang })
  },
}))
