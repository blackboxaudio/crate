import { writable, derived } from 'svelte/store'
import { getStoredString, setStoredString } from '$shared/utils/storage'
import { detectSourceType } from '$shared/utils/discoveryLinks'
import { discoveryStore } from '$shared/stores/discovery'
import * as discoveryApi from '$shared/api/discovery'
import type { DiscoverySourceType, DiscoveryReleaseCreate } from '$shared/types'

const STORAGE_KEY = 'discovery.pendingQueue'

export type PendingStatus = 'queued' | 'fetching' | 'failed'

export interface PendingRelease {
	id: string
	url: string
	sourceType: DiscoverySourceType
	addedAt: number
	status: PendingStatus
}

interface PendingState {
	items: PendingRelease[]
	processing: boolean
}

let nextId = 0
function genId(): string {
	return `pending-${Date.now()}-${nextId++}`
}

const { subscribe, set, update } = writable<PendingState>({ items: [], processing: false })

function persist(items: PendingRelease[]) {
	const serializable = items.map((p) => ({ id: p.id, url: p.url, sourceType: p.sourceType, addedAt: p.addedAt }))
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

async function processQueue() {
	let state: PendingState | undefined
	const unsub = subscribe((s) => (state = s))
	unsub()

	if (!state || state.processing || state.items.length === 0) return
	if (!navigator.onLine) return

	update((s) => ({ ...s, processing: true }))

	const queued = state.items.filter((p) => p.status === 'queued' || p.status === 'failed')
	for (const pending of queued) {
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
				}))
			}

			await discoveryStore.createRelease(create)
			remove(pending.id)
		} catch {
			update((s) => ({
				...s,
				items: s.items.map((p) => (p.id === pending.id ? { ...p, status: 'failed' as PendingStatus } : p)),
			}))
		}
	}

	update((s) => ({ ...s, processing: false }))
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
		}>
		const items: PendingRelease[] = parsed.map((p) => ({
			id: p.id || genId(),
			url: p.url,
			sourceType: p.sourceType,
			addedAt: p.addedAt,
			status: 'queued' as PendingStatus,
		}))
		set({ items, processing: false })
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
