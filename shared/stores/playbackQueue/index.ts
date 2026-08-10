import { writable, type Readable } from 'svelte/store'
import type { DiscoveryRelease, QueueItem, Track, UpNextEntry } from '../../types'
import * as discoveryApi from '../../api/discovery'
import { getStoredString, setStoredString } from '../../utils/storage'
import {
	isPlayablePick,
	isPreviewPlayable,
	libraryGroupKey,
	pickGroupKey,
	pickKey,
	type Pick,
	type PreviewPick,
} from './playable'

export {
	isPreviewPlayable,
	firstPlayablePreviewIndex,
	setLibraryPlayableFilter,
	type LibraryPick,
	type Pick,
	type PreviewPick,
} from './playable'

/**
 * Two-tier playback queue — the single "what plays next / previous / what's up next" for BOTH
 * platforms and BOTH sources (discovery previews and library tracks), so `player.ts` can stay focused
 * on the audio engines. Platform-agnostic (no desktop/mobile imports); `player.ts` drives playback by
 * asking it for the next/previous pick and feeding the iOS native window from `peekUpcoming`; desktop
 * captures its library/discovery contexts into it via `startLibrarySession`/`startPreviewSession`.
 *
 * Two tiers, Spotify/Apple-Music style:
 *  - **Context queue** — the list playback started from (the discovery feed, the visible library
 *    view), held as a FLAT list of picks with a per-pick group key ("release" for previews, album for
 *    library tracks). Shuffle reorders THIS. Captured per session via `start*Session`.
 *  - **User queue** — explicit items added with `addToQueue` (append) / `playNext` (front-insert).
 *    FIFO, never reshuffled, always plays before the context resumes. Preview entries are persisted
 *    across relaunch.
 *
 * "Up Next" = the forward replay tail (if the user stepped back) ++ the user queue ++ the upcoming
 * context. A unified play `history` powers "previous" across BOTH tiers and both shuffle/sequential
 * modes; the no-repeat shuffle bag (`shufflePlayed`) only governs how NEW context picks are drawn.
 * The repeat mode (see `RepeatMode`) scopes how the context continues once a pass ends — the committed
 * lookahead applies it in ONE place (`extendLookahead`) so playback, Up Next, the native window, and
 * `canAdvance` always agree.
 */

/**
 * Repeat cycle: off → track → release → context.
 *  - `off` — advance stops at the end of the context (no wrap, no reshuffle).
 *  - `track` — the current track loops, literally: natural end restarts it at the player level, and
 *    the queue's own advance (Next button, lock screen, auto-advance fallbacks) replays it too, with
 *    Up Next forecasting it. NOT Spotify's repeat-one (skip-proceeds) — users read "next plays a
 *    different track" as repeat not working. The explicit user queue still wins a manual skip, so
 *    queued items stay reachable (and the loop then re-anchors on them).
 *  - `release` — advance loops within the CURRENT group only (the release for previews, the album for
 *    library tracks; re-anchors to the playing context-tier pick — a user-queued interlude plays once
 *    and hands back to the release it interrupted, see `contextSeed`).
 *  - `context` — advance wraps over the whole context / reshuffles each pass (the pre-repeat behaviour).
 */
export type RepeatMode = 'off' | 'track' | 'release' | 'context'

// One explicit user-queue entry. `entryId` is stable per occurrence (the same track can be queued
// twice), so reorder/remove can target exactly one row.
interface UserEntry {
	entryId: string
	pick: Pick
}

// One flat context slot: a pick plus its precomputed identity/group keys (playability is evaluated
// live — `preview_unavailable` flags mutate in place on stream extraction, without a context swap).
interface ContextItem {
	key: string
	groupKey: string
	pick: Pick
}

const USER_QUEUE_KEY = 'player.userQueue'
const RECENT_KEY = 'player.recentlyPlayed'
// Bounds the persistent listening log ("Recently played" in the queue sheet).
const RECENT_CAP = 50
// Bounds the play history so a very long session can't grow it without limit; "previous" past this
// far back simply restarts (you can't step further than the retained history).
const HISTORY_CAP = 300

