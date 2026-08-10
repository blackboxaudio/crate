import { derived } from 'svelte/store'

import { previewInfo } from '$shared/stores/player'

// Trailing scroll padding that clears the floating mini-player card (MiniPlayer.svelte: ~4rem of card +
// the gaps around it). Two values because the two scroll contexts measure from different bottoms:
//
// - Shell tab views scroll inside <main>'s content box — MobileShell already pads the tab bar + bottom
//   safe-area below them, so they only need the card's clearance above that box.
// - The full-screen detail overlays (release / playlist / tag / follow drill-ins) own the physical screen
//   bottom, and the card drops by the tab bar's height while one is open (it floats just above the
//   safe-area — see MiniPlayer), so the safe-area inset must be part of their padding. They keep a small
//   base even with no preview active so the last row still clears the home indicator.
//
// MobileShell publishes the shell value as `--mini-player-inset` for its subtree; each overlay publishes
// the overlay value under the same name via Drawer's `style` (overlays mount OUTSIDE the shell's div, so
// they'd otherwise resolve the variable to its 0px fallback).
export const shellMiniPlayerInset = derived(previewInfo, ($previewInfo) => ($previewInfo ? '5rem' : '0px'))

export const overlayMiniPlayerInset = derived(previewInfo, ($previewInfo) =>
	$previewInfo ? 'calc(env(safe-area-inset-bottom) + 5.5rem)' : 'calc(env(safe-area-inset-bottom) + 0.75rem)'
)
