import { register, init, getLocaleFromNavigator, locale, waitLocale, _ } from 'svelte-i18n'

export type Language = 'en' | 'ja'

export const SUPPORTED_LANGUAGES: { value: Language; label: string; nativeLabel: string }[] = [
	{ value: 'en', label: 'English', nativeLabel: 'English' },
	{ value: 'ja', label: 'Japanese', nativeLabel: '日本語' },
]

// Register locale files - lazy loaded
register('en', () => import('./locales/en.json'))
register('ja', () => import('./locales/ja.json'))

/**
 * Get system language, fallback to English if not supported
 */
function getSystemLanguage(): Language {
	const systemLocale = getLocaleFromNavigator() || 'en'
	const lang = systemLocale.split('-')[0] as Language
	return SUPPORTED_LANGUAGES.some((l) => l.value === lang) ? lang : 'en'
}

/**
 * Initialize i18n with a specific language or detect from system
 */
export async function initializeI18n(savedLanguage?: Language | null): Promise<void> {
	const language = savedLanguage || getSystemLanguage()

	init({
		fallbackLocale: 'en',
		initialLocale: language,
	})

	await waitLocale()
}

/**
 * Set language at runtime
 */
export async function setLanguage(language: Language): Promise<void> {
	locale.set(language)
	await waitLocale()
}

/**
 * Get the current language
 */
export function getCurrentLanguage(): Language {
	const current = locale.subscribe ? undefined : 'en'
	let lang: Language = 'en'
	const unsubscribe = locale.subscribe((value) => {
		lang = (value as Language) || 'en'
	})
	unsubscribe()
	return lang
}

// Re-export commonly used items from svelte-i18n
export { locale, _, waitLocale }
