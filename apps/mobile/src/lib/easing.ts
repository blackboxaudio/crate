/**
 * Cubic-bézier easing for Svelte JS transitions (fly / slide / fade), matching the CSS `--ease-fluid`
 * token in `style.css` — `cubic-bezier(0.32, 0.72, 0, 1)`, the iOS-sheet curve. Keeping the CSS-driven
 * sheets/drawers and the Svelte-driven transitions on one conventional, smooth curve avoids the flat
 * "linear" feel and keeps everything consistent.
 */
function cubicBezier(x1: number, y1: number, x2: number, y2: number): (t: number) => number {
	const ax = 3 * x1 - 3 * x2 + 1
	const bx = 3 * x2 - 6 * x1
	const cx = 3 * x1
	const ay = 3 * y1 - 3 * y2 + 1
	const by = 3 * y2 - 6 * y1
	const cy = 3 * y1
	const sampleX = (t: number) => ((ax * t + bx) * t + cx) * t
	const sampleY = (t: number) => ((ay * t + by) * t + cy) * t
	const slopeX = (t: number) => (3 * ax * t + 2 * bx) * t + cx

	return (x: number) => {
		if (x <= 0) return 0
		if (x >= 1) return 1
		// Newton–Raphson: invert x(t) for the given progress x, then evaluate y(t).
		let t = x
		for (let i = 0; i < 6; i++) {
			const dx = sampleX(t) - x
			if (Math.abs(dx) < 1e-5) break
			const slope = slopeX(t)
			if (Math.abs(slope) < 1e-6) break
			t -= dx / slope
		}
		return sampleY(t)
	}
}

export const easeFluid = cubicBezier(0.32, 0.72, 0, 1)
