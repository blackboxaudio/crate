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
 * The repeat mode (see `RepeatMode`) scopes how the context continues once a pass ends — the committed
 * lookahead applies it in ONE place (`extendLookahead`) so playback, Up Next, the native window, and
 * `canAdvance` always agree.
 *
 * Desktop runs its own queue in `useAppSetup.ts` and never calls into here, so this stays inert there.
 */

// One playable unit. `release` is held by reference (live object from the context queue / a queued
// item), so resolution is synchronous; only ids are persisted (see `persistUserQueue`).
export interface Pick {
	release: DiscoveryRelease
	trackIndex: number
}

/**
 * Repeat cycle: off → track → release → context.
 *  - `off` — advance stops at the end of the context (no wrap, no reshuffle).
 *  - `track` — the current track loops on natural end. A player-level concern: the queue's advance
 *    semantics treat it as `context` (a manual skip proceeds, then the NEW track loops — Spotify's
 *    repeat-one), see `effectiveScope`.
 *  - `release` — advance loops within the CURRENT release only (re-anchors to whatever is playing).
 *  - `context` — advance wraps over the whole context / reshuffles each pass (the pre-repeat behaviour).
 */
export type RepeatMode = 'off' | 'track' | 'release' | 'context'

// One explicit user-queue entry. `entryId` is stable per occurrence (the same release+track can be
// queued twice), so reorder/remove can target exactly one row.
interface UserEntry {
	entryId: string
	release: DiscoveryRelease
	trackIndex: number
}

const USER_QUEUE_KEY = 'player.userQueue'
const RECENT_KEY = 'player.recentlyPlayed'
// Bounds the persistent listening log ("Recently played" in the queue sheet).
const RECENT_CAP = 50
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
let repeatMode: RepeatMode = 'off'
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

// --- Recently played (persistent listening log) ------------------------------------------------------
// Distinct from `history`, which is per-session prev/next machinery and resets on every startSession.
// Ids only — the UI resolves them against the loaded discovery set and drops what no longer exists.

/** One listening-log entry. Newest LAST internally; the UI renders the list reversed. */
export interface RecentPlay {
	releaseId: string
	trackIndex: number
	at: number
}

function readRecent(): RecentPlay[] {
	const raw = getStoredString(RECENT_KEY, '')
	if (!raw) return []
	try {
		const parsed = JSON.parse(raw)
		if (!Array.isArray(parsed)) return []
		return parsed.filter(
			(e): e is RecentPlay =>
				typeof e?.releaseId === 'string' && typeof e?.trackIndex === 'number' && typeof e?.at === 'number'
		)
	} catch {
		return []
	}
}

let recent: RecentPlay[] = readRecent()
const recentlyPlayedStore = writable<RecentPlay[]>([...recent])
export const recentlyPlayed: Readable<RecentPlay[]> = { subscribe: recentlyPlayedStore.subscribe }

function logRecent(pick: Pick) {
	const last = recent[recent.length - 1]
	// Dedupe consecutive repeats (re-taps, single-track loops) so the log reads as a timeline.
	if (last && last.releaseId === pick.release.id && last.trackIndex === pick.trackIndex) return
	recent.push({ releaseId: pick.release.id, trackIndex: pick.trackIndex, at: Date.now() })
	if (recent.length > RECENT_CAP) recent.splice(0, recent.length - RECENT_CAP)
	setStoredString(RECENT_KEY, JSON.stringify(recent))
	recentlyPlayedStore.set([...recent])
}

/** Wipe the persistent listening log. The session history / queues are untouched — and vice versa:
 *  stopping playback (`clearAll`) deliberately does NOT forget what was listened to. */
export function clearRecentlyPlayed() {
	if (recent.length === 0) return
	recent = []
	setStoredString(RECENT_KEY, JSON.stringify(recent))
	recentlyPlayedStore.set([])
}

// --- Small helpers ---------------------------------------------------------------------------------
function trackKey(releaseId: string, trackIndex: number): string {
	return `${releaseId}:${trackIndex}`
}

