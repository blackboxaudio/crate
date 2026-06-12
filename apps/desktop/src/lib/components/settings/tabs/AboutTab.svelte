<script lang="ts">
	import { Button, Text } from '$lib/components/common'
	import { appInfo } from '$lib/stores/app'
	import { updaterStore } from '$lib/stores/updater'
	import { translate } from '$shared/i18n'

	const statusLabel = $derived.by(() => {
		switch ($updaterStore.status) {
			case 'checking':
				return $translate('common.loading')
			case 'upToDate':
				return $translate('settings.about.upToDate')
			case 'available':
				return $translate('settings.about.updateAvailable', { values: { version: $updaterStore.version ?? '' } })
			case 'error':
				return $translate('errors.updateCheckFailed')
			default:
				return ''
		}
	})
</script>

<div class="space-y-8">
	<!-- Application Section -->
	<section>
		<Text variant="header-3" class="mb-4">{$translate('settings.about.application')}</Text>
		<div class="space-y-3">
			<div class="flex justify-between">
				<Text size="sm" color="secondary" as="span">{$translate('settings.about.version')}</Text>
				<Text size="sm" as="span">{$appInfo?.version ?? $translate('common.unknown')}</Text>
			</div>
			<div class="flex justify-between">
				<Text size="sm" color="secondary" as="span">{$translate('settings.about.environment')}</Text>
				<Text size="sm" as="span" class="capitalize">{$appInfo?.environment ?? $translate('common.unknown')}</Text>
			</div>
			<div class="flex justify-between">
				<Text size="sm" color="secondary" as="span">{$translate('settings.diagnostics.dataDirectory')}</Text>
				<Text variant="code" truncate class="max-w-xs" title={$appInfo?.dataDir}>
					{$appInfo?.dataDir ?? $translate('common.unknown')}
				</Text>
			</div>
		</div>
	</section>

	<!-- Updates Section -->
	<section>
		<Text variant="header-3" class="mb-4">{$translate('settings.about.updates')}</Text>
		<div class="flex items-center justify-between">
			<Text size="sm" color="secondary" as="span">{statusLabel}</Text>
			<Button
				size="sm"
				variant="secondary"
				disabled={$updaterStore.status === 'checking'}
				onclick={() => updaterStore.check(false)}
			>
				{$translate('settings.about.checkForUpdates')}
			</Button>
		</div>
	</section>
</div>
