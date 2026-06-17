<script lang="ts">
	// Mobile app shell: a single Discovery main view wrapped in the navigation shell (header + left/
	// right drawers). The cloud sign-in flow (#133) now lives in the right drawer's sync section.
	import MobileShell from '$lib/components/layout/MobileShell.svelte'
	import MobileDiscoveryView from '$lib/components/discovery/MobileDiscoveryView.svelte'
</script>

<MobileShell>
	<MobileDiscoveryView />
</MobileShell>
