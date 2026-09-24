import type { Action } from 'svelte/action'

/**
 * Re-parent the element to `<body>` on mount so a fixed overlay escapes ancestor stacking
 * contexts. Every Drawer panel is `position: fixed` with an inline z-index, which makes it a
 * stacking context — so an overlay nested inside one (a sort/filter sheet inside a z-30 detail
 * drawer, a context menu on its rows) is capped at the ANCESTOR's layer no matter its own
 * z-index, and ends up under the z-40 mini player. `fixed` positioning is viewport-anchored
 * wherever the node lives, so moving it only changes the stacking scope, never the geometry.
 * Pass `false` to leave the element in place (lets callers make portaling a prop).
 */
export const portalToBody: Action<HTMLElement, boolean | undefined> = (node, enabled = true) => {
	if (enabled) document.body.appendChild(node)
	return {
		destroy() {
			// Svelte removes the node through its own reference on unmount; this is just a
			// belt-and-braces detach in case the surrounding teardown order ever changes.
			if (enabled) node.remove()
		},
	}
}