// Shallow content equality for two release lists (same ids, same order). Lets `updateContext` ignore the
// feed's derived re-emitting an identical list (it recomputes on unrelated mobile-UI-store changes).
function sameReleaseList(a: DiscoveryRelease[], b: DiscoveryRelease[]): boolean {
	if (a === b) return true
	if (a.length !== b.length) return false
	for (let i = 0; i < a.length; i++) {
		if (a[i].id !== b[i].id) return false
	}
	return true
}

let entrySeq = 0
function genEntryId(): string {
	// Avoid crypto.randomUUID (needs a secure context, which Tauri's custom scheme may not be). A
	// monotonic counter + timestamp + randomness is plenty for a stable, unique per-entry key.
	entrySeq += 1
	return `q${Date.now().toString(36)}-${entrySeq}-${Math.random().toString(36).slice(2, 8)}`
}

// A track the source serves no preview for (a pre-order's unreleased tracks) is invisible to the
// queue: never advanced into, never shuffled, never enqueued — it would only fail to resolve.
function isPlayable(release: DiscoveryRelease, trackIndex: number): boolean {
	const track = release.tracks[trackIndex]
	// Mirrors desktop's `trackCanPlay` gate: no duration means the source never exposed the track as
	// playable (unreleased pre-order tracks, unenriched rows), and Discogs plays via YouTube video only.
	if (!track?.duration_ms) return false
	if (track.preview_unavailable) return false
	if (release.source_type === 'discogs') return track.video_id != null
	return true
}

// First playable track index at or after `from`, or -1 when the release has none left.
function firstPlayableIndex(release: DiscoveryRelease, from = 0): number {
	for (let i = from; i < release.tracks.length; i++) if (isPlayable(release, i)) return i
	return -1
}

function lastPlayableIndex(release: DiscoveryRelease): number {
	for (let i = release.tracks.length - 1; i >= 0; i--) if (isPlayable(release, i)) return i
	return -1
}

// Last playable track index at or before `from`, or -1 (the backward mirror of `firstPlayableIndex`).
function prevPlayableIndex(release: DiscoveryRelease, from: number): number {
	for (let i = Math.min(from, release.tracks.length - 1); i >= 0; i--) if (isPlayable(release, i)) return i
	return -1
}

// The scope advance/lookahead draws from. Repeat-track loops at the PLAYER level (natural end restarts
// the track before the queue is ever consulted); for everything the queue answers — manual next/prev,
// canAdvance, Up Next, the iOS native window tail — it behaves as repeat-context.
function effectiveScope(): 'off' | 'release' | 'context' {
	return repeatMode === 'track' ? 'context' : repeatMode
}

// Every playable (release, trackIndex) whose key isn't in `exclude` — the bag shuffle draws. Scoped to
// `onlyRelease` for repeat-release (which follows `cur` by reference, so it works even when the looped
// release isn't in the context queue — e.g. a user-queued release from outside the feed).
function buildContextPool(exclude: Set<string>, onlyRelease?: DiscoveryRelease): Pick[] {
	const pool: Pick[] = []
	for (const release of onlyRelease ? [onlyRelease] : contextQueue) {
		for (let i = 0; i < release.tracks.length; i++) {
			if (isPlayable(release, i) && !exclude.has(trackKey(release.id, i))) pool.push({ release, trackIndex: i })
		}
	}
	return pool
}

// Next sequential pick after `from` WITHOUT wrapping: the next playable track in its release, else the
// first playable track of a later release in the queue, else null (end of this pass — `extendLookahead`
// wraps under repeat-context, stops under repeat-off).
function sequentialAfterNoWrap(from: Pick): Pick | null {
	const next = firstPlayableIndex(from.release, from.trackIndex + 1)
	if (next !== -1) return { release: from.release, trackIndex: next }
	const idx = contextQueue.findIndex((r) => r.id === from.release.id)
	if (idx === -1) return null
	for (let i = idx + 1; i < contextQueue.length; i++) {
		const first = firstPlayableIndex(contextQueue[i])
		if (first !== -1) return { release: contextQueue[i], trackIndex: first }
	}
	return null
}

