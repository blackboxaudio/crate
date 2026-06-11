<script lang="ts">
	// Scaffold placeholder. Imports from `$shared` to prove the alias resolves at both build time
	// (Vite) and type-check time (svelte-check). Real mobile UI is implemented in follow-up issues.
	import type { Track } from '$shared/types'

	const placeholder: Pick<Track, 'id'> = { id: 'mobile-scaffold' }
</script>

<main style="font-family: sans-serif; padding: 2rem;">
	<h1>Crate Mobile</h1>
	<p>Scaffold ready — shared types resolve via <code>$shared</code> ({placeholder.id}).</p>
</main>
