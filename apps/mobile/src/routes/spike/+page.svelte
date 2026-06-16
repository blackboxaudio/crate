<script lang="ts">
	// iOS audio-proxy validation harness (#80). THROWAWAY dev UI — plain English strings, no i18n.
	//
	// Drives the REAL discovery preview path on-device (fetch metadata → create release → prefetch →
	// fetchPreviewStream → localhost axum proxy → HTML5 Audio in WKWebView) so the proxy can be
	// validated on iOS, plus a WebView script-injection round-trip that mirrors the mechanism the
	// YouTube n-param de-throttle (n_transform.rs) depends on. Reuses the shared player store, so
	// seeking exercises real Range requests through the proxy. Remove (or fold into #54) once findings
	// are captured. Inline styles match the existing mobile scaffold (no Tailwind configured here).
	import { invoke } from '@tauri-apps/api/core'
	import * as discoveryApi from '$shared/api/discovery'
	import {
		playerStore,
		isPlaying,
		playbackPosition,
		playbackDuration,
		playbackSource,
		previewLoadingReleaseId,
	} from '$shared/stores/player'
	import type { DiscoveryRelease, DiscoveryReleaseCreate, DiscoverySourceType } from '$shared/types'

	let url = $state('')
	let seeding = $state(false)
	let release = $state<DiscoveryRelease | null>(null)
	let cacheBytes = $state<number | null>(null)
	let evalResult = $state<string | null>(null)
	let evalRunning = $state(false)
	let logLines = $state<string[]>([])
	let resolvedUrl = $state<string | null>(null)

	const isPreview = $derived($playbackSource === 'preview')

	function log(msg: string) {
		// Newest first; mirror events to the screen so findings are readable on a physical device
		// without a tethered Xcode console. Cap the buffer so the panel stays responsive.
		logLines = [`${new Date().toTimeString().slice(0, 8)}  ${msg}`, ...logLines].slice(0, 200)
	}

	function fmtMs(ms: number): string {
		const s = Math.max(0, Math.floor(ms / 1000))
		return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
	}

	function errMsg(e: unknown): string {
		return typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e)
	}

	async function seed() {
		const target = url.trim()
		if (!target) return
		seeding = true
		release = null
		log(`Fetching metadata for ${target}`)
		try {
			const data = await discoveryApi.fetchMetadata(target)
			log(`Metadata: ${data.source_type} — ${data.artist ?? '?'} / ${data.title ?? '?'} (${data.tracks.length} tracks)`)
			const create: DiscoveryReleaseCreate = {
				url: target,
				source_type:
					data.source_type && data.source_type !== 'other' ? (data.source_type as DiscoverySourceType) : undefined,
				artist: data.artist ?? undefined,
				title: data.title ?? undefined,
				label: data.label ?? undefined,
				release_date: data.release_date ?? undefined,
				artwork_url: data.artwork_url ?? undefined,
				parent_url: data.parent_url ?? undefined,
				tracks: data.tracks.map((t) => ({
					name: t.name,
					position: t.position,
					duration_ms: t.duration_ms ?? undefined,
					video_id: t.video_id ?? undefined,
				})),
			}
			release = await discoveryApi.createRelease(create)
			log(`Release created: id=${release.id}, ${release.tracks.length} tracks. Stream prefetch started.`)
		} catch (e) {
			log(`SEED ERROR: ${errMsg(e)}`)
		} finally {
			seeding = false
		}
	}

	async function play() {
		if (!release) return
		log(`playPreview → track "${release.tracks[0]?.name ?? '?'}" (fetchPreviewStream → proxy → HTML5 Audio)`)
		try {
			await playerStore.playPreview(release, 0)
		} catch (e) {
			log(`PLAY ERROR: ${errMsg(e)}`)
		}
	}

	function seekTo(fraction: number) {
		const ms = Math.floor($playbackDuration * fraction)
		log(`seek → ${fmtMs(ms)} (${(fraction * 100).toFixed(0)}%) — expect a 206 Range request in the console`)
		playerStore.seek(ms)
	}

	function seekBy(offsetMs: number) {
		log(`seekRelative ${offsetMs > 0 ? '+' : ''}${offsetMs / 1000}s — expect a 206 Range request`)
		playerStore.seekRelative(offsetMs)
	}

	async function refreshCache() {
		try {
			cacheBytes = await discoveryApi.getAudioCacheSize()
			log(`Audio cache on disk: ${(cacheBytes / 1_048_576).toFixed(2)} MB`)
		} catch (e) {
			log(`CACHE ERROR: ${errMsg(e)}`)
		}
	}

	async function clearCache() {
		try {
			await discoveryApi.clearAudioCache()
			log('Audio cache cleared')
			await refreshCache()
		} catch (e) {
			log(`CLEAR ERROR: ${errMsg(e)}`)
		}
	}

	async function runInjectionTest() {
		evalRunning = true
		evalResult = null
		log('WebView round-trip: invoking spike_webview_roundtrip (inject JS → JS calls Rust back)')
		try {
			const res = await invoke<string>('spike_webview_roundtrip')
			evalResult = res
			log(`Round-trip OK: ${res}`)
		} catch (e) {
			evalResult = `FAILED: ${errMsg(e)}`
			log(`Round-trip FAILED: ${errMsg(e)}`)
		} finally {
			evalRunning = false
		}
	}

	// --- Raw proxy test: bypasses the shared player store to isolate "does the localhost proxy +
	// HTML5 Audio actually work on iOS WKWebView?". `resolveUrl` is async (gets the proxy URL);
	// `playRaw` calls audio.play() fully synchronously inside the tap so the iOS user-gesture isn't
	// lost across an await — the shared playPreview awaits fetchPreviewStream BEFORE play(), which can
	// forfeit the gesture and make play() reject with NotAllowedError on iOS.
	let rawAudio: HTMLAudioElement | null = null

	async function resolveUrl() {
		if (!release) return
		const track = release.tracks[0]
		if (!track) return
		try {
			resolvedUrl = await discoveryApi.fetchPreviewStream(release.id, track.position)
			log(`Resolved stream URL: ${resolvedUrl}`)
		} catch (e) {
			log(`RESOLVE ERROR: ${errMsg(e)}`)
		}
	}

	function playRaw() {
		if (!resolvedUrl) {
			log('No resolved URL — tap "Resolve stream URL" first')
			return
		}
		const a = new Audio()
		rawAudio = a
		const events = [
			'loadstart',
			'loadedmetadata',
			'canplay',
			'playing',
			'waiting',
			'stalled',
			'suspend',
			'ended',
			'error',
		]
		for (const ev of events) {
			a.addEventListener(ev, () => {
				if (ev === 'error') {
					const me = a.error
					log(
						`raw ERROR code=${me?.code ?? '?'} (1=abort 2=network 3=decode 4=src-unsupported) net=${a.networkState} ready=${a.readyState}`
					)
				} else if (ev === 'loadedmetadata') {
					log(`raw loadedmetadata: duration=${a.duration.toFixed(2)}s`)
				} else {
					log(`raw ${ev}`)
				}
			})
		}
		a.src = resolvedUrl
		log(`raw play() on ${resolvedUrl}`)
		a.play()
			.then(() => log('raw play() resolved'))
			.catch((e) => log(`raw play() REJECTED — ${e?.name ?? ''} ${errMsg(e)}`))
	}

	function stopRaw() {
		if (rawAudio) {
			rawAudio.pause()
			rawAudio.src = ''
			rawAudio = null
			log('raw audio stopped')
		}
	}

	// Surface player-store errors on-screen — the mobile scaffold has no toast UI, so preview
	// failures (proxy 502, autoplay blocked, …) would otherwise be invisible.
	let lastError: string | null = null
	$effect(() => {
		const e = $playerStore.error
		if (e !== lastError) {
			lastError = e
			if (e) log(`playerStore.error: ${e}`)
		}
	})

	// Passively surface key playback transitions to the log so the duration/playing changes are
	// visible on-device. The duration line is the tell for the AVFoundation 206-without-200 bug.
	let lastDuration = 0
	let lastPlaying = false
	$effect(() => {
		const d = $playbackDuration
		if (d !== lastDuration) {
			lastDuration = d
			if (d > 0) log(`duration_ms = ${d} (${fmtMs(d)}) — verify this matches the real track length`)
		}
	})
	$effect(() => {
		const p = $isPlaying
		if (p !== lastPlaying) {
			lastPlaying = p
			log(p ? 'playing' : 'paused/stopped')
		}
	})
