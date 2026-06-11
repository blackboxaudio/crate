import { writable, derived } from 'svelte/store'
import type { FollowedReleasesFound, FollowedSource, FollowedSourceCreate } from '$shared/types'
import * as followApi from '$shared/api/follow'
import { toastStore } from '$shared/stores/toast'

export type FollowSort = 'newCount' | 'name' | 'recentlyReleased'

interface FollowState {
	sources: FollowedSource[]
	loading: boolean
	error: string | null
	search: string
	sort: FollowSort
	selectMode: boolean
	selectedIds: Set<string>
	checkingIds: Set<string>
	checkingAll: boolean
}

const initialState: FollowState = {
	sources: [],
	loading: false,
	error: null,
	search: '',
	sort: 'newCount',
	selectMode: false,
	selectedIds: new Set(),
	checkingIds: new Set(),
	checkingAll: false,
}

function errMsg(error: unknown, fallback: string): string {
	return typeof error === 'string' ? error : error instanceof Error ? error.message : fallback
}

function upsert(sources: FollowedSource[], source: FollowedSource): FollowedSource[] {
	const idx = sources.findIndex((x) => x.id === source.id)
	if (idx >= 0) {
		const next = [...sources]
		next[idx] = source
		return next
	}
	return [source, ...sources]
}

function createFollowStore() {
	const { subscribe, set, update } = writable<FollowState>(initialState)

	return {
		subscribe,

		async load() {
			update((s) => ({ ...s, loading: true, error: null }))
			try {
				const sources = await followApi.getFollowedSources()
				update((s) => ({ ...s, sources, loading: false }))
			} catch (error) {
				update((s) => ({ ...s, loading: false, error: errMsg(error, 'Failed to load follows') }))
			}
		},

		async followFromUrl(url: string): Promise<FollowedSource | null> {
			try {
				const source = await followApi.followSource(url)
				update((s) => ({ ...s, sources: upsert(s.sources, source) }))
				return source
			} catch (error) {
				toastStore.error(errMsg(error, 'Failed to follow source'))
				return null
			}
		},

		async followEntity(create: FollowedSourceCreate): Promise<FollowedSource | null> {
			try {
				const source = await followApi.followFromEntity(create)
				update((s) => ({ ...s, sources: upsert(s.sources, source) }))
				return source
			} catch (error) {
				toastStore.error(errMsg(error, 'Failed to follow'))
				return null
			}
		},

		async unfollow(id: string) {
			try {
				await followApi.unfollowSource(id)
				update((s) => ({ ...s, sources: s.sources.filter((x) => x.id !== id) }))
			} catch (error) {
				toastStore.error(errMsg(error, 'Failed to unfollow'))
			}
		},

		async unfollowMany(ids: string[]) {
			const idSet = new Set(ids)
			await Promise.all(ids.map((id) => followApi.unfollowSource(id).catch(() => {})))
			update((s) => ({
				...s,
				sources: s.sources.filter((x) => !idSet.has(x.id)),
				selectedIds: new Set(),
				selectMode: false,
			}))
		},

		async setEnabled(id: string, enabled: boolean) {
			try {
				const source = await followApi.setFollowEnabled(id, enabled)
				update((s) => ({ ...s, sources: s.sources.map((x) => (x.id === id ? source : x)) }))
			} catch (error) {
				toastStore.error(errMsg(error, 'Failed to update follow'))
			}
		},

		/** Correct a follow's artist-vs-label type in place (avoids unfollow + re-follow). */
		async setType(id: string, followType: 'artist' | 'label') {
			try {
				const source = await followApi.setFollowType(id, followType)
				update((s) => ({ ...s, sources: s.sources.map((x) => (x.id === id ? source : x)) }))
			} catch (error) {
				toastStore.error(errMsg(error, 'Failed to update follow'))
			}
		},

		async setEnabledMany(ids: string[], enabled: boolean) {
			await Promise.all(ids.map((id) => followApi.setFollowEnabled(id, enabled).catch(() => null)))
			await this.load()
			update((s) => ({ ...s, selectedIds: new Set(), selectMode: false }))
		},

		async check(id: string) {
			update((s) => ({ ...s, checkingIds: new Set([...s.checkingIds, id]) }))
			try {
				await followApi.checkFollowedSource(id)
				await this.load()
			} catch (error) {
				toastStore.error(errMsg(error, 'Failed to check source'))
			} finally {
				update((s) => {
					const next = new Set(s.checkingIds)
					next.delete(id)
					return { ...s, checkingIds: next }
				})
			}
		},

		/** Re-link a source to already-imported releases (bandaid: backfills source_page_url).
		 *  Returns the count linked; discovery rows refresh live via `discovery-release-updated`. */
		async relink(id: string): Promise<number> {
			update((s) => ({ ...s, checkingIds: new Set([...s.checkingIds, id]) }))
			try {
				const count = await followApi.relinkSource(id)
				await this.load()
				return count
			} catch (error) {
				toastStore.error(errMsg(error, 'Failed to re-link source'))
				return 0
			} finally {
				update((s) => {
					const next = new Set(s.checkingIds)
					next.delete(id)
					return { ...s, checkingIds: next }
				})
			}
		},

		async checkAll(): Promise<FollowedReleasesFound | null> {
			update((s) => ({ ...s, checkingAll: true }))
			try {
				const found = await followApi.checkAllFollowedSources()
				await this.load()
				return found
			} catch (error) {
				toastStore.error(errMsg(error, 'Failed to check sources'))
				return null
			} finally {
				update((s) => ({ ...s, checkingAll: false }))
			}
		},

		/** Bump local new-counts from a background check event without a full reload. */
		applyAggregate(found: FollowedReleasesFound) {
			update((s) => {
				const byId = new Map(found.bySource.map((r) => [r.sourceId, r.newCount]))
				return {
					...s,
					sources: s.sources.map((x) => (byId.has(x.id) ? { ...x, newCount: x.newCount + (byId.get(x.id) ?? 0) } : x)),
				}
			})
		},

		setSearch(search: string) {
			update((s) => ({ ...s, search }))
		},

		setSort(sort: FollowSort) {
			update((s) => ({ ...s, sort }))
		},

		toggleSelectMode() {
			update((s) => ({ ...s, selectMode: !s.selectMode, selectedIds: new Set() }))
		},

		toggleSelected(id: string) {
			update((s) => {
				const next = new Set(s.selectedIds)
				if (next.has(id)) next.delete(id)
				else next.add(id)
				return { ...s, selectedIds: next }
			})
		},

		clearSelection() {
			update((s) => ({ ...s, selectedIds: new Set(), selectMode: false }))
		},

		reset() {
			set(initialState)
		},
	}
}

