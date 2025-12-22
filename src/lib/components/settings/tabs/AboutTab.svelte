<script lang="ts">
	import { Text } from '$lib/components/common'
	import { appInfo } from '$lib/stores/app'
	import { translate } from '$lib/i18n'
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
</div>
