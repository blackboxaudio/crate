// =============================================================================
// Early Error Handler
// =============================================================================
// This file runs before any SvelteKit components. It provides a fallback crash
// screen for catastrophic errors that occur before SvelteKit's +error.svelte
// can render.
//
// For most errors, SvelteKit's +error.svelte will handle display.
// For runtime errors after mount, useGlobalErrorHandler + CrashScreen handles them.
// This is only for errors so early that nothing else can catch them.
// =============================================================================

interface EarlyError {
	message: string
	stack?: string
	timestamp: string
}

// =============================================================================
// Theme Configuration
// =============================================================================

interface ThemeColors {
	background: string
	card: string
	cardInner: string
	border: string
	textPrimary: string
	textSecondary: string
	danger: string
	dangerMuted: string
	success: string
	successMuted: string
}

const THEME_COLORS: Record<'dark' | 'light', ThemeColors> = {
	dark: {
		background: '#09090b', // surface-0
		card: '#18181b', // surface-1
		cardInner: '#27272a', // surface-2
		border: '#3f3f46', // stroke
		textPrimary: '#fafafa',
		textSecondary: '#a1a1aa',
		danger: '#ef4444',
		dangerMuted: 'rgba(239, 68, 68, 0.2)',
		success: '#10b981',
		successMuted: 'rgba(16, 185, 129, 0.2)',
	},
	light: {
		background: '#ffffff',
		card: '#fafafa',
		cardInner: '#f4f4f5',
		border: '#d4d4d8',
		textPrimary: '#18181b',
		textSecondary: '#52525b',
		danger: '#ef4444',
		dangerMuted: 'rgba(239, 68, 68, 0.2)',
		success: '#10b981',
		successMuted: 'rgba(16, 185, 129, 0.2)',
	},
}

const ACCENT_COLORS: Record<string, string> = {
	blue: '#3b82f6',
	indigo: '#6366f1',
	violet: '#8b5cf6',
	purple: '#a855f7',
	pink: '#ec4899',
	rose: '#f43f5e',
	orange: '#f97316',
	amber: '#f59e0b',
	emerald: '#10b981',
	teal: '#14b8a6',
}

function getThemePreference(): 'light' | 'dark' {
	// 1. Check data-theme attribute (if settings already applied)
	const dataTheme = document.documentElement.getAttribute('data-theme')
	if (dataTheme === 'light' || dataTheme === 'dark') return dataTheme

	// 2. Check localStorage (persisted preference)
	try {
		const stored = localStorage.getItem('crate-theme')
		if (stored === 'light' || stored === 'dark') return stored
		if (stored === 'system') {
			return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
		}
	} catch {
		// localStorage not available
	}

	// 3. Fall back to system preference
	return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
}

function getAccentColor(): string {
	// 1. Check data-accent attribute
	const dataAccent = document.documentElement.getAttribute('data-accent')
	if (dataAccent && ACCENT_COLORS[dataAccent]) return ACCENT_COLORS[dataAccent]

	// 2. Check localStorage
	try {
		const stored = localStorage.getItem('crate-accent')
		if (stored && ACCENT_COLORS[stored]) return ACCENT_COLORS[stored]
	} catch {
		// localStorage not available
	}

	// 3. Default to blue
	return ACCENT_COLORS.blue
}

// =============================================================================
// Crash Screen
// =============================================================================

const FALLBACK_CRASH_SCREEN_ID = 'early-crash-screen'
let svelteKitReady = false

function escapeHtml(text: string): string {
	const div = document.createElement('div')
	div.textContent = text
	return div.innerHTML
}

