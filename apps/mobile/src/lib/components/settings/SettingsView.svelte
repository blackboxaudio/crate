<script lang="ts">
	import { tick } from 'svelte'
	import { mobileUIStore, settingsPage } from '$lib/stores/mobileUI'
	import { registerBackLayer } from '$lib/androidBack'
	import { swipe, type SwipeOptions } from '$lib/actions/swipe'
	import { easeFluid } from '$lib/easing'
	import SettingsRoot from './pages/SettingsRoot.svelte'
	import GeneralPage from './pages/GeneralPage.svelte'
	import AppearancePage from './pages/AppearancePage.svelte'
	import FollowingPage from './pages/FollowingPage.svelte'
	import CollectionPage from './pages/CollectionPage.svelte'
	import CloudSyncPage from './pages/CloudSyncPage.svelte'
	import StoragePage from './pages/StoragePage.svelte'
	import AboutPage from './pages/AboutPage.svelte'

	// iOS-style two-level settings: a root grouped list that pushes flat sub-pages with an in-place
	// {#key} slide styled as a UINavigationController push (see levelTransition). One
	// Drawer, one scrim, one header (SettingsDrawer) — nesting a second Drawer per page would stack
	// scrims into the expanded player's z band and duplicate chrome for a hierarchy of exactly two
	// levels. Each page is pure content; this router owns the transition, the sub-page edge-swipe
	// back, and the Android back layer.
	const page = $derived($settingsPage)

	let reduceMotion = $state(false)
	$effect(() => {
		const mq = window.matchMedia('(prefers-reduced-motion: reduce)')
		reduceMotion = mq.matches
		const onMq = () => (reduceMotion = mq.matches)
		mq.addEventListener('change', onMq)
		return () => mq.removeEventListener('change', onMq)
	})

	// Two levels only, so the direction falls out of the destination: any move to a sub-page slides
	// forward, any move to root slides back. Set in a pre-effect so the outgoing/incoming transitions
	// (which read it as they build) see the fresh value regardless of which caller navigated.
	/* eslint-disable svelte/prefer-writable-derived */
	let navDirection = $state<'forward' | 'back'>('forward')
	$effect.pre(() => {
		navDirection = $settingsPage === 'root' ? 'back' : 'forward'
	})

	// iOS navigation push, like UINavigationController: the sub-page layer does the full-width slide
	// (forward: enters from the right; back: exits to the right) stacked ABOVE, while the root does a
	// one-third parallax underneath. No cross-fade — the layers are opaque (bg-surface-0 below) and
	// occlude each other, so the in/out durations must match for the pair to track as one surface.
	function levelTransition(_node: Element, { incoming }: { incoming: boolean }) {
		if (reduceMotion) return { duration: 0 }
		const isSubPageLayer = (navDirection === 'forward') === incoming
		return {
			duration: 350,
			easing: easeFluid,
			css: (_t: number, u: number) =>
				isSubPageLayer
					? `transform: translateX(${u * 100}%); z-index: 1;`
					: `transform: translateX(${-u * 30}%); z-index: 0;`,
		}
	}

	function pop() {
		mobileUIStore.setSettingsPage('root')
	}

	// iOS left-edge swipe on a sub-page pops to root. The Drawer's own edge-swipe (dismiss) is
	// disabled on sub-pages via `panelDrag`, so the two gestures never share the edge.
	const backSwipe = $derived<SwipeOptions>({
		side: 'right',
		mode: 'close',
		closeEdgeFrom: 'left',
		closeEdgeSize: 24,
		enabled: page !== 'root',
		onClose: pop,
	})

	// Android Back pops a sub-page before the Drawer's own layer closes the whole drawer. The
	// registration is deferred a tick: on a deep-link mount (sync chip → Cloud Sync) this child's
	// effects flush BEFORE the parent Drawer registers its layer, so a synchronous registration
	// would land under it and Back would close the whole drawer instead of popping.
	$effect(() => {
		if ($settingsPage === 'root') return
		let un: (() => void) | undefined
		let alive = true
		void tick().then(() => {
			if (alive) un = registerBackLayer(() => mobileUIStore.setSettingsPage('root'))
		})
		return () => {
			alive = false
			un?.()
		}
	})
</script>

<div class="relative h-full overflow-hidden" use:swipe={backSwipe}>
	{#key page}
		<div
			class="absolute inset-0 overflow-y-auto bg-surface-0 pt-2"
			style="padding-bottom: var(--mini-player-inset, 0px)"
			in:levelTransition|local={{ incoming: true }}
			out:levelTransition|local={{ incoming: false }}
		>
			{#if page === 'root'}
				<SettingsRoot />
			{:else if page === 'general'}
				<GeneralPage />
			{:else if page === 'appearance'}
				<AppearancePage />
			{:else if page === 'following'}
				<FollowingPage />
			{:else if page === 'collection'}
				<CollectionPage />
			{:else if page === 'cloudSync'}
				<CloudSyncPage />
			{:else if page === 'storage'}
				<StoragePage />
			{:else}
				<AboutPage />
			{/if}
		</div>
	{/key}
</div>
