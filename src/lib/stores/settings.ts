import { tick } from 'svelte'
import { writable, derived, get } from 'svelte/store'
import type {
	Theme,
	AccentColor,
	Font,
	AudioDevice,
	Language,
	KeyNotationFormat,
	DateFormat,
	ExportFormat,
} from '$lib/types'
import * as settingsApi from '$lib/api/settings'
import { rebuildMenu, type MenuTranslations } from '$lib/api/app'
import { setLanguage as setI18nLanguage, translate } from '$lib/i18n'
import { appStore } from './app'

// =============================================================================
// State
// =============================================================================

interface SettingsState {
	theme: Theme
	accentColor: AccentColor
	font: Font
	resolvedTheme: 'light' | 'dark' // Actual theme after resolving 'system'
	audioDevice: string | null
	audioDevices: AudioDevice[]
	language: Language
	keyNotationFormat: KeyNotationFormat
	dateFormat: DateFormat
	exportFormat: ExportFormat
	autoAnalyzeOnImport: boolean
	autoSyncOnConnect: boolean
	autoSyncOnChange: boolean
	ignoredDeviceIds: string[]
	loading: boolean
	error: string | null
}

const initialState: SettingsState = {
	theme: 'system',
	accentColor: 'blue',
	font: 'open-sans',
	resolvedTheme: 'dark',
	audioDevice: null,
	audioDevices: [],
	language: 'en',
	keyNotationFormat: 'camelot',
	dateFormat: 'locale',
	exportFormat: 'pdb',
	autoAnalyzeOnImport: true,
	autoSyncOnConnect: false,
	autoSyncOnChange: false,
	ignoredDeviceIds: [],
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

	function applyFont(font: Font) {
		if (typeof document === 'undefined') return
		document.documentElement.setAttribute('data-font', font)
	}

	function persistToLocalStorage(theme: Theme, accentColor: AccentColor, language?: Language) {
		if (typeof localStorage === 'undefined') return
		try {
			localStorage.setItem('crate-theme', theme)
			localStorage.setItem('crate-accent', accentColor)
			if (language) {
				localStorage.setItem('crate-language', language)
			}
		} catch {
			// localStorage not available or quota exceeded, ignore
		}
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

	function getAppName(): string {
		const appState = get(appStore)
		const environment = appState.info?.environment ?? 'development'
		if (environment === 'production') {
			return 'Crate'
		}
		if (environment === 'development') {
			return 'Crate Dev'
		}
		// Other environments (alpha, beta, staging, etc.) use capitalized name
		const suffix = environment.charAt(0).toUpperCase() + environment.slice(1)
		return `Crate ${suffix}`
	}

	function getMenuTranslations(): MenuTranslations {
		const t = get(translate)
		const appName = getAppName()
		return {
			// Menu titles
			file: t('menu.file'),
			edit: t('menu.edit'),
			playback: t('menu.playback'),
			view: t('menu.view'),
			window: t('menu.window'),
			help: t('menu.help'),
			// App menu items (about and quit contain app name via template)
			about: t('menu.about', { values: { appName } }),
			settings: t('menu.settings'),
			quit: t('menu.quit', { values: { appName } }),
			// File menu items
			importTracks: t('menu.importTracks'),
			newPlaylist: t('menu.newPlaylist'),
			newFolder: t('menu.newFolder'),
			quickExport: t('menu.quickExport'),
			// Edit menu items
			selectAllTracks: t('menu.selectAllTracks'),
			// Playback menu items
			playPause: t('menu.playPause'),
			stop: t('menu.stop'),
			jumpToPlaying: t('menu.jumpToPlaying'),
			// View menu items
			toggleView: t('menu.toggleView'),
			toggleEditor: t('menu.toggleEditor'),
			showDevTools: t('menu.showDevTools'),
			// Settings submenu
			settingsSubmenu: t('menu.settingsSubmenu'),
			settingsGeneral: t('menu.settingsGeneral'),
			settingsLibrary: t('menu.settingsLibrary'),
			settingsAppearance: t('menu.settingsAppearance'),
			settingsSound: t('menu.settingsSound'),
			settingsDiagnostics: t('menu.settingsDiagnostics'),
			// Window menu items
			minimize: t('menu.minimize'),
			zoom: t('menu.zoom'),
			// Help menu items
			documentation: t('menu.documentation', { values: { appName } }),
			reportIssue: t('menu.reportIssue'),
		}
	}

	async function updateMenuTranslations() {
		try {
			await rebuildMenu(getMenuTranslations())
		} catch (error) {
			console.error('Failed to rebuild menu:', error)
		}
	}

	return {
		subscribe,

		/**
		 * Load settings from backend
		 */
		async load() {
			update((s) => ({ ...s, loading: true, error: null }))

			try {
				const [settings, audioDevices] = await Promise.all([settingsApi.getSettings(), settingsApi.getAudioDevices()])
				const resolvedTheme = resolveTheme(settings.theme)

				update((s) => ({
					...s,
					theme: settings.theme,
					accentColor: settings.accentColor,
					font: settings.font,
					audioDevice: settings.audioDevice,
					audioDevices,
					language: settings.language,
					keyNotationFormat: settings.keyNotationFormat,
					dateFormat: settings.dateFormat ?? 'locale',
					exportFormat: settings.exportFormat ?? 'pdb',
					autoAnalyzeOnImport: settings.autoAnalyzeOnImport,
					autoSyncOnConnect: settings.autoSyncOnConnect,
					autoSyncOnChange: settings.autoSyncOnChange,
					ignoredDeviceIds: settings.ignoredDeviceIds,
					resolvedTheme,
					loading: false,
				}))

				applyTheme(resolvedTheme)
				applyAccentColor(settings.accentColor)
				applyFont(settings.font)
				persistToLocalStorage(settings.theme, settings.accentColor, settings.language)
				setupSystemThemeListener()

				// Update i18n language and menu
				await setI18nLanguage(settings.language)
				await tick()
				await updateMenuTranslations()
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
				applyFont('ibm-plex-mono')
				setupSystemThemeListener()
			}
		},

		/**
		 * Set theme preference
		 */
		async setTheme(theme: Theme) {
			const resolvedTheme = resolveTheme(theme)
			const state = get({ subscribe })

			update((s) => ({ ...s, theme, resolvedTheme }))
			applyTheme(resolvedTheme)
			persistToLocalStorage(theme, state.accentColor)

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
			const state = get({ subscribe })

			update((s) => ({ ...s, accentColor: color }))
			applyAccentColor(color)
			persistToLocalStorage(state.theme, color)

			try {
				await settingsApi.setSetting('accent_color', color)
			} catch (error) {
				console.error('Failed to save accent color setting:', error)
			}
		},

		/**
		 * Set font family
		 */
		async setFont(font: Font) {
			update((s) => ({ ...s, font }))
			applyFont(font)

			try {
				await settingsApi.setSetting('font', font)
			} catch (error) {
				console.error('Failed to save font setting:', error)
			}
		},

		/**
		 * Set audio output device
		 */
		async setAudioDevice(deviceName: string | null) {
			update((s) => ({ ...s, audioDevice: deviceName }))

			try {
				await settingsApi.setAudioDevice(deviceName)
			} catch (error) {
				console.error('Failed to save audio device setting:', error)
			}
		},

		/**
		 * Set display language
		 */
		async setLanguage(language: Language) {
			const state = get({ subscribe })

			update((s) => ({ ...s, language }))
			await setI18nLanguage(language)
			await tick()
			await updateMenuTranslations()
			persistToLocalStorage(state.theme, state.accentColor, language)

			try {
				await settingsApi.setSetting('language', language)
			} catch (error) {
				console.error('Failed to save language setting:', error)
			}
		},

		/**
		 * Set key notation format (standard or camelot)
		 */
		async setKeyNotationFormat(format: KeyNotationFormat) {
			update((s) => ({ ...s, keyNotationFormat: format }))

			try {
				await settingsApi.setSetting('key_notation_format', format)
			} catch (error) {
				console.error('Failed to save key notation format setting:', error)
			}
		},

		/**
		 * Set date format
		 */
		async setDateFormat(format: DateFormat) {
			update((s) => ({ ...s, dateFormat: format }))

			try {
				await settingsApi.setSetting('date_format', format)
			} catch (error) {
				console.error('Failed to save date format setting:', error)
			}
		},

		/**
		 * Set export format (pdb or device_library_plus)
		 */
		async setExportFormat(format: ExportFormat) {
			update((s) => ({ ...s, exportFormat: format }))

			try {
				await settingsApi.setSetting('export_format', format)
			} catch (error) {
				console.error('Failed to save export format setting:', error)
			}
		},

		/**
		 * Set auto-analyze on import
		 */
		async setAutoAnalyzeOnImport(enabled: boolean) {
			update((s) => ({ ...s, autoAnalyzeOnImport: enabled }))

			try {
				await settingsApi.setSetting('auto_analyze_on_import', enabled ? 'true' : 'false')
			} catch (error) {
				console.error('Failed to save auto analyze on import setting:', error)
			}
		},

		/**
		 * Set auto-sync on device connected
		 */
		async setAutoSyncOnConnect(enabled: boolean) {
			update((s) => ({ ...s, autoSyncOnConnect: enabled }))

			try {
				await settingsApi.setSetting('auto_sync_on_connect', enabled ? 'true' : 'false')
			} catch (error) {
				console.error('Failed to save auto sync on connect setting:', error)
			}
		},

		/**
		 * Set auto-sync on library changes
		 */
		async setAutoSyncOnChange(enabled: boolean) {
			update((s) => ({ ...s, autoSyncOnChange: enabled }))

			try {
				await settingsApi.setSetting('auto_sync_on_change', enabled ? 'true' : 'false')
			} catch (error) {
				console.error('Failed to save auto sync on change setting:', error)
			}
		},

		/**
		 * Add a device to the ignore list
		 */
		async ignoreDevice(deviceId: string) {
			const state = get({ subscribe })
			if (state.ignoredDeviceIds.includes(deviceId)) return

			const newList = [...state.ignoredDeviceIds, deviceId]
			update((s) => ({ ...s, ignoredDeviceIds: newList }))

			try {
				await settingsApi.setSetting('ignored_device_ids', JSON.stringify(newList))
			} catch (error) {
				console.error('Failed to save ignored devices setting:', error)
			}
		},

		/**
		 * Remove a device from the ignore list
		 */
		async unignoreDevice(deviceId: string) {
			const state = get({ subscribe })
			const newList = state.ignoredDeviceIds.filter((id) => id !== deviceId)
			update((s) => ({ ...s, ignoredDeviceIds: newList }))

			try {
				await settingsApi.setSetting('ignored_device_ids', JSON.stringify(newList))
			} catch (error) {
				console.error('Failed to save ignored devices setting:', error)
			}
		},

		/**
		 * Refresh the list of available audio devices
		 */
		async refreshAudioDevices() {
			try {
				const audioDevices = await settingsApi.getAudioDevices()
				update((s) => ({ ...s, audioDevices }))
			} catch (error) {
				console.error('Failed to refresh audio devices:', error)
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

export const font = derived(settingsStore, ($s) => $s.font)

export const resolvedTheme = derived(settingsStore, ($s) => $s.resolvedTheme)

export const audioDevice = derived(settingsStore, ($s) => $s.audioDevice)

export const audioDevices = derived(settingsStore, ($s) => $s.audioDevices)

export const language = derived(settingsStore, ($s) => $s.language)

export const keyNotationFormat = derived(settingsStore, ($s) => $s.keyNotationFormat)

export const dateFormat = derived(settingsStore, ($s) => $s.dateFormat)

export const exportFormat = derived(settingsStore, ($s) => $s.exportFormat)

export const autoAnalyzeOnImport = derived(settingsStore, ($s) => $s.autoAnalyzeOnImport)

export const autoSyncOnConnect = derived(settingsStore, ($s) => $s.autoSyncOnConnect)

export const autoSyncOnChange = derived(settingsStore, ($s) => $s.autoSyncOnChange)

export const ignoredDeviceIds = derived(settingsStore, ($s) => $s.ignoredDeviceIds)

export const settingsLoading = derived(settingsStore, ($s) => $s.loading)
