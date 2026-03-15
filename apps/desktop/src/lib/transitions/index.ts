import { cubicOut } from 'svelte/easing'

type TransitionParams = {
	delay?: number
	duration?: number
}

/**
 * Custom two-stage transition for device items.
 *
 * Enter: height expands (0-50% of duration), then opacity + width (50-100%)
 * Exit: opacity + width collapses (0-50%), then height shrinks (50-100%)
 */
export function deviceItem(node: HTMLElement, params: TransitionParams = {}) {
	const { delay = 0, duration = 200 } = params
	const style = getComputedStyle(node)
	const height = parseFloat(style.height)
	const paddingTop = parseFloat(style.paddingTop)
	const paddingBottom = parseFloat(style.paddingBottom)
	const marginTop = parseFloat(style.marginTop)
	const marginBottom = parseFloat(style.marginBottom)

	return {
		delay,
		duration,
		easing: cubicOut,
		css: (t: number) => {
			// Two phases: height first (0-0.5), then opacity+scaleX (0.5-1)
			const heightProgress = Math.min(t * 2, 1) // 0->1 in first half
			const contentProgress = Math.max((t - 0.5) * 2, 0) // 0->1 in second half

			return `
				overflow: hidden;
				height: ${heightProgress * height}px;
				padding-top: ${heightProgress * paddingTop}px;
				padding-bottom: ${heightProgress * paddingBottom}px;
				margin-top: ${heightProgress * marginTop}px;
				margin-bottom: ${heightProgress * marginBottom}px;
				opacity: ${contentProgress};
				transform: scaleX(${0.8 + contentProgress * 0.2});
				transform-origin: left center;
			`
		},
	}
}