// Next sequential pick WITHIN `from`'s release only, wrapping to its first playable track at the end.
// Repeat-release's walk. A single-playable-track release hands back that same track (the degenerate
// release loop IS a track loop); a release whose tracks all went `preview_unavailable` yields null.
function sequentialAfterInRelease(from: Pick): Pick | null {
	const next = firstPlayableIndex(from.release, from.trackIndex + 1)
	if (next !== -1) return { release: from.release, trackIndex: next }
	const first = firstPlayableIndex(from.release)
	if (first === -1) return null
	return { release: from.release, trackIndex: first }
}

// First playable track of the next release after `releaseId`, wrapping to the start of the queue. Used
// when a repeat-context sequential pass reaches the end and loops.
function nextReleaseStartWrap(releaseId: string): Pick | null {
	if (contextQueue.length === 0) return null
	const idx = contextQueue.findIndex((r) => r.id === releaseId)
	if (idx === -1) return null
	for (let i = 1; i <= contextQueue.length; i++) {
		const rel = contextQueue[(idx + i) % contextQueue.length]
		const first = firstPlayableIndex(rel)
		if (first !== -1) return { release: rel, trackIndex: first }
	}
	return null
}

// Last playable track of the previous release before `releaseId` (wraps). The repeat-context "previous"
// fallback at the very start of the play history — preserves the pre-repeat cross-release "previous".
function prevReleaseEndWrap(releaseId: string): Pick | null {
	if (contextQueue.length === 0) return null
	const idx = contextQueue.findIndex((r) => r.id === releaseId)
	if (idx === -1) return null
	for (let i = 1; i <= contextQueue.length; i++) {
		const rel = contextQueue[(idx - i + contextQueue.length) % contextQueue.length]
		const last = lastPlayableIndex(rel)
		if (last !== -1) return { release: rel, trackIndex: last }
	}
	return null
}

// Last playable track of an EARLIER release, without wrapping past the start of the queue — repeat-off's
// "previous" fallback (off removes only the wrap-around; earlier releases stay reachable).
function prevReleaseEndNoWrap(releaseId: string): Pick | null {
	const idx = contextQueue.findIndex((r) => r.id === releaseId)
	if (idx === -1) return null
	for (let i = idx - 1; i >= 0; i--) {
		const last = lastPlayableIndex(contextQueue[i])
		if (last !== -1) return { release: contextQueue[i], trackIndex: last }
	}
	return null
}

// Draw one fresh shuffle pick from the bag, marking it played/reserved so it can't repeat until the
// bag is exhausted. Returns null when nothing is left to draw this pass. In release scope the pool is
// the current release only.
function drawShuffle(scope: 'off' | 'release' | 'context'): Pick | null {
	const pool = buildContextPool(shufflePlayed, scope === 'release' ? cur?.release : undefined)
	if (pool.length === 0) return null
	const choice = pool[Math.floor(Math.random() * pool.length)]
	shufflePlayed.add(trackKey(choice.release.id, choice.trackIndex))
	return choice
}

/**
 * Extend the committed context lookahead until it holds `targetLen` picks (or nothing more can play).
 * Sequential walks forward from the tail; shuffle reserves fresh random draws. This is the ONE place
 * repeat scope shapes what comes next — wrapping/reshuffling happens here so actual playback, the Up
 * Next forecast, the iOS native window, and `canAdvance` all agree:
 *  - off      → stop at the end of the pass (no wrap, no reshuffle).
 *  - release  → loop within `cur`'s release; shuffle draws from that release only.
 *  - context  → wrap to the next release / re-seed the bag each pass (also repeat-track's skip scope).
 */
