import { create } from 'zustand'

export type Lang = 'zh' | 'en'

const messages = {
  zh: {
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
  },
  en: {
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
