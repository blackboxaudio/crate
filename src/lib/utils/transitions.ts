import { cubicOut } from 'svelte/easing'
import type { EasingFunction } from 'svelte/transition'

type FlyBlurParams = {
	delay?: number
	duration?: number
	x?: number
	blurAmount?: number
	easing?: EasingFunction
}

export function flyBlur(node: HTMLElement, params: FlyBlurParams = {}) {
	const { delay = 0, duration = 300, x = 0, blurAmount = 4, easing = cubicOut } = params

	const style = getComputedStyle(node)
	const opacity = +style.opacity
	const transform = style.transform === 'none' ? '' : style.transform

	return {
		delay,
		duration,
		easing,
		css: (t: number) => `
			transform: ${transform} translateX(${(1 - t) * x}px);
			opacity: ${t * opacity};
			filter: blur(${(1 - t) * blurAmount}px);
		`,
	}
}