function extendLookahead(targetLen: number) {
	const scope = effectiveScope()
	// Release scope follows whatever release is CURRENT (it re-anchors after e.g. a user-queued track
	// from another release played). Committed picks from a previous release are stale — drop them all.
	if (scope === 'release' && cur) {
		const anchorId = cur.release.id
		if (contextLookahead.some((p) => p.release.id !== anchorId)) contextLookahead = []
	}
	let guard = 0
	while (contextLookahead.length < targetLen) {
		if (guard++ > 1000) break // safety against any unforeseen non-terminating draw
		const tail = contextLookahead.length > 0 ? contextLookahead[contextLookahead.length - 1] : cur
		if (!tail) break
		let next: Pick | null
		if (shuffleEnabled) {
			next = drawShuffle(scope)
			if (!next && scope !== 'off') {
				// Pass exhausted while repeating: re-seed the bag anchored on the last committed pick (NOT
				// `cur` — the lookahead may already sit a pass ahead) and start the next pass.
				shufflePlayed = new Set([trackKey(tail.release.id, tail.trackIndex)])
				next = drawShuffle(scope)
				// A release with a single playable track has an empty pool even after re-seeding (the anchor
				// is excluded) — the degenerate release loop is a self-loop, matching the sequential walk.
				if (!next && scope === 'release' && isPlayable(tail.release, tail.trackIndex)) {
					next = { release: tail.release, trackIndex: tail.trackIndex }
				}
			}
		} else if (scope === 'release') {
			next = sequentialAfterInRelease(tail)
		} else {
			next = sequentialAfterNoWrap(tail)
			if (!next && scope === 'context') {
				const wrapped = nextReleaseStartWrap(tail.release.id)
				// Don't hand back the tail as its own "next" — a lone single-track context would loop onto
				// itself forever (repeat-track is the mode for that).
				next =
					wrapped && wrapped.release.id === tail.release.id && wrapped.trackIndex === tail.trackIndex ? null : wrapped
			}
		}
		if (!next) break
		contextLookahead.push(next)
	}
}

