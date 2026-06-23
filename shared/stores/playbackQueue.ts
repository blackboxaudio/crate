import { writable, type Readable } from 'svelte/store'
import type { DiscoveryRelease, QueueItem, UpNextEntry } from '../types'
import * as discoveryApi from '../api/discovery'
import { getStoredString, setStoredString } from '../utils/storage'

/**
 * Two-tier discovery-preview playback queue.
 *
 * This module owns "what plays next / previous / what's up next" so `player.ts` can stay focused on
 * the audio engines. It is platform-agnostic (no desktop/mobile imports) and depends only on shared
 * code; `player.ts` drives playback by asking it for the next/previous pick and feeding the iOS native
 * window from `peekUpcoming`.
 *
 * Two tiers, Spotify/Apple-Music style:
 *  - **Context queue** — the list playback started from (the discovery feed). Shuffle reorders THIS.
 *    Captured per session via `startSession`. Mirrors the implicit queue the "shuffle fix" introduced.
 *  - **User queue** — explicit items added with `addToQueue` (append) / `playNext` (front-insert).
 *    FIFO, never reshuffled, always plays before the context resumes. Persisted across relaunch.
 *
 * "Up Next" = the forward replay tail (if the user stepped back) ++ the user queue ++ the upcoming
 * context. A unified play `history` powers "previous" across BOTH tiers and both shuffle/sequential
 * modes; the no-repeat shuffle bag (`shufflePlayed`) only governs how NEW context picks are drawn.
 *
 * Desktop runs its own queue in `useAppSetup.ts` and never calls into here, so this stays inert there.
 */

// One playable unit. `release` is held by reference (live object from the context queue / a queued
// item), so resolution is synchronous; only ids are persisted (see `persistUserQueue`).
interface Pick {
	release: DiscoveryRelease
	trackIndex: number
}

// One explicit user-queue entry. `entryId` is stable per occurrence (the same release+track can be
// queued twice), so reorder/remove can target exactly one row.
interface UserEntry {
	entryId: string
	release: DiscoveryRelease
	trackIndex: number
}

const USER_QUEUE_KEY = 'player.userQueue'
// How many upcoming CONTEXT items the Up Next surface previews beyond the user queue. The whole user
// queue is always shown; this only bounds the (potentially looping) context forecast.
const DISPLAY_CONTEXT_DEPTH = 20
// Bounds the play history so a very long session can't grow it without limit; "previous" past this
// far back simply restarts (you can't step further than the retained history).
const HISTORY_CAP = 300

// --- State (closure singletons; one preview session at a time, like player.ts) ---------------------
let contextQueue: DiscoveryRelease[] = []
let userQueue: UserEntry[] = []
// Committed upcoming CONTEXT picks beyond the current track, drawn lazily. Committing the order here
// (rather than re-deriving per call) is what makes the Up Next forecast match what actually plays —
// essential for shuffle, where each pick is random.
let contextLookahead: Pick[] = []
// Everything that has actually played, in order, with a cursor at the current track. Drives "previous"
// (step back) and forward-replay (step forward after stepping back), uniformly across both tiers/modes.
let history: Pick[] = []
let historyPos = -1
// No-repeat-until-exhausted bag for drawing NEW shuffle picks (keys are `releaseId:trackIndex`).
let shufflePlayed = new Set<string>()
let shuffleEnabled = false
let cur: Pick | null = null

// Inverted dependency: player.ts registers this so a queue mutation can re-feed the iOS native window
// (the module must not import the engine). No-op on the HTML5 path / when nothing is playing.
let onQueueChanged: (() => void) | null = null

// --- Reactive surfaces (UI) ------------------------------------------------------------------------
const upNextStore = writable<UpNextEntry[]>([])
const userQueueCountStore = writable(0)
const canAdvanceStore = writable(false)

export const upNext: Readable<UpNextEntry[]> = { subscribe: upNextStore.subscribe }
export const userQueueCount: Readable<number> = { subscribe: userQueueCountStore.subscribe }
/** Whether a "next" exists (user queue, a forward-replay step, or more context). Drives transport. */
export const canAdvance: Readable<boolean> = { subscribe: canAdvanceStore.subscribe }

// --- Small helpers ---------------------------------------------------------------------------------
function trackKey(releaseId: string, trackIndex: number): string {
	return `${releaseId}:${trackIndex}`
}

let entrySeq = 0
function genEntryId(): string {
	// Avoid crypto.randomUUID (needs a secure context, which Tauri's custom scheme may not be). A
	// monotonic counter + timestamp + randomness is plenty for a stable, unique per-entry key.
	entrySeq += 1
	return `q${Date.now().toString(36)}-${entrySeq}-${Math.random().toString(36).slice(2, 8)}`
}

