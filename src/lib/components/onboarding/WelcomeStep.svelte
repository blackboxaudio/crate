<script lang="ts">
	import { Text } from '$lib/components/common'
	import { translate } from '$lib/i18n'
</script>

<div class="flex flex-col items-center gap-6 text-center">
	<div
		class="h-20 w-20 bg-brand-primary"
		style="-webkit-mask-image: url('/crate-logo.svg'); -webkit-mask-size: contain; -webkit-mask-repeat: no-repeat; -webkit-mask-position: center; mask-image: url('/crate-logo.svg'); mask-size: contain; mask-repeat: no-repeat; mask-position: center;"
	></div>
	<div class="flex flex-col gap-2">
		<Text variant="header-1" weight="bold"
			>{$translate('onboarding.welcome.title', { values: { appName: 'Crate' } })}</Text
		>
		<Text variant="body-1" color="secondary" class="max-w-sm">{$translate('onboarding.welcome.subtitle')}</Text>
	</div>
</div>
