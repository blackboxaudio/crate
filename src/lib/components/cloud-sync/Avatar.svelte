<script lang="ts">
	type Props = {
		photoUrl?: string | null
		name?: string | null
		email?: string | null
		size?: number
		class?: string
	}

	let { photoUrl, name, email, size = 40, class: className = '' }: Props = $props()

	let loadError = $state(false)

	$effect(() => {
		// Reset error state when the URL changes so a new attempt is made.
		// eslint-disable-next-line @typescript-eslint/no-unused-expressions
		photoUrl
		loadError = false
	})

	const initials = $derived.by(() => {
		const source = (name ?? email ?? '').trim()
		if (!source) return ''
		const parts = source.split(/[\s@._-]+/).filter(Boolean)
		return parts
			.slice(0, 2)
			.map((part) => part[0]?.toUpperCase() ?? '')
			.join('')
	})

	const showImage = $derived(!!photoUrl && !loadError)
</script>

<div
	class="flex flex-shrink-0 items-center justify-center overflow-hidden rounded-full bg-brand-muted text-brand-primary {className}"
	style="width: {size}px; height: {size}px;"
>
	{#if showImage}
		<img
			src={photoUrl}
			alt={name ?? ''}
			class="h-full w-full object-cover"
			referrerpolicy="no-referrer"
			onerror={() => (loadError = true)}
		/>
	{:else}
		<span class="text-sm font-semibold">{initials || '?'}</span>
	{/if}
</div>