// Every (release, trackIndex) in the context queue whose key isn't in `exclude` — the bag shuffle draws.
function buildContextPool(exclude: Set<string>): Pick[] {
	const pool: Pick[] = []
	for (const release of contextQueue) {
		for (let i = 0; i < release.tracks.length; i++) {
			if (!exclude.has(trackKey(release.id, i))) pool.push({ release, trackIndex: i })
		}
	}
	return pool
}

// Next sequential pick after `from` WITHOUT wrapping: the next track in its release, else the first
// track of a later release in the queue, else null (end of this pass — the caller wraps on real advance).
function sequentialAfterNoWrap(from: Pick): Pick | null {
	if (from.trackIndex + 1 < from.release.tracks.length) {
		return { release: from.release, trackIndex: from.trackIndex + 1 }
	}
	const idx = contextQueue.findIndex((r) => r.id === from.release.id)
	if (idx === -1) return null
	for (let i = idx + 1; i < contextQueue.length; i++) {
		if (contextQueue[i].tracks.length > 0) return { release: contextQueue[i], trackIndex: 0 }
	}
	return null
}

// First track of the next release after `releaseId`, wrapping to the start of the queue. Used when a
// sequential pass reaches the end and loops (matches the pre-queue continuous-playback behaviour).
function nextReleaseStartWrap(releaseId: string): Pick | null {
	if (contextQueue.length === 0) return null
	const idx = contextQueue.findIndex((r) => r.id === releaseId)
	if (idx === -1) return null
	for (let i = 1; i <= contextQueue.length; i++) {
		const rel = contextQueue[(idx + i) % contextQueue.length]
		if (rel.tracks.length > 0) return { release: rel, trackIndex: 0 }
	}
	return null
}

// Last track of the previous release before `releaseId` (wraps). The sequential "previous" fallback at
// the very start of the play history — preserves the pre-queue cross-release "previous".
function prevReleaseEndWrap(releaseId: string): Pick | null {
	if (contextQueue.length === 0) return null
	const idx = contextQueue.findIndex((r) => r.id === releaseId)
	if (idx === -1) return null
	for (let i = 1; i <= contextQueue.length; i++) {
		const rel = contextQueue[(idx - i + contextQueue.length) % contextQueue.length]
		if (rel.tracks.length > 0) return { release: rel, trackIndex: rel.tracks.length - 1 }
	}
	return null
}

// Draw one fresh shuffle pick from the bag, marking it played/reserved so it can't repeat until the
// bag is exhausted. Returns null when nothing is left to draw this pass.
function drawShuffle(): Pick | null {
	const pool = buildContextPool(shufflePlayed)
	if (pool.length === 0) return null
	const choice = pool[Math.floor(Math.random() * pool.length)]
	shufflePlayed.add(trackKey(choice.release.id, choice.trackIndex))
	return choice
}

// Extend the committed context lookahead until it holds `targetLen` picks (or the pass is exhausted).
// Sequential walks forward from the tail; shuffle reserves fresh random draws.
function extendLookahead(targetLen: number) {
	let guard = 0
	while (contextLookahead.length < targetLen) {
		if (guard++ > 1000) break // safety against any unforeseen non-terminating draw
		let next: Pick | null
		if (shuffleEnabled) {
			next = drawShuffle()
		} else {
			const from = contextLookahead.length > 0 ? contextLookahead[contextLookahead.length - 1] : cur
			next = from ? sequentialAfterNoWrap(from) : null
		}
		if (!next) break
		contextLookahead.push(next)
	}
}

// Take the next context pick to actually PLAY. Prefers the committed lookahead; when this pass is
// exhausted, starts a new one (reshuffle / wrap) so playback loops like it did before the queue.
function takeContextNext(): Pick | null {
	extendLookahead(1)
	if (contextLookahead.length > 0) return contextLookahead.shift() ?? null
	if (!cur || contextQueue.length === 0) return null
	if (shuffleEnabled) {
		// Re-anchor the bag on the current track and draw; `drawShuffle` excludes it, so a lone single-track
		// context yields null (no infinite self-repeat) rather than redrawing the current track.
		shufflePlayed = new Set([trackKey(cur.release.id, cur.trackIndex)])
		return drawShuffle()
	}
	const wrapped = nextReleaseStartWrap(cur.release.id)
	// Don't hand back the current track as its own "next" — a single-track context would loop forever.
	if (wrapped && wrapped.release.id === cur.release.id && wrapped.trackIndex === cur.trackIndex) return null
	return wrapped
}

function pushHistory(pick: Pick) {
	history.push(pick)
	if (history.length > HISTORY_CAP) history.splice(0, history.length - HISTORY_CAP)
	historyPos = history.length - 1
}

