<script lang="ts">
	// The mobile app's loading spinner — a spinning refresh ("arrow-path") icon, matching the desktop
	// spinner (Icon name="refresh" + animate-spin). The glyph is duplicated here because mobile can't
	// import desktop-only components (the shared/desktop boundary). Decorative: the surrounding control
	// (button/row) owns the accessible label, so this is aria-hidden.
	type Props = {
		class?: string
	}
	let { class: className = 'h-4 w-4' }: Props = $props()
</script>

<svg
	class="{className} animate-spin"
	viewBox="0 0 24 24"
	fill="none"
	stroke="currentColor"
	stroke-width="2"
	stroke-linecap="round"
	stroke-linejoin="round"
	aria-hidden="true"
>
	<path
		d="M20 4v5h-.582m-15.356 2A8.001 8.001 0 01 19.418 9m0 0H15m-11 11v-5h.581m0 0a8.003 8.003 0 00 15.357-2m-15.357 2H9"
	/>
</svg>
