import { register, init, getLocaleFromNavigator, locale, waitLocale, _ } from 'svelte-i18n'

export type Language = 'en' | 'ja' | 'nl' | 'fr' | 'de' | 'es' | 'it' | 'sv' | 'ko' | 'pt' | 'zh'

export const SUPPORTED_LANGUAGES: { value: Language; label: string; nativeLabel: string }[] = [
	{ value: 'en', label: 'English', nativeLabel: 'English' },
	{ value: 'ja', label: 'Japanese', nativeLabel: '日本語' },
	{ value: 'nl', label: 'Dutch', nativeLabel: 'Nederlands' },
	{ value: 'fr', label: 'French', nativeLabel: 'Français' },
	{ value: 'de', label: 'German', nativeLabel: 'Deutsch' },
	{ value: 'es', label: 'Spanish', nativeLabel: 'Español' },
	{ value: 'it', label: 'Italian', nativeLabel: 'Italiano' },
	{ value: 'sv', label: 'Swedish', nativeLabel: 'Svenska' },
	{ value: 'ko', label: 'Korean', nativeLabel: '한국어' },
	{ value: 'pt', label: 'Portuguese', nativeLabel: 'Português' },
	{ value: 'zh', label: 'Chinese', nativeLabel: '中文' },
]

// Register locale files - lazy loaded
register('en', () => import('./locales/en.json'))
register('ja', () => import('./locales/ja.json'))
register('nl', () => import('./locales/nl.json'))
register('fr', () => import('./locales/fr.json'))
register('de', () => import('./locales/de.json'))
register('es', () => import('./locales/es.json'))
register('it', () => import('./locales/it.json'))
register('sv', () => import('./locales/sv.json'))
register('ko', () => import('./locales/ko.json'))
register('pt', () => import('./locales/pt.json'))
register('zh', () => import('./locales/zh.json'))

// Initialize with default locale synchronously at module load
// This prevents "Cannot format a message without first setting the initial locale" errors
// The actual user preference is loaded later in +layout.svelte onMount
init({
	fallbackLocale: 'en',
	initialLocale: 'en',
})

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
 * Note: init() is called at module load with 'en' default, this updates to the user's preference
 */
export async function initializeI18n(savedLanguage?: Language | null): Promise<void> {
	const language = savedLanguage || getSystemLanguage()
	locale.set(language)
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
export { locale, waitLocale }

// Re-export _ as 'translate' for cleaner usage: {$translate('key')}
export { _ as translate }