// --- State (closure singletons; one session at a time, like player.ts) -----------------------------
let contextKind: 'preview' | 'library' = 'preview'
// The raw list the context was built from — cheap identity pre-check for update*Context.
let contextRaw: DiscoveryRelease[] | Track[] = []
let contextItems: ContextItem[] = []
let contextIndexByKey = new Map<string, number>()
let contextGroupKeys = new Set<string>()
let userQueue: UserEntry[] = []
// Committed upcoming CONTEXT picks beyond the current track, drawn lazily. Committing the order here
// (rather than re-deriving per call) is what makes the Up Next forecast match what actually plays —
// essential for shuffle, where each pick is random.
let contextLookahead: Pick[] = []
// Everything that has actually played, in order, with a cursor at the current track. Drives "previous"
// (step back) and forward-replay (step forward after stepping back), uniformly across both tiers/modes.
let history: Pick[] = []
// Which tier each history entry entered playback from, aligned with `history`. A 'user' entry is an
// interlude the explicit queue injected — the context walk must never anchor on it (see `contextSeed`).
let historySource: Array<'user' | 'context'> = []
let historyPos = -1
// No-repeat-until-exhausted bag for drawing NEW shuffle picks (keys are `pickKey`s).
let shufflePlayed = new Set<string>()
let shuffleEnabled = false
let repeatMode: RepeatMode = 'off'
let cur: Pick | null = null

// Inverted dependency: player.ts registers this so a queue mutation can re-feed the iOS native window
// (the module must not import the engine). No-op on the HTML5 path / when nothing is playing.
let onQueueChanged: (() => void) | null = null

// How many upcoming CONTEXT items the Up Next surface previews beyond the user queue. The whole user
// queue is always shown; this only bounds the (potentially looping) context forecast. Library sessions
// render no Up Next sheet, so their forecast depth is 0 (canAdvance commits its own single pick).
function displayContextDepth(): number {
	if (contextKind === 'library') return 0
	// Repeat-track forecasts the looping track ONCE — a 20-deep list of the same track reads as noise
	// (the native window sizes itself separately via `peekUpcoming`, unaffected by this).
	return repeatMode === 'track' ? 1 : 20
}

// --- Reactive surfaces (UI) ------------------------------------------------------------------------
const upNextStore = writable<UpNextEntry[]>([])
const userQueueCountStore = writable(0)
const canAdvanceStore = writable(false)

export const upNext: Readable<UpNextEntry[]> = { subscribe: upNextStore.subscribe }
export const userQueueCount: Readable<number> = { subscribe: userQueueCountStore.subscribe }
/** Whether a "next" exists (user queue, a forward-replay step, or more context). Drives transport. */
export const canAdvance: Readable<boolean> = { subscribe: canAdvanceStore.subscribe }

// --- Recently played (persistent listening log) ------------------------------------------------------
// Distinct from `history`, which is per-session prev/next machinery and resets on every session start.
// Ids only — the UI resolves them against the loaded discovery set and drops what no longer exists.
// Preview-only: the log's persisted shape and its sole consumer (UpNextSheet) are release-based.

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
	if (pick.kind !== 'preview') return
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

// --- Context construction --------------------------------------------------------------------------
function flattenPreview(releases: DiscoveryRelease[]): ContextItem[] {
	// Every track is flattened, playable or not: `preview_unavailable` is refreshed on stream
	// extraction, so keying the item list off playability would churn the context on every extraction.
	const items: ContextItem[] = []
	for (const release of releases) {
		for (let i = 0; i < release.tracks.length; i++) {
			items.push({
				key: `p:${release.id}:${i}`,
				groupKey: `r:${release.id}`,
				pick: { kind: 'preview', release, trackIndex: i },
			})
		}
	}
	return items
}

function flattenLibrary(tracks: Track[]): ContextItem[] {
	return tracks.map((t) => ({ key: `l:${t.id}`, groupKey: libraryGroupKey(t), pick: { kind: 'library', track: t } }))
}

function installContext(kind: 'preview' | 'library', raw: DiscoveryRelease[] | Track[], items: ContextItem[]) {
	contextKind = kind
	contextRaw = raw
	contextItems = items
	contextIndexByKey = new Map(items.map((it, i) => [it.key, i]))
	contextGroupKeys = new Set(items.map((it) => it.groupKey))
}

// Shallow content equality for two release lists (same ids, same order, same track counts). Lets
// `updatePreviewContext` ignore the feed's derived re-emitting an identical list (it recomputes on
// unrelated UI-store changes). Track count is part of the identity: a release that gained tracks
// after a metadata refresh must re-flatten.
function samePreviewList(a: DiscoveryRelease[], b: DiscoveryRelease[]): boolean {
	if (a === b) return true
	if (a.length !== b.length) return false
	for (let i = 0; i < a.length; i++) {
		if (a[i].id !== b[i].id || a[i].tracks.length !== b[i].tracks.length) return false
	}
	return true
}

