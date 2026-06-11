<script lang="ts">
	import { page } from '$app/stores'
	import Button from '$lib/components/common/Button.svelte'
	import Icon from '$lib/components/common/Icon.svelte'
	import IconButton from '$lib/components/common/IconButton.svelte'
	import Tooltip from '$lib/components/common/Tooltip.svelte'

	let copySuccess = $state(false)
	let copyTooltip: ReturnType<typeof Tooltip> | undefined = $state()

	async function handleCopyError() {
		const details = [
			`Status: ${$page.status}`,
			`Message: ${$page.error?.message || 'Unknown error'}`,
			`URL: ${$page.url.pathname}`,
			`Timestamp: ${new Date().toISOString()}`,
		].join('\n')

		try {
			await navigator.clipboard.writeText(details)
			copySuccess = true
			copyTooltip?.show('Copied!')
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
			copyTooltip?.show('Copied!')
			setTimeout(() => {
				copySuccess = false
			}, 2000)
		}
	}

	function handleReset() {
		window.location.reload()
	}
</script>

<div class="fixed inset-0 z-[9999] flex items-center justify-center bg-surface-0">
	<div class="w-full max-w-md rounded-lg border border-stroke bg-surface-1 p-6 shadow-xl">
		<!-- Header -->
		<div class="mb-4 flex items-center gap-3">
			<div class="flex h-10 w-10 items-center justify-center rounded-full bg-danger/20">
				<Icon name="alert-circle" class="h-5 w-5 text-danger" />
			</div>
			<div>
				<h2 class="text-lg font-semibold text-text-primary">Something went wrong</h2>
				<p class="text-sm text-text-secondary">The application encountered an unexpected error.</p>
			</div>
		</div>

		<!-- Error details box -->
		<div class="mb-4 rounded-md border border-stroke bg-surface-2 p-3">
			<div class="mb-2 flex items-center justify-between">
				<span class="text-xs font-medium text-text-secondary">Error Details</span>
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
					{$page.error?.message || 'Unknown error'}
				</code>
			</div>
		</div>

		<!-- Action buttons -->
		<div class="flex justify-end">
			<Button variant="primary" onclick={handleReset}>Reset Application</Button>
		</div>
	</div>
</div>