// The upcoming sequence (without consuming): forward-replay tail (if stepped back) ++ user queue ++
// committed context. Shared by the Up Next forecast and the iOS native window so both match playback.
function upcomingPicks(depth: number): Array<{ pick: Pick; source: 'user' | 'context'; entryId?: string }> {
	const out: Array<{ pick: Pick; source: 'user' | 'context'; entryId?: string }> = []
	// 1) Forward-replay tail: when the user has stepped back, "next" replays history before anything new.
	for (let i = historyPos + 1; i < history.length && out.length < depth; i++) {
		out.push({ pick: history[i], source: 'context' })
	}
	// 2) Explicit user queue, in order.
	for (const e of userQueue) {
		if (out.length >= depth) break
		out.push({ pick: { release: e.release, trackIndex: e.trackIndex }, source: 'user', entryId: e.entryId })
	}
	// 3) Context continuation.
	if (out.length < depth) {
		extendLookahead(depth - out.length)
		for (let i = 0; i < contextLookahead.length && out.length < depth; i++) {
			out.push({ pick: contextLookahead[i], source: 'context' })
		}
	}
	return out
}

function computeCanAdvance(): boolean {
	if (historyPos < history.length - 1) return true // can replay forward
	if (userQueue.length > 0) return true
	if (contextQueue.length === 0) return false
	const totalTracks = contextQueue.reduce((n, r) => n + r.tracks.length, 0)
	// Sequential wraps and shuffle reshuffles, so any second track anywhere means there's always a next
	// (matches the pre-queue `canNext`). A lone single-track context has none.
	return totalTracks > 1
}

function refresh() {
	const replayCount = Math.max(0, history.length - 1 - historyPos)
	const depth = replayCount + userQueue.length + DISPLAY_CONTEXT_DEPTH
	const entries = upcomingPicks(depth).map(
		(u, i): UpNextEntry => ({
			key: u.entryId ?? `${trackKey(u.pick.release.id, u.pick.trackIndex)}:${i}`,
			source: u.source,
			release: u.pick.release,
			trackIndex: u.pick.trackIndex,
		})
	)
	upNextStore.set(entries)
	userQueueCountStore.set(userQueue.length)
	canAdvanceStore.set(computeCanAdvance())
}

// --- Persistence (explicit user queue only — ids, re-hydrated on launch) ---------------------------
function persistUserQueue() {
	const items: QueueItem[] = userQueue.map((e) => ({
		entryId: e.entryId,
		payload: { kind: 'preview', releaseId: e.release.id, trackIndex: e.trackIndex },
	}))
	setStoredString(USER_QUEUE_KEY, JSON.stringify(items))
}

// =============================================================================
// Public API
// =============================================================================

/** Register the handler player.ts uses to re-feed the iOS native window after a queue mutation. */
export function setQueueChangedHandler(handler: (() => void) | null) {
	onQueueChanged = handler
}

/** Mirror the player's shuffle flag at init (no side effects beyond anchoring an active session). */
export function initShuffle(enabled: boolean) {
	shuffleEnabled = enabled
}

/** The currently-playing pick, or null. Player keeps this in sync by routing every transition here. */
export function currentPick(): Pick | null {
	return cur
}

/**
 * Begin a session from a user-initiated play: capture the context list, anchor the current track and
 * a fresh play history, and reset the shuffle bag/lookahead. The user queue is intentionally KEPT —
 * explicitly queued items survive starting a new track.
 */
export function startSession(release: DiscoveryRelease, trackIndex: number, contextReleases: DiscoveryRelease[]) {
	contextQueue = contextReleases
	cur = { release, trackIndex }
	history = [cur]
	historyPos = 0
	contextLookahead = []
	shufflePlayed = shuffleEnabled ? new Set([trackKey(release.id, trackIndex)]) : new Set()
	refresh()
}

/** Toggle shuffle: re-anchor + redraw the CONTEXT order only. User queue and history are untouched. */
export function setShuffle(enabled: boolean) {
	if (shuffleEnabled === enabled) return
	shuffleEnabled = enabled
	contextLookahead = []
	shufflePlayed = enabled && cur ? new Set([trackKey(cur.release.id, cur.trackIndex)]) : new Set()
	refresh()
	onQueueChanged?.()
}

/** Append a track to the end of the explicit user queue. */
export function addToQueue(release: DiscoveryRelease, trackIndex: number) {
	userQueue.push({ entryId: genEntryId(), release, trackIndex })
	persistUserQueue()
	refresh()
	onQueueChanged?.()
}

/** Front-insert a track so it plays immediately after the current one (ahead of the rest of the queue). */
export function playNext(release: DiscoveryRelease, trackIndex: number) {
	userQueue.unshift({ entryId: genEntryId(), release, trackIndex })
	persistUserQueue()
	refresh()
	onQueueChanged?.()
}

