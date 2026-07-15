import { writable, derived } from 'svelte/store'
import { getStoredString, setStoredString } from '$shared/utils/storage'
import { detectSourceType } from '$shared/utils/discoveryLinks'
import { discoveryStore } from '$shared/stores/discovery'
import * as discoveryApi from '$shared/api/discovery'
import type { DiscoverySourceType, DiscoveryReleaseCreate } from '$shared/types'

const STORAGE_KEY = 'discovery.pendingQueue'

// Exponential backoff for metadata-fetch retries, mirroring the follow watch loop
// (src-tauri/src/services/follow/watch.rs): 5 min base, doubling per attempt, capped at 6 h. A
// failed URL retries on this schedule (and immediately when connectivity returns) rather than
// stalling until the app restarts.
const BACKOFF_BASE_MS = 5 * 60 * 1000
const BACKOFF_MAX_MS = 6 * 60 * 60 * 1000

function backoffMs(attempts: number): number {
	const shift = Math.min(Math.max(attempts - 1, 0), 10)
	return Math.min(BACKOFF_BASE_MS * 2 ** shift, BACKOFF_MAX_MS)
}

export type PendingStatus = 'queued' | 'fetching' | 'failed'

export interface PendingRelease {
	id: string
	url: string
	sourceType: DiscoverySourceType
	addedAt: number
	status: PendingStatus
	/** Number of failed fetch attempts (drives the backoff schedule). */
	attempts: number
	/** Epoch ms before which this item should not be retried (0 = eligible now). */
	nextRetryAt: number
}

interface PendingState {
	items: PendingRelease[]
	processing: boolean
}

let nextId = 0
function genId(): string {
	return `pending-${Date.now()}-${nextId++}`
}

let retryTimer: ReturnType<typeof setTimeout> | null = null

const { subscribe, set, update } = writable<PendingState>({ items: [], processing: false })

function persist(items: PendingRelease[]) {
	const serializable = items.map((p) => ({
		id: p.id,
		url: p.url,
		sourceType: p.sourceType,
		addedAt: p.addedAt,
		attempts: p.attempts,
		nextRetryAt: p.nextRetryAt,
	}))
	setStoredString(STORAGE_KEY, JSON.stringify(serializable))
}

function enqueue(url: string) {
	update((s) => {
		if (s.items.some((p) => p.url === url)) return s
		const item: PendingRelease = {
			id: genId(),
			url,
			sourceType: detectSourceType(url),
			addedAt: Date.now(),
			status: 'queued',
			attempts: 0,
			nextRetryAt: 0,
		}
		const items = [...s.items, item]
		persist(items)
		return { ...s, items }
	})
}

function remove(id: string) {
	update((s) => {
		const items = s.items.filter((p) => p.id !== id)
		persist(items)
		return { ...s, items }
	})
}

/**
 * Schedule the next retry sweep for the soonest item still in backoff, so failed URLs retry on
 * their own timer without needing an `online` event or an app restart. Replaces any pending timer.
 */
function scheduleNextRetry(items: PendingRelease[]) {
	if (retryTimer) {
		clearTimeout(retryTimer)
		retryTimer = null
	}
	const now = Date.now()
	const soonest = items
		.filter((p) => p.status === 'failed' && p.nextRetryAt > now)
		.reduce((min, p) => Math.min(min, p.nextRetryAt), Number.POSITIVE_INFINITY)
	if (!Number.isFinite(soonest)) return
	// Cap the delay so a very large backoff still schedules a bounded timer.
	const delay = Math.min(Math.max(soonest - now, 0), BACKOFF_MAX_MS)
	retryTimer = setTimeout(() => void processQueue(), delay)
}

async function processQueue() {
	let state: PendingState | undefined
	const unsub = subscribe((s) => (state = s))
	unsub()

	if (!state || state.processing || state.items.length === 0) return
	if (!navigator.onLine) return

	update((s) => ({ ...s, processing: true }))

	// Eligible = queued, or failed whose backoff window has elapsed. Items still in backoff are
	// left for the retry timer (or the next `online` event).
	const now = Date.now()
	const eligible = state.items.filter((p) => p.status === 'queued' || (p.status === 'failed' && p.nextRetryAt <= now))
	for (const pending of eligible) {
		update((s) => ({
			...s,
			items: s.items.map((p) => (p.id === pending.id ? { ...p, status: 'fetching' as PendingStatus } : p)),
		}))

		try {
			const metadata = await discoveryApi.fetchMetadata(pending.url)
			const create: DiscoveryReleaseCreate = {
				url: pending.url,
				source_type: (metadata.source_type as DiscoverySourceType) || pending.sourceType,
			}
			if (metadata.artist) create.artist = metadata.artist
			if (metadata.title) create.title = metadata.title
			if (metadata.label) create.label = metadata.label
			if (metadata.release_date) create.release_date = metadata.release_date
			if (metadata.artwork_url) create.artwork_url = metadata.artwork_url
			if (metadata.parent_url) create.parent_url = metadata.parent_url
			if (metadata.tracks.length > 0) {
				create.tracks = metadata.tracks.map((t) => ({
					name: t.name,
					position: t.position,
					duration_ms: t.duration_ms ?? undefined,
					video_id: t.video_id ?? undefined,
					url: t.url ?? undefined,
				}))
			}

			await discoveryStore.createRelease(create)
			remove(pending.id)
		} catch {
			// Bump the attempt count and push the next retry out on the backoff schedule.
			update((s) => {
				const items = s.items.map((p) => {
					if (p.id !== pending.id) return p
					const attempts = p.attempts + 1
					return {
						...p,
						status: 'failed' as PendingStatus,
						attempts,
						nextRetryAt: Date.now() + backoffMs(attempts),
					}
				})
				persist(items)
				return { ...s, items }
			})
		}
	}

	let latest: PendingState | undefined
	const unsub2 = subscribe((s) => (latest = s))
	unsub2()
	update((s) => ({ ...s, processing: false }))
	if (latest) scheduleNextRetry(latest.items)
}

function hydrate() {
	const raw = getStoredString(STORAGE_KEY, '')
	if (!raw) return
	try {
		const parsed = JSON.parse(raw) as Array<{
			id: string
			url: string
			sourceType: DiscoverySourceType
			addedAt: number
			attempts?: number
			nextRetryAt?: number
		}>
		const items: PendingRelease[] = parsed.map((p) => {
			const attempts = p.attempts ?? 0
			const nextRetryAt = p.nextRetryAt ?? 0
			// Preserve backoff across restarts: an item still inside its window stays 'failed' so the
			// retry timer picks it up, rather than being retried immediately on boot.
			const status: PendingStatus = attempts > 0 && nextRetryAt > Date.now() ? 'failed' : 'queued'
			return {
				id: p.id || genId(),
				url: p.url,
				sourceType: p.sourceType,
				addedAt: p.addedAt,
				status,
				attempts,
				nextRetryAt,
			}
		})
		set({ items, processing: false })
		scheduleNextRetry(items)
	} catch {
		// Corrupt data — start fresh
	}
}

let listenersAttached = false
function attachNetworkListeners() {
	if (listenersAttached) return
	listenersAttached = true
	window.addEventListener('online', () => void processQueue())
}

export const pendingReleasesStore = {
	subscribe,
	enqueue,
	remove,
	processQueue,
	hydrate,
	attachNetworkListeners,
}

export const pendingReleases = derived(pendingReleasesStore, ($s) => $s.items)
export const hasPendingReleases = derived(pendingReleasesStore, ($s) => $s.items.length > 0)