// Take the next context pick to actually PLAY. The committed lookahead is the single source of what
// comes next — `extendLookahead` already applied the repeat scope (including the off-mode stop).
function takeContextNext(): Pick | null {
	extendLookahead(1)
	return contextLookahead.shift() ?? null
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
	// The mode-aware lookahead already knows whether anything more can play (repeat wraps/reshuffles,
	// off stops, `preview_unavailable` tracks never count) — commit one pick and check.
	extendLookahead(1)
	return contextLookahead.length > 0
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

/** Mirror the player's persisted repeat mode at init (no side effects — nothing is playing yet). */
export function initRepeatMode(mode: RepeatMode) {
	repeatMode = mode
}

/**
 * Change the repeat mode: re-anchor + redraw the CONTEXT order only (the committed lookahead was built
 * under the old scope). User queue and history are untouched. Same coupled lookahead/bag reset as
 * `setShuffle` — lookahead draws reserve bag keys, so the two only ever reset together.
 */
export function setRepeatMode(mode: RepeatMode) {
	if (repeatMode === mode) return
	repeatMode = mode
	contextLookahead = []
	shufflePlayed = shuffleEnabled && cur ? new Set([trackKey(cur.release.id, cur.trackIndex)]) : new Set()
	refresh()
	onQueueChanged?.()
}

/** The currently-playing pick, or null. Player keeps this in sync by routing every transition here. */
export function currentPick(): Pick | null {
	return cur
}

/**
 * Begin a session from a user-initiated play: capture the context list, anchor the current track and
 * a fresh play history, and reset the shuffle bag/lookahead. The user queue is intentionally KEPT —
 * explicitly queued items survive starting a new track. `opts.logPlay: false` skips the listening log
 * (the relaunch restore re-anchors the last session's track without the user playing anything).
 */
export function startSession(
	release: DiscoveryRelease,
	trackIndex: number,
	contextReleases: DiscoveryRelease[],
	opts?: { logPlay?: boolean }
) {
	contextQueue = contextReleases
	cur = { release, trackIndex }
	history = [cur]
	historyPos = 0
	contextLookahead = []
	shufflePlayed = shuffleEnabled ? new Set([trackKey(release.id, trackIndex)]) : new Set()
	if (opts?.logPlay !== false) logRecent(cur)
	refresh()
}

/**
 * Swap the CONTEXT list of the active session in place, keeping the current track, play history, and user
 * queue — only what plays next/after is re-derived from the new list. Used when the view the session was
 * started from changes its on-screen set (e.g. the discovery feed's filter is applied/reset while a
 * feed-originated preview plays), so next/shuffle keep spanning exactly what's on screen. No-op shape when
 * nothing is playing (`cur` null): it just stores the list for the next `startSession`. A repeat-release
 * loop is unaffected by design — it follows `cur.release` by reference, not the context list.
 */
export function updateContext(contextReleases: DiscoveryRelease[]) {
	// No-op when the list is unchanged: the feed's derived re-emits on unrelated UI-store changes, and
	// resetting the lookahead / shuffle bag every time would disrupt shuffle (and churn the native window).
	if (sameReleaseList(contextQueue, contextReleases)) return
	contextQueue = contextReleases
	contextLookahead = []
	// Re-anchor the shuffle bag on the current track so fresh draws come from the new list (mirrors setShuffle).
	shufflePlayed = shuffleEnabled && cur ? new Set([trackKey(cur.release.id, cur.trackIndex)]) : new Set()
	refresh()
	onQueueChanged?.()
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

// Build a user-queue entry per track of a release, in track order — the whole-release equivalent of one
// `addToQueue`/`playNext` call. Each track gets its own stable entry id (it's an independent queue row).
function releaseEntries(release: DiscoveryRelease): UserEntry[] {
	return release.tracks
		.map((_, i) => ({ entryId: genEntryId(), release, trackIndex: i }))
		.filter((e) => isPlayable(release, e.trackIndex))
}

/** Append every track of a release to the end of the user queue, in track order. No-op if it has none. */
export function addReleaseToQueue(release: DiscoveryRelease) {
	const entries = releaseEntries(release)
	if (entries.length === 0) return
	userQueue.push(...entries)
	persistUserQueue()
	refresh()
	onQueueChanged?.()
}

/** Front-insert every track of a release so the whole release plays next, in track order (ahead of the
 *  rest of the queue). No-op if it has no tracks. */
export function playReleaseNext(release: DiscoveryRelease) {
	const entries = releaseEntries(release)
	if (entries.length === 0) return
	userQueue.unshift(...entries)
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
		logRecent(cur)
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
	logRecent(cur)
	refresh()
	return pick
}

// The deterministic sequential "previous" of `from` under the current repeat scope (the fallback once
// the play history is exhausted): the previous playable track in its release, then per scope — release:
// wrap to the release's last playable (null when `from` is its only one); off: an earlier release's last
// playable, never wrapping past the start; context/track: the previous release's last playable, wrapping.
function sequentialPrevOf(from: Pick): Pick | null {
	const within = prevPlayableIndex(from.release, from.trackIndex - 1)
	if (within !== -1) return { release: from.release, trackIndex: within }
	const scope = effectiveScope()
	if (scope === 'release') {
		const last = lastPlayableIndex(from.release)
		if (last === -1 || last === from.trackIndex) return null
		return { release: from.release, trackIndex: last }
	}
	if (scope === 'off') return prevReleaseEndNoWrap(from.release.id)
	return prevReleaseEndWrap(from.release.id)
}

/**
 * Step back to the previous pick. Walks the play history; at the very start, falls back (sequential
 * only) to the scope-aware deterministic "previous" (`sequentialPrevOf`), prepending it so the cursor
 * stays consistent. Shuffle restarts (returns null) at the start.
 */
export function advancePrev(): Pick | null {
	if (historyPos > 0) {
		historyPos--
		cur = history[historyPos]
		refresh()
		return cur
	}
	if (shuffleEnabled || !cur) return null
	const prev = sequentialPrevOf(cur)
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

/**
 * The pick `advancePrev` WOULD step to, without mutating anything — the non-consuming mirror of its
 * history-walk + sequential fallback. Used by the expanded player's swipe pager to render the incoming
 * previous cover during a drag and to rubber-band when there is no previous (shuffle at history start).
 */
export function peekPrevious(): Pick | null {
	if (historyPos > 0) return history[historyPos - 1]
	if (shuffleEnabled || !cur) return null
	return sequentialPrevOf(cur)
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
		if (!isPlayable(release, trackIndex)) continue
		resolved.push({ entryId: item.entryId || genEntryId(), release, trackIndex })
	}
	userQueue = resolved
	persistUserQueue()
	refresh()
	onQueueChanged?.()
}
