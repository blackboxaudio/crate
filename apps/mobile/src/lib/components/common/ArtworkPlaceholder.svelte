<script lang="ts">
	// Polished placeholder tile for missing or failed artwork: a subtle gradient with a
	// music-note mark that scales with the tile. Replaces both the ad-hoc flat gray boxes
	// and WebKit's broken-image icon (the small blue question mark) that showed whenever a
	// cover URL was absent or dead. Pass the same `class` you'd give the <img> (sizing +
	// rounding); stray image-only utilities like `object-cover` are harmless on the div.
	type Props = { class?: string }
	let { class: className = '' }: Props = $props()
</script>

<div
	class="flex items-center justify-center overflow-hidden border border-stroke-subtle/50 bg-gradient-to-br from-surface-2 to-surface-1 {className}"
	aria-hidden="true"
>
	<svg viewBox="0 0 24 24" class="h-[38%] w-[38%] text-text-tertiary opacity-70" fill="currentColor">
		<path d="M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6zm-2 16a2 2 0 1 1 0-4 2 2 0 0 1 0 4z" />
	</svg>
</div>
