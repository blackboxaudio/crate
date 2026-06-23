<script lang="ts">
	import { translate } from '$shared/i18n'
	import { mobileUIStore, addReleaseOpen } from '$lib/stores/mobileUI'
	import MobileModal from '$lib/components/common/MobileModal.svelte'

	// Entry-point placeholder for adding a discovery release. Issue #53 ships the toolbar / empty-state
	// buttons that open this sheet; the functional flow — URL input, source detection, metadata fetch,
	// editable preview, save, plus the offline queue and clipboard detection — is built in issue #56, which
	// replaces this body. Kept as a real (openable) sheet so the wiring is exercised end-to-end now.
</script>

<MobileModal open={$addReleaseOpen} onClose={mobileUIStore.closeAddRelease} title={$translate('discovery.addRelease')}>
	<p class="py-4 text-center text-sm text-text-secondary">
		{$translate('discovery.mobileAddHint')}
	</p>
</MobileModal>