function showFallbackCrashScreen(error: EarlyError): void {
	// Don't show if SvelteKit has taken over or if already visible
	if (svelteKitReady) return
	if (document.getElementById(FALLBACK_CRASH_SCREEN_ID)) return

	// Get theme colors based on user preference
	const theme = getThemePreference()
	const colors = THEME_COLORS[theme]
	const accentColor = getAccentColor()

	const container = document.createElement('div')
	container.id = FALLBACK_CRASH_SCREEN_ID
	container.style.cssText = `
		position: fixed;
		inset: 0;
		z-index: 9999;
		display: flex;
		align-items: center;
		justify-content: center;
		background-color: ${colors.background};
		font-family: system-ui, -apple-system, sans-serif;
	`

	container.innerHTML = `
		<div style="
			width: 100%;
			max-width: 28rem;
			border-radius: 0.5rem;
			border: 1px solid ${colors.border};
			background-color: ${colors.card};
			padding: 1.5rem;
			box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5);
		">
			<div style="margin-bottom: 1rem; display: flex; align-items: center; gap: 0.75rem;">
				<div style="
					display: flex;
					height: 2.5rem;
					width: 2.5rem;
					align-items: center;
					justify-content: center;
					border-radius: 9999px;
					background-color: ${colors.dangerMuted};
				">
					<svg style="height: 1.25rem; width: 1.25rem; color: ${colors.danger};" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
					</svg>
				</div>
				<div>
					<h2 style="font-size: 1.125rem; font-weight: 600; color: ${colors.textPrimary}; margin: 0;">Something went wrong</h2>
					<p style="font-size: 0.875rem; color: ${colors.textSecondary}; margin: 0.25rem 0 0 0;">The application encountered an unexpected error.</p>
				</div>
			</div>

			<div style="
				margin-bottom: 1rem;
				border-radius: 0.375rem;
				border: 1px solid ${colors.border};
				background-color: ${colors.cardInner};
				padding: 0.75rem;
			">
				<div style="margin-bottom: 0.5rem; display: flex; align-items: center; justify-content: space-between;">
					<span style="font-size: 0.75rem; font-weight: 500; color: ${colors.textSecondary};">Error Details</span>
					<div style="position: relative; display: inline-flex;">
						<button
							id="early-crash-copy-btn"
							style="
								display: inline-flex;
								align-items: center;
								justify-content: center;
								width: 1.5rem;
								height: 1.5rem;
								border-radius: 0.375rem;
								border: none;
								background: transparent;
								color: ${colors.textSecondary};
								cursor: pointer;
								transition: background-color 0.15s, color 0.15s;
							"
							onmouseover="this.style.backgroundColor='${colors.card}'"
							onmouseout="this.style.backgroundColor='transparent'"
						>
							<svg style="width: 1rem; height: 1rem;" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
								<rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
								<path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
							</svg>
						</button>
						<div
							id="early-crash-tooltip"
							style="
								position: absolute;
								right: 100%;
								top: 50%;
								transform: translateY(-50%);
								margin-right: 0.5rem;
								padding: 0.25rem 0.5rem;
								border-radius: 0.25rem;
								border: 1px solid ${colors.border};
								background-color: ${colors.card};
								font-size: 0.75rem;
								font-weight: 500;
								white-space: nowrap;
								color: ${colors.textPrimary};
								box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);
								opacity: 0;
								pointer-events: none;
								transition: opacity 0.2s;
							"
						>Copied!</div>
					</div>
				</div>
				<div style="max-height: 8rem; overflow-y: auto;">
					<code style="
						display: block;
						font-size: 0.75rem;
						word-break: break-all;
						white-space: pre-wrap;
						color: ${colors.textSecondary};
						font-family: ui-monospace, monospace;
					">${escapeHtml(error.message)}</code>
				</div>
			</div>

			<div style="display: flex; justify-content: flex-end;">
				<button
					onclick="window.location.reload()"
					style="
						padding: 0.5rem 1rem;
						border-radius: 0.375rem;
						border: none;
						background-color: ${accentColor};
						color: white;
						font-size: 0.875rem;
						font-weight: 500;
						cursor: pointer;
					"
				>Reset Application</button>
			</div>
		</div>
	`

	document.body.appendChild(container)

	// Set up copy button handler
	const copyBtn = document.getElementById('early-crash-copy-btn')
	const tooltip = document.getElementById('early-crash-tooltip')

	if (copyBtn && tooltip) {
		copyBtn.addEventListener('click', async () => {
			const details = [
				`Timestamp: ${error.timestamp}`,
				`Message: ${error.message}`,
				error.stack ? `Stack Trace:\n${error.stack}` : '',
			]
				.filter(Boolean)
				.join('\n')

			try {
				await navigator.clipboard.writeText(details)
			} catch {
				// Fallback for clipboard API not available
				const textarea = document.createElement('textarea')
				textarea.value = details
				textarea.style.position = 'fixed'
				textarea.style.opacity = '0'
				document.body.appendChild(textarea)
				textarea.select()
				document.execCommand('copy')
				document.body.removeChild(textarea)
			}

			// Show tooltip
			tooltip.style.opacity = '1'

			// Change to checkmark icon
			copyBtn.innerHTML = `
				<svg style="width: 1rem; height: 1rem; color: ${colors.success};" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
					<polyline points="20 6 9 17 4 12"></polyline>
				</svg>
			`

			// Reset after 2 seconds
			setTimeout(() => {
				tooltip.style.opacity = '0'
				copyBtn.innerHTML = `
					<svg style="width: 1rem; height: 1rem;" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
						<rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
						<path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
					</svg>
				`
			}, 2000)
		})
	}
}

function handleError(event: ErrorEvent): void {
	if (svelteKitReady) return // Let SvelteKit handle it

	event.preventDefault()
	showFallbackCrashScreen({
		message: event.message || 'An unexpected error occurred',
		stack: event.error?.stack,
		timestamp: new Date().toISOString(),
	})
}

function handleRejection(event: PromiseRejectionEvent): void {
	if (svelteKitReady) return // Let SvelteKit handle it

	event.preventDefault()
	const reason = event.reason
	const message =
		reason instanceof Error ? reason.message : typeof reason === 'string' ? reason : 'Unhandled promise rejection'

	showFallbackCrashScreen({
		message,
		stack: reason instanceof Error ? reason.stack : undefined,
		timestamp: new Date().toISOString(),
	})
}

// Register handlers immediately
window.addEventListener('error', handleError)
window.addEventListener('unhandledrejection', handleRejection)

/**
 * Mark that SvelteKit has successfully initialized.
 * After this, early error handlers will defer to SvelteKit's error handling.
 */
export function markSvelteKitReady(): void {
	svelteKitReady = true

	// Remove fallback screen if it was shown
	const container = document.getElementById(FALLBACK_CRASH_SCREEN_ID)
	if (container) {
		container.remove()
	}

	// Remove early handlers (SvelteKit + useGlobalErrorHandler will handle errors now)
	window.removeEventListener('error', handleError)
	window.removeEventListener('unhandledrejection', handleRejection)
}
