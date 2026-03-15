<script lang="ts">
	import type { Track } from '$shared/types'

	// Prove $shared alias resolution works
	const placeholder: Track | null = null
</script>

<main>
	<h1>Crate Mobile</h1>
	<p>Scaffold placeholder — shared type resolution {placeholder === null ? 'works' : 'works'}.</p>
</main>
