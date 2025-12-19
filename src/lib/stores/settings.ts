import { writable, derived, get } from 'svelte/store'
import type { Theme, AccentColor } from '$lib/types'
import * as settingsApi from '$lib/api/settings'

// =============================================================================
// State
// =============================================================================

interface SettingsState {
	theme: Theme
	accentColor: AccentColor
	resolvedTheme: 'light' | 'dark' // Actual theme after resolving 'system'
	loading: boolean
	error: string | null
}

const initialState: SettingsState = {
	theme: 'system',
	accentColor: 'blue',
	resolvedTheme: 'dark',
	loading: false,
	error: null,
}

// =============================================================================
// System Theme Detection
// =============================================================================

function getSystemTheme(): 'light' | 'dark' {
	if (typeof window === 'undefined') return 'dark'
	return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
}

// =============================================================================
// Store
// =============================================================================

function createSettingsStore() {
	const { subscribe, set, update } = writable<SettingsState>(initialState)
	let systemThemeMediaQuery: MediaQueryList | null = null
	let mediaQueryHandler: ((e: MediaQueryListEvent) => void) | null = null

	function resolveTheme(theme: Theme): 'light' | 'dark' {
		if (theme === 'system') {
			return getSystemTheme()
		}
		return theme
	}

	function applyTheme(resolvedTheme: 'light' | 'dark') {
		if (typeof document === 'undefined') return
		document.documentElement.setAttribute('data-theme', resolvedTheme)
	}

	function applyAccentColor(color: AccentColor) {
		if (typeof document === 'undefined') return
		document.documentElement.setAttribute('data-accent', color)
	}

	function setupSystemThemeListener() {
		if (typeof window === 'undefined') return

		// Clean up existing listener
		if (systemThemeMediaQuery && mediaQueryHandler) {
			systemThemeMediaQuery.removeEventListener('change', mediaQueryHandler)
		}

		systemThemeMediaQuery = window.matchMedia('(prefers-color-scheme: dark)')

		mediaQueryHandler = () => {
			const state = get({ subscribe })
			if (state.theme === 'system') {
				const resolved = getSystemTheme()
				update((s) => ({ ...s, resolvedTheme: resolved }))
				applyTheme(resolved)
			}
		}

		systemThemeMediaQuery.addEventListener('change', mediaQueryHandler)
	}

	return {
		subscribe,

		/**
		 * Load settings from backend
		 */
		async load() {
			update((s) => ({ ...s, loading: true, error: null }))

			try {
				const settings = await settingsApi.getSettings()
				const resolvedTheme = resolveTheme(settings.theme)

				update((s) => ({
					...s,
					theme: settings.theme,
					accentColor: settings.accentColor,
					resolvedTheme,
					loading: false,
				}))

				applyTheme(resolvedTheme)
				applyAccentColor(settings.accentColor)
				setupSystemThemeListener()
			} catch (error) {
				update((s) => ({
					...s,
					loading: false,
					error: error instanceof Error ? error.message : 'Failed to load settings',
				}))

				// Apply defaults on error
				const resolvedTheme = resolveTheme('system')
				applyTheme(resolvedTheme)
				applyAccentColor('blue')
				setupSystemThemeListener()
			}
		},

		/**
		 * Set theme preference
		 */
		async setTheme(theme: Theme) {
			const resolvedTheme = resolveTheme(theme)

			update((s) => ({ ...s, theme, resolvedTheme }))
			applyTheme(resolvedTheme)

			try {
				await settingsApi.setSetting('theme', theme)
			} catch (error) {
				console.error('Failed to save theme setting:', error)
			}
		},

		/**
		 * Set accent color
		 */
		async setAccentColor(color: AccentColor) {
			update((s) => ({ ...s, accentColor: color }))
			applyAccentColor(color)

			try {
				await settingsApi.setSetting('accent_color', color)
			} catch (error) {
				console.error('Failed to save accent color setting:', error)
			}
		},

		/**
		 * Reset store to initial state
		 */
		reset() {
			set(initialState)
		},
	}
}

export const settingsStore = createSettingsStore()

// =============================================================================
// Derived Stores
// =============================================================================

export const theme = derived(settingsStore, ($s) => $s.theme)

export const accentColor = derived(settingsStore, ($s) => $s.accentColor)

export const resolvedTheme = derived(settingsStore, ($s) => $s.resolvedTheme)

export const settingsLoading = derived(settingsStore, ($s) => $s.loading)
