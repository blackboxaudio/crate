<script lang="ts">
	import { translate } from '$shared/i18n'

	// Fixed top bar: hamburger (left), centered title, settings gear (right). Owns the top safe-area
	// inset so the status bar / notch never overlaps the controls. Both buttons are 44pt hit areas.
	type Props = {
		title: string
		onMenu: () => void
		onSettings: () => void
	}

	let { title, onMenu, onSettings }: Props = $props()
</script>

<header class="pt-safe fixed inset-x-0 top-0 z-30 border-b border-stroke-subtle bg-surface-1">
	<div class="relative flex h-14 items-center justify-between px-1">
		<button
			type="button"
			class="flex h-11 w-11 items-center justify-center rounded-md text-text-primary active:bg-surface-2"
			aria-label={$translate('nav.openMenu')}
			onclick={onMenu}
		>
			<svg
				viewBox="0 0 24 24"
				class="h-6 w-6"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
			>
				<line x1="3" y1="6" x2="21" y2="6" />
				<line x1="3" y1="12" x2="21" y2="12" />
				<line x1="3" y1="18" x2="21" y2="18" />
			</svg>
		</button>

		<h1
			class="pointer-events-none absolute inset-x-0 truncate px-14 text-center text-base font-medium text-text-primary"
		>
			{title}
		</h1>

		<button
			type="button"
			class="flex h-11 w-11 items-center justify-center rounded-md text-text-primary active:bg-surface-2"
			aria-label={$translate('nav.openSettings')}
			onclick={onSettings}
		>
			<svg
				viewBox="0 0 24 24"
				class="h-6 w-6"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
				stroke-linejoin="round"
			>
				<circle cx="12" cy="12" r="3" />
				<path
					d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"
				/>
			</svg>
		</button>
	</div>
</header>