function sameTrackList(a: Track[], b: Track[]): boolean {
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

// --- Flat index walks ------------------------------------------------------------------------------
// Group members need not be contiguous (a library list sorted by BPM scatters an album); every walk
// filters by group key when scoped, so "the release in order" means "its members in list order".

function indexOfPick(p: Pick): number {
	return contextIndexByKey.get(pickKey(p)) ?? -1
}

function nextPlayableFrom(i: number, group?: string): number {
	for (let j = i + 1; j < contextItems.length; j++) {
		const it = contextItems[j]
		if (group !== undefined && it.groupKey !== group) continue
		if (isPlayablePick(it.pick)) return j
	}
	return -1
}

function prevPlayableFrom(i: number, group?: string): number {
	for (let j = Math.min(i, contextItems.length) - 1; j >= 0; j--) {
		const it = contextItems[j]
		if (group !== undefined && it.groupKey !== group) continue
		if (isPlayablePick(it.pick)) return j
	}
	return -1
}

function firstPlayable(group?: string): number {
	return nextPlayableFrom(-1, group)
}

function lastPlayable(group?: string): number {
	return prevPlayableFrom(contextItems.length, group)
}

// --- Foreign-pick helpers --------------------------------------------------------------------------
// A "foreign" pick is one the context list doesn't hold — typically a restored session (or a
// still-playing track) whose context was since re-scoped to a list without it. A user-queued track
// from another view is NOT walked as foreign: the context walk seeds past interludes entirely
// (`contextSeed`), except as the last-resort fallback when nothing context-tier ever played. A
// preview pick carries its self-contained release, so its own release keeps walking/looping BY
// REFERENCE; a library pick has no standalone group, so its group operations degenerate (release
// loop → track loop).

function localPreviewNext(p: PreviewPick): PreviewPick | null {
	for (let i = p.trackIndex + 1; i < p.release.tracks.length; i++) {
		if (isPreviewPlayable(p.release, i)) return { kind: 'preview', release: p.release, trackIndex: i }
	}
	return null
}

function localPreviewPrev(p: PreviewPick): PreviewPick | null {
	for (let i = Math.min(p.trackIndex, p.release.tracks.length) - 1; i >= 0; i--) {
		if (isPreviewPlayable(p.release, i)) return { kind: 'preview', release: p.release, trackIndex: i }
	}
	return null
}

function localPreviewLast(p: PreviewPick): PreviewPick | null {
	for (let i = p.release.tracks.length - 1; i >= 0; i--) {
		if (isPreviewPlayable(p.release, i)) return { kind: 'preview', release: p.release, trackIndex: i }
	}
	return null
}

// Every playable pick of a foreign preview release not in `exclude` — the release-scoped shuffle pool
// for a looped release that isn't on screen.
function foreignReleasePool(p: PreviewPick, exclude: Set<string>): Pick[] {
	const pool: Pick[] = []
	for (let i = 0; i < p.release.tracks.length; i++) {
		const pick: PreviewPick = { kind: 'preview', release: p.release, trackIndex: i }
		if (isPreviewPlayable(p.release, i) && !exclude.has(pickKey(pick))) pool.push(pick)
	}
	return pool
}

// Whether a pick's GROUP is part of the current context (release-level for previews, mirroring the
// pre-flat `inContext(releaseId)` — a pick whose exact track drifted out of range but whose release
// is still on screen is not "foreign").
function inContext(p: Pick): boolean {
	return contextGroupKeys.has(pickGroupKey(p))
}

// The scope the sequential-previous fallback uses. Repeat-track has no meaningful "previous within
// the loop" — stepping back walks the play history like every mode, and past it the fallback behaves
// as repeat-context rather than self-looping backwards.
function prevScope(): 'off' | 'release' | 'context' {
	return repeatMode === 'track' ? 'context' : repeatMode
}

// Every playable context pick whose key isn't in `exclude` — the bag shuffle draws, optionally scoped
// to one group (repeat-release).
function buildContextPool(exclude: Set<string>, onlyGroup?: string): Pick[] {
	const pool: Pick[] = []
	for (const it of contextItems) {
		if (onlyGroup !== undefined && it.groupKey !== onlyGroup) continue
		if (isPlayablePick(it.pick) && !exclude.has(it.key)) pool.push(it.pick)
	}
	return pool
}

// Next sequential pick after `from` WITHOUT wrapping: the next playable item in list order, else null
// (end of this pass — `extendLookahead` wraps under repeat-context, stops under repeat-off). A foreign
// preview pick plays through its own self-contained release first, by reference; once exhausted (or
// for a foreign library pick, which has no standalone group) null hands over to the rejoin logic.
function sequentialAfterNoWrap(from: Pick): Pick | null {
	const i = indexOfPick(from)
	if (i !== -1) {
		const j = nextPlayableFrom(i)
		return j === -1 ? null : contextItems[j].pick
	}
	return from.kind === 'preview' ? localPreviewNext(from) : null
}

// Next sequential pick WITHIN `from`'s group only, wrapping to its first playable at the end —
// repeat-release's walk. A single-playable group hands back that same pick (the degenerate release
// loop IS a track loop); a group whose members all went unplayable yields null.
function sequentialAfterInRelease(from: Pick): Pick | null {
	const i = indexOfPick(from)
	if (i !== -1) {
		const g = contextItems[i].groupKey
		const j = nextPlayableFrom(i, g)
		if (j !== -1) return contextItems[j].pick
		const f = firstPlayable(g)
		return f === -1 ? null : contextItems[f].pick
	}
	if (from.kind === 'preview') {
		return localPreviewNext(from) ?? localPreviewFirst(from)
	}
	// A foreign library pick has no standalone group — its release loop degenerates to a track loop.
	return isPlayablePick(from) ? from : null
}

function localPreviewFirst(p: PreviewPick): PreviewPick | null {
	for (let i = 0; i < p.release.tracks.length; i++) {
		if (isPreviewPlayable(p.release, i)) return { kind: 'preview', release: p.release, trackIndex: i }
	}
	return null
}

// The most recent history entry still in the context list — where a foreign tail rejoins the context
// (Spotify semantics: a queued interlude ends, the context resumes where it left off).
function lastContextHistoryPick(): Pick | null {
	for (let i = historyPos; i >= 0; i--) {
		if (inContext(history[i])) return history[i]
	}
	return null
}

// The context's first playable pick, in list order — the wrap target and the rejoin point when no
// context pick has played.
function firstContextPick(): Pick | null {
	const j = firstPlayable()
	return j === -1 ? null : contextItems[j].pick
}

// Where the context continuation resumes from: `cur` normally, but when the current track is a
// user-queued interlude, the last context-tier pick that played. Queued items borrow playback and
// hand it back (Spotify semantics) — anchoring the walk on one made a single queued track from
// another release loop/play out its ENTIRE release (repeat-release re-anchored its group purge to
// it; the sequential walk treated it as a foreign pick and played its release through). Falls back
// to `cur` when nothing context-tier has played (a session driven purely from the user queue).
function contextSeed(): Pick | null {
	if (historyPos >= 0 && historySource[historyPos] === 'user') {
		for (let i = historyPos - 1; i >= 0; i--) {
			if (historySource[i] !== 'user') return history[i]
		}
	}
	return cur
}

// Draw one fresh shuffle pick from the bag, marking it played/reserved so it can't repeat until the
// bag is exhausted. Returns null when nothing is left to draw this pass. In release scope the pool is
// the anchor's group only — by reference when the anchor is a foreign preview release (a context play
// whose release was re-scoped off the list, or the no-context-history fallback), so its loop survives
// off-screen (a foreign library anchor has no group; its empty pool falls to the caller's degenerate
// track loop).
function drawShuffle(scope: 'off' | 'release' | 'context', anchor?: Pick): Pick | null {
	let pool: Pick[]
	if (scope === 'release' && anchor) {
		pool =
			indexOfPick(anchor) === -1 && anchor.kind === 'preview' && !inContext(anchor)
				? foreignReleasePool(anchor, shufflePlayed)
				: buildContextPool(shufflePlayed, pickGroupKey(anchor))
	} else {
		pool = buildContextPool(shufflePlayed)
	}
	if (pool.length === 0) return null
	const choice = pool[Math.floor(Math.random() * pool.length)]
	shufflePlayed.add(pickKey(choice))
	return choice
}

// Fingerprint of everything the committed lookahead's VALIDITY depends on: repeat mode, shuffle,
// and the scope anchor (repeat-release's group / repeat-track's pick). Checked on EVERY read
// (`extendLookahead`): if any of these changed and some mutation path failed to clear the lookahead,
// the next read rebuilds it instead of serving a stale committed order — validity is structural, not
// event-wired. The context LIST is deliberately not fingerprinted (a list swap keeps surviving picks
// on purpose, see `reconcileLookaheadAfterContextSwap`), and a fingerprint reset does not touch the
// shuffle bag (keys for dropped picks are inert; the coupled bag reset stays in setShuffle /
// setRepeatMode, which remain the primary invalidation path — this is the safety net).
function lookaheadFingerprint(): string {
	let anchor = ''
	if (repeatMode === 'track') {
		anchor = cur ? pickKey(cur) : ''
	} else if (repeatMode === 'release') {
		const seed = contextSeed()
		if (seed) anchor = pickGroupKey(seed)
	}
	return `${repeatMode}|${shuffleEnabled ? '1' : '0'}|${anchor}`
}
let committedFingerprint = ''

/**
 * Extend the committed context lookahead until it holds `targetLen` picks (or nothing more can play).
 * Sequential walks forward from the tail; shuffle reserves fresh random draws. This is the ONE place
 * repeat scope shapes what comes next — wrapping/reshuffling happens here so actual playback, the Up
 * Next forecast, the iOS native window, and `canAdvance` all agree:
 *  - off      → stop at the end of the pass (no wrap, no reshuffle).
 *  - track    → the current pick repeats, literally (see `RepeatMode`).
 *  - release  → loop within the seed's group (see `contextSeed`); shuffle draws from that group only.
 *  - context  → wrap to the context's first playable / re-seed the bag each pass.
 */
function extendLookahead(targetLen: number) {
	const fp = lookaheadFingerprint()
	if (fp !== committedFingerprint) {
		contextLookahead = []
		committedFingerprint = fp
	}
	const mode = repeatMode
	// Repeat-track is literal (see `RepeatMode`): the queue's own "next" replays the current pick and
	// the forecast shows it. The explicit user queue still precedes the context in `advanceNext`, so
	// queued items stay reachable by a manual skip.
	if (mode === 'track') {
		if (!cur || !isPlayablePick(cur)) return
		while (contextLookahead.length < targetLen) contextLookahead.push(cur)
		return
	}
	const scope = mode
	// The context continuation seeds from `contextSeed`, NOT `cur`: a user-queued interlude that is
	// currently playing must not redirect the walk to its own release.
	const seed = contextSeed()
	let guard = 0
	while (contextLookahead.length < targetLen) {
		if (guard++ > 1000) break // safety against any unforeseen non-terminating draw
		const tail = contextLookahead.length > 0 ? contextLookahead[contextLookahead.length - 1] : seed
		if (!tail) break
		let next: Pick | null
		if (shuffleEnabled) {
			const anchor = seed ?? tail
			next = drawShuffle(scope, anchor)
			if (!next && scope !== 'off') {
				// Pass exhausted while repeating: re-seed the bag anchored on the last committed pick (NOT
				// `cur` — the lookahead may already sit a pass ahead) and start the next pass.
				shufflePlayed = new Set([pickKey(tail)])
				next = drawShuffle(scope, anchor)
				// A group with a single playable pick has an empty pool even after re-seeding (the anchor
				// is excluded) — the degenerate release loop is a self-loop, matching the sequential walk.
				if (!next && scope === 'release' && isPlayablePick(tail)) {
					next = tail
				}
			}
		} else if (scope === 'release') {
			next = sequentialAfterInRelease(tail)
		} else {
			next = sequentialAfterNoWrap(tail)
			if (!next) {
				// A foreign tail (see `inContext`) has finished its own release and can't walk the context
				// cross-release — rejoin the context instead of dead-ending (which disabled "next" and
				// silently ended playback in every mode but repeat-release).
				if (!inContext(tail)) {
					const anchor = lastContextHistoryPick()
					next = anchor ? sequentialAfterNoWrap(anchor) : firstContextPick()
				}
				if (!next && scope === 'context') {
					const wrapped = firstContextPick()
					// Don't hand back the tail as its own "next" — a lone single-playable context would loop
					// onto itself forever (repeat-track is the mode for that).
					next = wrapped && pickKey(wrapped) === pickKey(tail) ? null : wrapped
				}
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

function pushHistory(pick: Pick, source: 'user' | 'context') {
	// Consecutive same-pick entries collapse: a repeat-track skip (or a degenerate one-pick loop)
	// REPLAYS the entry rather than moving to a new one. Stacking dupes would make "previous" step
	// back through every replay, and would bury the entry's tier — a looped user-queue interlude must
	// keep its 'user' provenance so `contextSeed` still skips it once the mode changes.
	const last = history[history.length - 1]
	if (last && pickKey(last) === pickKey(pick)) {
		historyPos = history.length - 1
		return
	}
	history.push(pick)
	historySource.push(source)
	if (history.length > HISTORY_CAP) {
		history.splice(0, history.length - HISTORY_CAP)
		historySource.splice(0, historySource.length - HISTORY_CAP)
	}
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
		out.push({ pick: e.pick, source: 'user', entryId: e.entryId })
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
	// off stops, unplayable picks never count) — commit one pick and check.
	extendLookahead(1)
	return contextLookahead.length > 0
}

function refresh() {
	const replayCount = Math.max(0, history.length - 1 - historyPos)
	const depth = replayCount + userQueue.length + displayContextDepth()
	// UpNextEntry stays preview-shaped (its only consumers are the mobile queue sheet / pager);
	// library picks are simply not surfaced there.
	const entries = upcomingPicks(depth).flatMap((u, i): UpNextEntry[] =>
		u.pick.kind === 'preview'
			? [
					{
						key: u.entryId ?? `${pickKey(u.pick)}:${i}`,
						source: u.source,
						release: u.pick.release,
						trackIndex: u.pick.trackIndex,
					},
				]
			: []
	)
	upNextStore.set(entries)
	userQueueCountStore.set(userQueue.length)
	canAdvanceStore.set(computeCanAdvance())
}

// --- Persistence (explicit user queue only — ids, re-hydrated on launch) ---------------------------
// Bumped on every user-queue commit (each mutation persists). `hydrate` snapshots it before its async
// release fetches and stands down if anything committed meanwhile — so a slow boot-time hydrate can
// never clobber a queue the user (or an earlier hydrate) already touched.
let userQueueGeneration = 0
function persistUserQueue() {
	userQueueGeneration++
	// Preview entries only: library queue entries are session-local (a library track id without its
	// context list isn't worth restoring, and `QueuePayload` is preview-shaped today).
	const items: QueueItem[] = userQueue.flatMap((e) =>
		e.pick.kind === 'preview'
			? [
					{
						entryId: e.entryId,
						payload: { kind: 'preview' as const, releaseId: e.pick.release.id, trackIndex: e.pick.trackIndex },
					},
				]
			: []
	)
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
 * `setShuffle` — lookahead draws reserve bag keys, so the two only ever reset together (a context-list
 * swap is the deliberate exception, see `updatePreviewContext`).
 */
export function setRepeatMode(mode: RepeatMode) {
	if (repeatMode === mode) return
	repeatMode = mode
	contextLookahead = []
	shufflePlayed = shuffleEnabled && cur ? new Set([pickKey(cur)]) : new Set()
	refresh()
	onQueueChanged?.()
}

/** The currently-playing pick, or null. Player keeps this in sync by routing every transition here. */
export function currentPick(): Pick | null {
	return cur
}

/** The kind of the ACTIVE session's context, or null when nothing is playing. Desktop's live-context
 *  subscriptions gate on this so a library view change can't re-scope a preview session or vice versa. */
export function contextKindOf(): 'preview' | 'library' | null {
	return cur ? contextKind : null
}

function startSessionWith(
	kind: 'preview' | 'library',
	pick: Pick,
	items: ContextItem[],
	raw: DiscoveryRelease[] | Track[],
	opts?: { logPlay?: boolean }
) {
	installContext(kind, raw, items)
	cur = pick
	history = [pick]
	historySource = ['context']
	historyPos = 0
	contextLookahead = []
	shufflePlayed = shuffleEnabled ? new Set([pickKey(pick)]) : new Set()
	if (opts?.logPlay !== false) logRecent(pick)
	refresh()
}

/**
 * Begin a preview session from a user-initiated play: capture the context list, anchor the current
 * track and a fresh play history, and reset the shuffle bag/lookahead. The user queue is intentionally
 * KEPT — explicitly queued items survive starting a new track. `opts.logPlay: false` skips the
 * listening log (the relaunch restore re-anchors the last session's track without the user playing
 * anything).
 */
export function startPreviewSession(
	release: DiscoveryRelease,
	trackIndex: number,
	contextReleases: DiscoveryRelease[],
	opts?: { logPlay?: boolean }
) {
	startSessionWith(
		'preview',
		{ kind: 'preview', release, trackIndex },
		flattenPreview(contextReleases),
		contextReleases,
		opts
	)
}

/** Begin a library session from a user-initiated play — the library counterpart of
 *  `startPreviewSession`, with the visible track list as the context. */
export function startLibrarySession(track: Track, contextTracks: Track[], opts?: { logPlay?: boolean }) {
	startSessionWith('library', { kind: 'library', track }, flattenLibrary(contextTracks), contextTracks, opts)
}

/**
 * Swap the CONTEXT list of the active session in place, keeping the current track, play history, user
 * queue, committed lookahead (filtered to survivors), and the shuffle bag — only picks that left the
 * list are re-derived. Used when the view the session was started from changes its on-screen set
 * (e.g. the discovery feed's filter is applied/reset while a feed-originated preview plays), so
 * next/shuffle keep spanning exactly what's on screen. No-op shape when nothing is playing (`cur`
 * null): it just stores the list for the next session start. A repeat-release loop on a foreign
 * release is unaffected by design — it follows `cur`'s release by reference, not the context list.
 *
 * The bag and lookahead survive a list change ON PURPOSE, unlike setShuffle/setRepeatMode's coupled
 * reset: the feed mutates itself during normal playback (playing a "new"-filtered release clears its
 * flag and drops it off the list; background sync adds rows), and resetting the bag to one key on
 * every such change is exactly what made shuffle replay tracks. Bag keys for departed picks are
 * inert — `buildContextPool` iterates the CURRENT context — and keeping the rest is what makes
 * no-repeat-until-exhausted actually mean that.
 */
export function updatePreviewContext(contextReleases: DiscoveryRelease[]) {
	// No-op when the list is unchanged: the feed's derived re-emits on unrelated UI-store changes, and
	// re-flattening/filtering every time would churn the native window for nothing.
	if (contextKind === 'preview' && samePreviewList(contextRaw as DiscoveryRelease[], contextReleases)) return
	installContext('preview', contextReleases, flattenPreview(contextReleases))
	reconcileLookaheadAfterContextSwap()
}

/** The library counterpart of `updatePreviewContext` (sort change, filter, playlist edit while the
 *  originating view is still active). */
export function updateLibraryContext(contextTracks: Track[]) {
	if (contextKind === 'library' && sameTrackList(contextRaw as Track[], contextTracks)) return
	installContext('library', contextTracks, flattenLibrary(contextTracks))
	reconcileLookaheadAfterContextSwap()
}

function reconcileLookaheadAfterContextSwap() {
	const before = contextLookahead.length
	// Keep the committed order for picks still on screen, re-mapped onto the fresh objects (the list's
	// releases/tracks may be new instances carrying updated flags).
	contextLookahead = contextLookahead.flatMap((p) => {
		const i = contextIndexByKey.get(pickKey(p))
		return i !== undefined ? [contextItems[i].pick] : []
	})
	const filtered = contextLookahead.length
	refresh()
	// The upcoming content changed if the filter dropped committed picks OR the refresh drew fresh ones
	// into the vacated/extended slots — either way the iOS engine's tail must be re-fed. Checking only
	// the post-refresh length would miss a drop that refilled back to the same depth.
	if (filtered !== before || contextLookahead.length !== filtered) onQueueChanged?.()
}

/** Toggle shuffle: re-anchor + redraw the CONTEXT order only. User queue and history are untouched. */
export function setShuffle(enabled: boolean) {
	if (shuffleEnabled === enabled) return
	shuffleEnabled = enabled
	contextLookahead = []
	shufflePlayed = enabled && cur ? new Set([pickKey(cur)]) : new Set()
	refresh()
	onQueueChanged?.()
}

/** Append a preview track to the end of the explicit user queue. */
export function addToQueue(release: DiscoveryRelease, trackIndex: number) {
	userQueue.push({ entryId: genEntryId(), pick: { kind: 'preview', release, trackIndex } })
	persistUserQueue()
	refresh()
	onQueueChanged?.()
}

/** Front-insert a preview track so it plays immediately after the current one (ahead of the queue). */
export function playNext(release: DiscoveryRelease, trackIndex: number) {
	userQueue.unshift({ entryId: genEntryId(), pick: { kind: 'preview', release, trackIndex } })
	persistUserQueue()
	refresh()
	onQueueChanged?.()
}

// Build a user-queue entry per playable track of a release, in track order — the whole-release
// equivalent of one `addToQueue`/`playNext` call. Each track gets its own stable entry id (it's an
// independent queue row).
function releaseEntries(release: DiscoveryRelease): UserEntry[] {
	const entries: UserEntry[] = []
	for (let i = 0; i < release.tracks.length; i++) {
		if (isPreviewPlayable(release, i)) {
			entries.push({ entryId: genEntryId(), pick: { kind: 'preview', release, trackIndex: i } })
		}
	}
	return entries
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
 * Advance to the next pick (the single source of truth for "next", used by every platform and both
 * sources). Replays forward through history when the user stepped back; otherwise consumes the user
 * queue, then context. Returns null when nothing more can play. Pops the consumed user item (and
 * persists).
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
	let source: 'user' | 'context'
	if (userQueue.length > 0) {
		const entry = userQueue.shift()!
		pick = entry.pick
		source = 'user'
		persistUserQueue()
	} else {
		pick = takeContextNext()
		source = 'context'
	}
	if (!pick) {
		refresh()
		return null
	}
	pushHistory(pick, source)
	cur = pick
	logRecent(cur)
	refresh()
	return pick
}

// The deterministic sequential "previous" of `from` under the current repeat scope (the fallback once
// the play history is exhausted): the previous playable pick in list order, then per scope — release:
// wrap to the group's last playable (null when `from` is its only one); off: no wrap past the start;
// context/track: wrap to the context's last playable. A foreign preview pick walks its own release by
// reference first; under a non-release scope it sits "past the end", so every on-screen pick is an
// earlier one.
function sequentialPrevOf(from: Pick): Pick | null {
	const scope = prevScope()
	const i = indexOfPick(from)
	if (i !== -1) {
		if (scope === 'release') {
			const g = contextItems[i].groupKey
			const j = prevPlayableFrom(i, g)
			if (j !== -1) return contextItems[j].pick
			const l = lastPlayable(g)
			if (l === -1 || l === i) return null
			return contextItems[l].pick
		}
		const j = prevPlayableFrom(i)
		if (j !== -1) return contextItems[j].pick
		if (scope === 'off') return null
		const l = lastPlayable()
		return l === -1 ? null : contextItems[l].pick
	}
	if (from.kind === 'preview') {
		const within = localPreviewPrev(from)
		if (within) return within
		if (scope === 'release') {
			const last = localPreviewLast(from)
			if (!last || last.trackIndex === from.trackIndex) return null
			return last
		}
	} else if (scope === 'release') {
		// A foreign library pick has no standalone group — nothing to step back to within it.
		return null
	}
	const l = lastPlayable()
	return l === -1 ? null : contextItems[l].pick
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
		// Mirror advanceNext's forward-replay branch: stepping back to a track and hearing it is a listen.
		logRecent(cur)
		refresh()
		return cur
	}
	if (shuffleEnabled || !cur) return null
	const prev = sequentialPrevOf(cur)
	if (!prev) return null
	history.unshift(prev)
	historySource.unshift('context')
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

/**
 * End the session (stop / reset): clear the context, lookahead, history, and current pick. The
 * explicit user queue — in memory AND persisted — deliberately survives: stopping playback is not
 * "forget what I queued" (call `clearUserQueue` for that; `playerStore.reset` does).
 */
export function clearAll() {
	contextRaw = []
	contextItems = []
	contextIndexByKey = new Map()
	contextGroupKeys = new Set()
	contextLookahead = []
	history = []
	historySource = []
	historyPos = -1
	shufflePlayed = new Set()
	cur = null
	refresh()
}

/**
 * Re-hydrate the persisted user queue after a relaunch: re-fetch each release by id (like
 * restorePreview), dropping entries whose release no longer exists or whose track index is now out of
 * range. Safe to call on the non-critical boot path; triggers a native re-feed if something is playing.
 * Non-clobbering: any user-queue commit during the async fetches (a boot-time addToQueue, a concurrent
 * hydrate finishing first) makes this one stand down rather than overwrite it. Call it unconditionally
 * at app boot — restorePreview only reaches it when a preview was playing, and an unhydrated module
 * would overwrite the persisted queue on the very next addToQueue.
 */
export async function hydrate(): Promise<void> {
	const generationAtStart = userQueueGeneration
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
		if (!isPreviewPlayable(release, trackIndex)) continue
		resolved.push({ entryId: item.entryId || genEntryId(), pick: { kind: 'preview', release, trackIndex } })
	}
	if (userQueueGeneration !== generationAtStart) return // something committed mid-hydrate — stand down
	userQueue = resolved
	persistUserQueue()
	refresh()
	onQueueChanged?.()
}