/** Remove one user-queue entry by its stable id. */
export function removeEntry(entryId: string) {
	const i = userQueue.findIndex((e) => e.entryId === entryId)
	if (i === -1) return
	userQueue.splice(i, 1)
	persistUserQueue()
	refresh()
	onQueueChanged?.()
}

/** Move a user-queue entry to a new index (clamped). */
export function moveEntry(entryId: string, toIndex: number) {
	const from = userQueue.findIndex((e) => e.entryId === entryId)
	if (from === -1) return
	const to = Math.max(0, Math.min(userQueue.length - 1, toIndex))
	if (from === to) return
	const [entry] = userQueue.splice(from, 1)
	userQueue.splice(to, 0, entry)
	persistUserQueue()
	refresh()
	onQueueChanged?.()
}

/** Clear the explicit user queue (the context queue / current track keep playing). */
export function clearUserQueue() {
	if (userQueue.length === 0) return
	userQueue = []
	persistUserQueue()
	refresh()
	onQueueChanged?.()
}

/**
 * Advance to the next pick (the single source of truth for "next", used by every platform). Replays
 * forward through history when the user stepped back; otherwise consumes the user queue, then context.
 * Returns null when nothing more can play. Pops the consumed user item (and persists).
 */
export function advanceNext(): Pick | null {
	if (historyPos < history.length - 1) {
		historyPos++
		cur = history[historyPos]
		refresh()
		return cur
	}
	let pick: Pick | null
	if (userQueue.length > 0) {
		const entry = userQueue.shift()!
		pick = { release: entry.release, trackIndex: entry.trackIndex }
		persistUserQueue()
	} else {
		pick = takeContextNext()
	}
	if (!pick) {
		refresh()
		return null
	}
	pushHistory(pick)
	cur = pick
	refresh()
	return pick
}

/**
 * Step back to the previous pick. Walks the play history; at the very start, falls back (sequential
 * only) to the context's deterministic "previous" — within-release, then the previous release's last
 * track — prepending it so the cursor stays consistent. Shuffle restarts (returns null) at the start.
 */
export function advancePrev(): Pick | null {
	if (historyPos > 0) {
		historyPos--
		cur = history[historyPos]
		refresh()
		return cur
	}
	if (shuffleEnabled || !cur) return null
	const prev =
		cur.trackIndex > 0 ? { release: cur.release, trackIndex: cur.trackIndex - 1 } : prevReleaseEndWrap(cur.release.id)
	if (!prev) return null
	history.unshift(prev)
	historyPos = 0
	cur = prev
	refresh()
	return prev
}

/** The next `depth` upcoming picks without consuming — used to build the iOS native sliding window. */
export function peekUpcoming(depth: number): Pick[] {
	return upcomingPicks(Math.max(0, depth)).map((u) => u.pick)
}

/** Whether at least one explicit user-queue item is pending (affects the native-feed decision). */
export function hasUserQueueAhead(): boolean {
	return userQueue.length > 0
}

/** Current explicit-queue length (sync) — player.ts uses it to size the native window. */
export function userQueueLength(): number {
	return userQueue.length
}

/** Clear everything (stop / reset). The persisted user queue is cleared too. */
export function clearAll() {
	contextQueue = []
	userQueue = []
	contextLookahead = []
	history = []
	historyPos = -1
	shufflePlayed = new Set()
	cur = null
	persistUserQueue()
	refresh()
}

/**
 * Re-hydrate the persisted user queue after a relaunch: re-fetch each release by id (like
 * restorePreview), dropping entries whose release no longer exists or whose track index is now out of
 * range. Safe to call on the non-critical boot path; triggers a native re-feed if something is playing.
 */
export async function hydrate(): Promise<void> {
	const raw = getStoredString(USER_QUEUE_KEY, '')
	if (!raw) return
	let items: QueueItem[]
	try {
		const parsed = JSON.parse(raw)
		if (!Array.isArray(parsed)) return
		items = parsed
	} catch {
		return
	}
	const cache = new Map<string, DiscoveryRelease | null>()
	const resolved: UserEntry[] = []
	for (const item of items) {
		if (!item?.payload || item.payload.kind !== 'preview') continue
		const { releaseId, trackIndex } = item.payload
		let release = cache.get(releaseId)
		if (release === undefined) {
			try {
				release = await discoveryApi.getRelease(releaseId)
			} catch {
				release = null
			}
			cache.set(releaseId, release)
		}
		if (!release) continue
		if (trackIndex < 0 || trackIndex >= release.tracks.length) continue
		resolved.push({ entryId: item.entryId || genEntryId(), release, trackIndex })
	}
	userQueue = resolved
	persistUserQueue()
	refresh()
	onQueueChanged?.()
}
