<script lang="ts">
	import { crashStore, hasCrashed, crashError } from '$lib/stores/crash'
	import { resolvedTheme, accentColor } from '$lib/stores/settings'
	import Icon from './Icon.svelte'
	import IconButton from './IconButton.svelte'
	import Button from './Button.svelte'
	import Tooltip from './Tooltip.svelte'
	import { translate } from '$lib/i18n'
	import { get } from 'svelte/store'

	let copySuccess = $state(false)
	let copyTooltip: ReturnType<typeof Tooltip> | undefined = $state()

	// Ensure theme attributes are applied when crash screen shows
	$effect(() => {
		if ($hasCrashed) {
			const html = document.documentElement
			if (!html.hasAttribute('data-theme')) {
				html.setAttribute('data-theme', $resolvedTheme)
			}
			if (!html.hasAttribute('data-accent')) {
				html.setAttribute('data-accent', $accentColor)
			}
		}
	})

	async function handleCopyError() {
		let state: { hasCrashed: boolean; error: import('$lib/stores/crash').CrashInfo | null }
		const unsubscribe = crashStore.subscribe((s) => {
			state = s
		})
		unsubscribe()

		const details = crashStore.formatErrorDetails(state!)

		try {
			await navigator.clipboard.writeText(details)
			copySuccess = true
			copyTooltip?.show(get(translate)('settings.diagnostics.copied'))
			setTimeout(() => {
				copySuccess = false
			}, 2000)
		} catch {
			// Fallback: create a temporary textarea and copy
			const textarea = document.createElement('textarea')
			textarea.value = details
			textarea.style.position = 'fixed'
			textarea.style.opacity = '0'
			document.body.appendChild(textarea)
			textarea.select()
			document.execCommand('copy')
			document.body.removeChild(textarea)
			copySuccess = true
			copyTooltip?.show(get(translate)('settings.diagnostics.copied'))
			setTimeout(() => {
				copySuccess = false
			}, 2000)
		}
	}

	function handleReset() {
		window.location.reload()
	}
</script>

{#if $hasCrashed}
	<!-- Full-screen overlay that covers everything -->
	<div class="fixed inset-0 z-[9999] flex items-center justify-center bg-surface-0">
		<!-- Modal container -->
		<div class="w-full max-w-md rounded-lg border border-stroke bg-surface-1 p-6 shadow-xl">
			<!-- Header -->
			<div class="mb-4 flex items-center gap-3">
				<div class="flex h-10 w-10 items-center justify-center rounded-full bg-danger/20">
					<Icon name="alert-circle" class="h-5 w-5 text-danger" />
				</div>
				<div>
					<h2 class="text-lg font-semibold text-text-primary">{$translate('crash.title')}</h2>
					<p class="text-sm text-text-secondary">{$translate('crash.description')}</p>
				</div>
			</div>

			<!-- Error details box -->
			<div class="mb-4 rounded-md border border-stroke bg-surface-2 p-3">
				<div class="mb-2 flex items-center justify-between">
					<span class="text-xs font-medium text-text-secondary">{$translate('crash.errorDetails')}</span>
					<Tooltip bind:this={copyTooltip} position="left">
						<IconButton
							size="sm"
							icon={copySuccess ? 'check' : 'copy'}
							iconClass="h-4 w-4 {copySuccess ? 'text-success' : ''}"
							onclick={handleCopyError}
						/>
					</Tooltip>
				</div>
				<div class="max-h-32 overflow-y-auto">
					<code class="block text-xs break-all whitespace-pre-wrap text-text-secondary">
						{$crashError?.message || 'Unknown error'}
					</code>
				</div>
			</div>

			<!-- Action buttons -->
			<div class="flex justify-end">
				<Button variant="primary" onclick={handleReset}>{$translate('crash.resetApp')}</Button>
			</div>
		</div>
	</div>
{/if}