</script>

<main style="font-family: sans-serif; padding: 1rem; max-width: 680px; margin: 0 auto;">
	<p style="margin: 0 0 0.5rem;"><a href="/">← back</a></p>
	<h1 style="margin: 0 0 1rem;">iOS Audio Proxy Spike (#80)</h1>

	<section style="margin-bottom: 1.5rem;">
		<h2 style="font-size: 1.1rem;">1 · Seed a real release</h2>
		<p style="font-size: 0.85rem; color: #555; margin: 0.25rem 0;">
			Paste a Bandcamp or SoundCloud track/album URL (no n-param needed), then create it. This runs the genuine metadata
			fetch + stream prefetch on the device.
		</p>
		<input
			type="url"
			bind:value={url}
			placeholder="https://artist.bandcamp.com/track/..."
			autocapitalize="off"
			autocomplete="off"
			spellcheck="false"
			style="width: 100%; padding: 0.6rem; box-sizing: border-box; font-size: 0.95rem;"
		/>
		<button onclick={seed} disabled={seeding || !url.trim()} style="margin-top: 0.5rem; padding: 0.5rem 0.9rem;">
			{seeding ? 'Seeding…' : 'Fetch metadata + create release'}
		</button>
		{#if release}
			<p style="font-size: 0.85rem; margin: 0.5rem 0 0;">
				✓ {release.artist ?? '?'} — {release.title ?? '?'} ({release.tracks.length} tracks)
			</p>
		{/if}
	</section>

	<section style="margin-bottom: 1.5rem;">
		<h2 style="font-size: 1.1rem;">2 · Play & seek through the proxy</h2>
		<div style="display: flex; gap: 0.4rem; flex-wrap: wrap;">
			<button onclick={play} disabled={!release || $previewLoadingReleaseId !== null}>Play preview</button>
			<button onclick={() => playerStore.pause()} disabled={!isPreview}>Pause</button>
			<button onclick={() => playerStore.resume()} disabled={!isPreview}>Resume</button>
			<button onclick={() => seekBy(-10000)} disabled={!isPreview}>−10s</button>
			<button onclick={() => seekBy(10000)} disabled={!isPreview}>+10s</button>
			<button onclick={() => seekTo(0.25)} disabled={!isPreview || !$playbackDuration}>25%</button>
			<button onclick={() => seekTo(0.5)} disabled={!isPreview || !$playbackDuration}>50%</button>
			<button onclick={() => seekTo(0.75)} disabled={!isPreview || !$playbackDuration}>75%</button>
			<button onclick={() => playerStore.stop()} disabled={!isPreview}>Stop</button>
		</div>
		<table style="margin-top: 0.6rem; font-size: 0.85rem; border-collapse: collapse;">
			<tbody>
				<tr><td style="padding-right: 1rem; color: #555;">source</td><td>{$playbackSource}</td></tr>
				<tr><td style="padding-right: 1rem; color: #555;">playing</td><td>{$isPlaying}</td></tr>
				<tr
					><td style="padding-right: 1rem; color: #555;">position</td><td
						>{fmtMs($playbackPosition)} ({$playbackPosition} ms)</td
					></tr
				>
				<tr
					><td style="padding-right: 1rem; color: #555;"><strong>duration</strong></td><td
						><strong>{fmtMs($playbackDuration)} ({$playbackDuration} ms)</strong></td
					></tr
				>
			</tbody>
		</table>
		<p style="font-size: 0.8rem; color: #a15; margin: 0.4rem 0 0;">
			⚠ If duration reads ~2× the real track length, that's the AVFoundation 206-without-200 bug.
		</p>
	</section>

	<section style="margin-bottom: 1.5rem;">
		<h2 style="font-size: 1.1rem;">2½ · Raw proxy test (isolates the proxy)</h2>
		<p style="font-size: 0.85rem; color: #555; margin: 0.25rem 0;">
			Bypasses the player store. <strong>Resolve</strong> gets the proxy URL (async), then
			<strong>Play raw</strong> calls <code>audio.play()</code> synchronously inside the tap so the iOS user-gesture survives.
			Every audio event + the exact error code is logged below.
		</p>
		<div style="display: flex; gap: 0.4rem; flex-wrap: wrap;">
			<button onclick={resolveUrl} disabled={!release}>Resolve stream URL</button>
			<button onclick={playRaw} disabled={!resolvedUrl}>Play raw</button>
			<button onclick={stopRaw}>Stop raw</button>
		</div>
		{#if resolvedUrl}
			<pre
				style="white-space: pre-wrap; word-break: break-all; background: #f4f4f4; padding: 0.4rem; font-size: 0.72rem; margin: 0.5rem 0 0;">{resolvedUrl}</pre>
		{/if}
	</section>

	<section style="margin-bottom: 1.5rem;">
		<h2 style="font-size: 1.1rem;">3 · Memory / cache</h2>
		<div style="display: flex; gap: 0.4rem; flex-wrap: wrap;">
			<button onclick={refreshCache}>Refresh cache size</button>
			<button onclick={clearCache}>Clear audio cache</button>
		</div>
		{#if cacheBytes !== null}
			<p style="font-size: 0.85rem; margin: 0.5rem 0 0;">
				Audio cache on disk: {(cacheBytes / 1_048_576).toFixed(2)} MB
			</p>
		{/if}
		<p style="font-size: 0.8rem; color: #555; margin: 0.4rem 0 0;">
			Play 3+ different releases to fill the in-memory cache (3 entries / ~50 MB) and watch Xcode's memory gauge for
			pressure or eviction.
		</p>
	</section>

	<section style="margin-bottom: 1.5rem;">
		<h2 style="font-size: 1.1rem;">4 · WebView script-injection round-trip</h2>
		<p style="font-size: 0.85rem; color: #555; margin: 0.25rem 0;">
			Validates the mechanism YouTube n-param de-throttling needs: inject JS into the "main" WebView, have it call Rust
			back, unblock via a oneshot channel. Should return a sum and the WKWebView UA.
		</p>
		<button onclick={runInjectionTest} disabled={evalRunning} style="padding: 0.5rem 0.9rem;">
			{evalRunning ? 'Running…' : 'Run round-trip test'}
		</button>
		{#if evalResult}
			<pre
				style="white-space: pre-wrap; word-break: break-all; background: #f4f4f4; padding: 0.5rem; font-size: 0.8rem; margin: 0.5rem 0 0;">{evalResult}</pre>
		{/if}
	</section>

	<section>
		<div style="display: flex; align-items: center; gap: 0.6rem;">
			<h2 style="font-size: 1.1rem; margin: 0;">Event log</h2>
			<button onclick={() => (logLines = [])}>Clear</button>
		</div>
		<pre
			style="white-space: pre-wrap; word-break: break-word; background: #111; color: #3f8; padding: 0.6rem; font-size: 0.72rem; max-height: 45vh; overflow: auto; margin: 0.5rem 0 0;">{logLines.join(
				'\n'
			)}</pre>
	</section>
</main>
