<script lang="ts">
	import Icon from './Icon.svelte'

	type Props = {
		class?: string
	}

	let { class: className = 'h-4 w-4' }: Props = $props()
</script>

<Icon name="refresh" class="{className} animate-spin text-text-tertiary" />