export const followStore = createFollowStore()

// =============================================================================
// Derived Stores
// =============================================================================

export const followedSources = derived(followStore, ($f) => $f.sources)

/** Total "new" count across enabled sources — drives the toolbar Following badge. */
export const followNewCount = derived(followStore, ($f) =>
	$f.sources.filter((x) => x.enabled).reduce((n, x) => n + (x.newCount ?? 0), 0)
)

/** Keys of followed entities for idempotent popover/import toggles: `type:platform:name`. */
export const followedEntityKeys = derived(followStore, ($f) => {
	const keys = new Set<string>()
	for (const s of $f.sources) {
		if (s.name) keys.add(`${s.followType}:${s.sourceType}:${s.name.toLowerCase()}`)
	}
	return keys
})

/** The Following modal's filtered + sorted list (search matches name AND url/domain). */
export const sortedFollowedSources = derived(followStore, ($f) => {
	let sources = [...$f.sources]
	const q = $f.search.trim().toLowerCase()
	if (q) {
		sources = sources.filter((s) => s.name?.toLowerCase().includes(q) || s.url.toLowerCase().includes(q))
	}
	sources.sort((a, b) => {
		if ($f.sort === 'name') return (a.name ?? '').localeCompare(b.name ?? '')
		if ($f.sort === 'recentlyReleased') return (b.lastReleaseAt ?? '').localeCompare(a.lastReleaseAt ?? '')
		return (b.newCount ?? 0) - (a.newCount ?? 0) || (a.name ?? '').localeCompare(b.name ?? '')
	})
	return sources
})
