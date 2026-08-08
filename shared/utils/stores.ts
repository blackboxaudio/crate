import { readable, type Readable } from 'svelte/store'

/**
 * Wraps a store so subscribers are only notified when the value actually changed.
 *
 * Svelte's `derived` re-emits on EVERY source emission when the value is an object or function
 * (`safe_not_equal` treats those as always-changed) — so an object-valued derived store fans a
 * notification out to every subscriber each time its source ticks, even when it re-selected the
 * exact same object. That matters for the player store, which ticks at 10Hz while playing: without
 * dedupe, every mounted feed row subscribed to `previewInfo` re-evaluates 10×/sec. Primitives
 * don't need this — svelte stores already skip notification for unchanged primitive values.
 *
 * `equals` defaults to reference equality (`Object.is`), the right check for a selected sub-object
 * that only gets a new identity when it genuinely changes. Pass a custom `equals` for values
 * rebuilt each emission (e.g. Sets — see `setsEqual`).
 */
export function dedupe<T>(source: Readable<T>, equals: (a: T, b: T) => boolean = Object.is): Readable<T> {
	let current: T
	let primed = false
	return readable<T>(undefined as T, (set) => {
		const unsubscribe = source.subscribe((value) => {
			if (primed && equals(current, value)) return
			primed = true
			current = value
			set(value)
		})
		return () => {
			unsubscribe()
			// The cached value survives an unsubscribe on purpose: it's still the source's latest
			// value, so a resubscribe shouldn't re-notify for it. `primed` stays true.
		}
	})
}

/** Membership equality for Set-valued stores that rebuild their Set on every emission. */
export function setsEqual<T>(a: Set<T>, b: Set<T>): boolean {
	if (a === b) return true
	if (a.size !== b.size) return false
	for (const value of a) {
		if (!b.has(value)) return false
	}
	return true
}
