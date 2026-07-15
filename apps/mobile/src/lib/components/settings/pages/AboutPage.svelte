<script lang="ts">
	import { onMount } from 'svelte'
	import { openUrl } from '@tauri-apps/plugin-opener'
	import { translate } from '$shared/i18n'
	import { getAppInfo } from '$shared/api/app'
	// @ts-expect-error — PUBLIC_APP_VERSION is set dynamically by vite.config.ts
	import { PUBLIC_APP_VERSION } from '$env/static/public'

	// About: version, environment, project link. Environment (dev/staging/prod) tells TestFlight /
	// sideloaded builds apart; the row hides if the command fails.
	let environment = $state<string | null>(null)
	onMount(async () => {
		try {
			environment = (await getAppInfo()).environment
		} catch {
			environment = null
		}
	})
</script>

<div class="px-4 py-2">
	<div class="flex flex-col gap-2">
		<div class="flex items-center justify-between">
			<p class="text-sm text-text-secondary">{$translate('settings.about.version')}</p>
			<p class="text-sm text-text-primary">{PUBLIC_APP_VERSION}</p>
		</div>
		{#if environment}
			<div class="flex items-center justify-between">
				<p class="text-sm text-text-secondary">{$translate('settings.about.environment')}</p>
				<p class="text-sm text-text-primary">{environment}</p>
			</div>
		{/if}
		<div class="flex items-center justify-between">
			<p class="text-sm text-text-secondary">{$translate('settings.about.project')}</p>
			<button
				type="button"
				class="text-sm font-medium text-brand-primary active:opacity-70"
				onclick={() => void openUrl('https://github.com/blackboxaudio/crate')}
			>
				GitHub
			</button>
		</div>
	</div>
</div>
