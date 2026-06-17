<script lang="ts">
	import type { Snippet } from 'svelte'
	import { onMount } from 'svelte'
	import { translate } from '$shared/i18n'
	import { mobileUIStore, openDrawer, isLeftOpen, isRightOpen, isAnyDrawerOpen } from '$lib/stores/mobileUI'
	import { swipe } from '$lib/actions/swipe'
	import Header from './Header.svelte'
	import Backdrop from './Backdrop.svelte'
	import LeftDrawer from './LeftDrawer.svelte'
	import RightDrawer from './RightDrawer.svelte'

	// Composition root for the mobile app: fixed header + scrollable content (the page, slotted as
	// `children`) + dimming backdrop + the two drawers. Owns the left-edge open gesture and the drawer
	// width used to scale its finger-follow progress.
	type Props = {
		children: Snippet
	}

	let { children }: Props = $props()

	// Drives the left drawer's finger-follow while opening via the left-edge swipe.
	let leftDrag = $state<number | null>(null)

	// Track viewport width so the edge-open gesture scales progress against the real drawer width
	// (85% capped at 320px — must match the drawer's `w-[85%] max-w-[320px]`).
	let winW = $state(typeof window !== 'undefined' ? window.innerWidth : 360)
	onMount(() => {
		const onResize = () => (winW = window.innerWidth)
		window.addEventListener('resize', onResize)
		return () => window.removeEventListener('resize', onResize)
	})
	const drawerWidthPx = $derived(Math.min(winW * 0.85, 320))

	// How far in from the left screen edge a swipe can start and still grab the drawer-open gesture.
	// Wider than the default so the gesture is forgiving and doesn't require hugging the bezel.
	const EDGE_SWIPE_ZONE = 64
</script>

<div class="relative h-dvh w-screen overflow-hidden bg-surface-0">
	<Header
		title={$translate('nav.discovery')}
		onMenu={mobileUIStore.toggleLeft}
		onSettings={mobileUIStore.toggleRight}
	/>

	<main
		class="pb-safe h-full overflow-y-auto"
		style="padding-top: calc(3.5rem + env(safe-area-inset-top))"
		use:swipe={{
			side: 'left',
			mode: 'open',
			enabled: $openDrawer === null,
			width: drawerWidthPx,
			edgeSize: EDGE_SWIPE_ZONE,
			onProgress: (o) => (leftDrag = o),
			onOpen: () => {
				leftDrag = null
				mobileUIStore.openLeft()
			},
			onClose: () => (leftDrag = null),
		}}
	>
		{@render children()}
	</main>

	<Backdrop show={$isAnyDrawerOpen} onclick={mobileUIStore.close} />

	<LeftDrawer open={$isLeftOpen} dragOpenness={leftDrag} widthPx={drawerWidthPx} onClose={mobileUIStore.close} />
	<RightDrawer open={$isRightOpen} widthPx={drawerWidthPx} onClose={mobileUIStore.close} />
</div>
