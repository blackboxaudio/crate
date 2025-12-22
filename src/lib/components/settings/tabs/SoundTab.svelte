<script lang="ts">
	import { Select, Text } from '$lib/components/common'
	import { settingsStore, audioDevice, audioDevices } from '$lib/stores/settings'
	import { translate } from '$lib/i18n'

	type SelectOption = { value: string; label: string }
	type SelectOptionGroup = { label: string; options: SelectOption[] }

	const audioDeviceOptions = $derived.by(() => {
		const defaultLabel = $translate('common.default')
		const systemDevices: SelectOption[] = [{ value: '', label: defaultLabel }]
		const externalDevices: SelectOption[] = []

		for (const device of $audioDevices) {
			const option: SelectOption = {
				value: device.name,
				label: device.isDefault ? `${device.name} (${defaultLabel})` : device.name,
			}

			if (device.isBuiltIn) {
				systemDevices.push(option)
			} else {
				externalDevices.push(option)
			}
		}

		const groups: SelectOptionGroup[] = [{ label: $translate('settings.sound.system'), options: systemDevices }]

		// Only add External section if there are external devices
		if (externalDevices.length > 0) {
			groups.push({ label: $translate('settings.sound.external'), options: externalDevices })
		}

		return groups
	})

	function handleAudioDeviceChange(value: string) {
		settingsStore.setAudioDevice(value === '' ? null : value)
	}
</script>

<div class="space-y-8">
	<!-- Output Device Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.sound.outputDevice')}</Text>
		<Text variant="caption" as="p" class="mb-2">{$translate('settings.sound.outputDeviceDescription')}</Text>
		<div class="max-w-md">
			<Select
				value={$audioDevice ?? ''}
				options={audioDeviceOptions}
				placeholder={$translate('settings.sound.systemDefault')}
				onchange={handleAudioDeviceChange}
			/>
		</div>
	</section>
</div>
