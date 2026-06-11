<script lang="ts">
	import { toasts, toastStore } from '$shared/stores/toast'
	import Toast from './Toast.svelte'
</script>

{#if $toasts.length > 0}
	<div class="fixed right-4 bottom-4 z-50 flex flex-col gap-2">
		{#each $toasts as toast (toast.id)}
			<Toast {toast} onDismiss={() => toastStore.dismiss(toast.id)} />
		{/each}
	</div>
{/if}
