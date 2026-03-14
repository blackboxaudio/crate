<script lang="ts">
	import { Text } from '$lib/components/common'
	import Icon from '$lib/components/common/Icon.svelte'
	import { translate } from '$lib/i18n'
</script>

<div class="flex flex-col items-center gap-6 text-center">
	<div class="flex h-20 w-20 items-center justify-center rounded-full bg-brand-muted">
		<Icon name="check" class="h-10 w-10 text-brand-primary" />
	</div>
	<div class="flex flex-col gap-2">
		<Text variant="header-1" weight="bold">{$translate('onboarding.ready.title')}</Text>
		<Text variant="body-1" color="secondary">{$translate('onboarding.ready.subtitle')}</Text>
	</div>
</div>
