import { writable, derived } from 'svelte/store'
import type { CollectionAccount, CollectionItem, CollectionRefreshSummary } from '../types'
import * as collectionApi from '../api/collection'
import { toastStore } from './toast'

interface CollectionState {
	accounts: CollectionAccount[]
	items: CollectionItem[]
	/** Derived ownership id-sets (see CollectionOwnership). */
	fullyOwned: Set<string>
	partiallyOwned: Set<string>
	ownedTracks: Set<string>
	loading: boolean
	linking: boolean
	refreshingIds: Set<string>
	refreshingAll: boolean
}

const initialState: CollectionState = {
	accounts: [],
	items: [],
	fullyOwned: new Set(),
	partiallyOwned: new Set(),
	ownedTracks: new Set(),
	loading: false,
	linking: false,
	refreshingIds: new Set(),
	refreshingAll: false,
}

function errMsg(error: unknown, fallback: string): string {
	return typeof error === 'string' ? error : error instanceof Error ? error.message : fallback
}

function createCollectionStore() {
	const { subscribe, set, update } = writable<CollectionState>(initialState)

	// Coalesce concurrent ownership/item refreshes (boot + events can overlap).
	let inFlight: Promise<void> | null = null

	async function refreshData(): Promise<void> {
		if (inFlight) return inFlight
		inFlight = (async () => {
			try {
				const [accounts, items, ownership] = await Promise.all([
					collectionApi.getCollectionAccounts(),
					collectionApi.getCollectionItems(),
					collectionApi.getCollectionOwnership(),
				])
				update((s) => ({
					...s,
					accounts,
					items,
					fullyOwned: new Set(ownership.fullyOwnedReleaseIds),
					partiallyOwned: new Set(ownership.partiallyOwnedReleaseIds),
					ownedTracks: new Set(ownership.ownedTrackIds),
					loading: false,
				}))
			} catch {
				// Non-fatal: badges simply stay stale until the next refresh.
				update((s) => ({ ...s, loading: false }))
			} finally {
				inFlight = null
			}
		})()
		return inFlight
	}

	return {
		subscribe,

		/** Full load: accounts + items + ownership. Call on boot and on `collection-changed`. */
		async load() {
			update((s) => ({ ...s, loading: true }))
			await refreshData()
		},

		/** Event-driven refresh (collection-changed, cloud-sync merge, discovery reload). */
		async refresh() {
			await refreshData()
		},

		/** Link a fan-page URL. Returns null on failure (with a toast) so dialogs stay open. */
		async linkFromUrl(url: string): Promise<CollectionAccount | null> {
			update((s) => ({ ...s, linking: true }))
			try {
				const account = await collectionApi.linkCollectionAccount(url)
				await refreshData()
				return account
			} catch (error) {
				toastStore.error(errMsg(error, 'Failed to link collection account'))
				return null
			} finally {
				update((s) => ({ ...s, linking: false }))
			}
		},

		async unlink(id: string) {
			try {
				await collectionApi.unlinkCollectionAccount(id)
				await refreshData()
			} catch (error) {
				toastStore.error(errMsg(error, 'Failed to unlink account'))
			}
		},

		async setEnabled(id: string, enabled: boolean) {
			try {
				await collectionApi.setCollectionAccountEnabled(id, enabled)
				await refreshData()
			} catch (error) {
				toastStore.error(errMsg(error, 'Failed to update account'))
			}
		},

		async refreshAccount(id: string) {
			update((s) => ({ ...s, refreshingIds: new Set([...s.refreshingIds, id]) }))
			try {
				const result = await collectionApi.refreshCollectionAccount(id)
				if (result.error) toastStore.error(result.error)
				await refreshData()
			} catch (error) {
				toastStore.error(errMsg(error, 'Failed to refresh collection'))
			} finally {
				update((s) => {
					const next = new Set(s.refreshingIds)
					next.delete(id)
					return { ...s, refreshingIds: next }
				})
			}
		},

		async refreshAllAccounts(): Promise<CollectionRefreshSummary | null> {
			update((s) => ({ ...s, refreshingAll: true }))
			try {
				const summary = await collectionApi.refreshAllCollectionAccounts()
				await refreshData()
				return summary
			} catch (error) {
				toastStore.error(errMsg(error, 'Failed to refresh collections'))
				return null
			} finally {
				update((s) => ({ ...s, refreshingAll: false }))
			}
		},

		reset() {
			set(initialState)
		},
	}
}

export const collectionStore = createCollectionStore()

// =============================================================================
// Derived Stores
// =============================================================================

export const collectionAccounts = derived(collectionStore, ($c) => $c.accounts)

export const collectionItems = derived(collectionStore, ($c) => $c.items)

export const hasLinkedCollection = derived(collectionStore, ($c) => $c.accounts.length > 0)

/** Releases fully owned (album purchased, or every track individually owned). */
export const fullyOwnedReleaseIds = derived(collectionStore, ($c) => $c.fullyOwned)

/** Releases with some but not all tracks owned. */
export const partiallyOwnedReleaseIds = derived(collectionStore, ($c) => $c.partiallyOwned)

/** Union of fully + partially owned — "show purchased" filters use this. */
export const ownedReleaseIds = derived(collectionStore, ($c) => new Set([...$c.fullyOwned, ...$c.partiallyOwned]))

/** Individually purchased tracks (album ownership is implied by the release). */
export const ownedTrackIds = derived(collectionStore, ($c) => $c.ownedTracks)
