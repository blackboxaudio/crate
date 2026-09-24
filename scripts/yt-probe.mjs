#!/usr/bin/env node
// Probe the YouTube / Discogs preview pipeline outside the app.
//
// Replays exactly what src-tauri/src/services/discovery/metadata/youtube.rs does — the same
// innertube client chain, the same page-issued visitor session — against real endpoints, so a
// "previews stopped working" report can be diagnosed in a minute without a build:
//
//   node scripts/yt-probe.mjs                 # default sample of videos
//   node scripts/yt-probe.mjs dQw4w9WgXcQ …   # specific video ids
//
// Keep CLIENTS in sync with `YT_CLIENTS` in youtube.rs. When every client fails, diff that table
// against yt-dlp's `INNERTUBE_CLIENTS` (yt_dlp/extractor/youtube/_base.py) first.

const SOCS = 'SOCS=CAISNJAgJB'
const CLIENTS = [
	{
		name: 'VISIONOS',
		id: '101',
		version: '1.02',
		userAgent:
			'Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15',
		context: { deviceMake: 'Apple', deviceModel: 'RealityDevice17,1', osName: 'visionOS', osVersion: '26.5.23O471' },
	},
	{
		name: 'ANDROID_VR',
		id: '28',
		version: '1.65.10',
		userAgent:
			'com.google.android.apps.youtube.vr.oculus/1.65.10 (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip',
		context: { deviceMake: 'Oculus', deviceModel: 'Quest 3', osName: 'Android', osVersion: '12L', androidSdkVersion: 32 },
	},
]
const DEFAULT_VIDEOS = ['dQw4w9WgXcQ', '7FwDP17XPlk', 'hTWKbfoikeg']
const PLAYLIST_ID = 'PLlaN88a7y2_oBUxLd3j23dkAFNtM-P24e'
const DISCOGS_RELEASE = 249504

const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const cookies = new Map([['SOCS', 'CAISNJAgJB']])
const cookieHeader = () => [...cookies].map(([k, v]) => `${k}=${v}`).join('; ')
function absorbCookies(res) {
	for (const line of res.headers.getSetCookie?.() ?? []) {
		const [pair] = line.split(';')
		const eq = pair.indexOf('=')
		if (eq > 0) cookies.set(pair.slice(0, eq).trim(), pair.slice(eq + 1).trim())
	}
}

async function establishSession() {
	const res = await fetch('https://www.youtube.com/', {
		headers: { 'User-Agent': CLIENTS[0].userAgent, Cookie: cookieHeader(), 'Accept-Language': 'en-US,en;q=0.9' },
	})
	absorbCookies(res)
	const html = await res.text()
	const m = html.match(/"VISITOR_DATA":"([^"]+)"/)
	if (!m) throw new Error('homepage has no VISITOR_DATA (ytcfg shape changed?)')
	return m[1]
}

async function player(videoId, client, visitorData) {
	const body = {
		videoId,
		contentCheckOk: true,
		racyCheckOk: true,
		context: {
			client: {
				clientName: client.name,
				clientVersion: client.version,
				hl: 'en',
				timeZone: 'UTC',
				utcOffsetMinutes: 0,
				visitorData,
				...client.context,
				userAgent: client.userAgent,
			},
		},
		playbackContext: { contentPlaybackContext: { html5Preference: 'HTML5_PREF_WANTS' } },
	}
	const res = await fetch('https://www.youtube.com/youtubei/v1/player?prettyPrint=false', {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			Origin: 'https://www.youtube.com',
			'User-Agent': client.userAgent,
			'X-YouTube-Client-Name': client.id,
			'X-YouTube-Client-Version': client.version,
			'X-Goog-Visitor-Id': visitorData,
			Cookie: cookieHeader(),
		},
		body: JSON.stringify(body),
	})
	absorbCookies(res)
	if (!res.ok) return { httpError: res.status }
	return res.json()
}

async function cdnRange(url, userAgent) {
	const t0 = Date.now()
	const res = await fetch(url, { headers: { Range: 'bytes=0-1048575', 'User-Agent': userAgent } })
	const bytes = (await res.arrayBuffer()).byteLength
	return `${res.status} ${res.headers.get('content-type')} ${(bytes / 1024).toFixed(0)}KB in ${Date.now() - t0}ms`
}

async function probeVideo(videoId, visitorData) {
	for (const client of CLIENTS) {
		const p = await player(videoId, client, visitorData)
		if (p.httpError) {
			console.log(`  ${client.name.padEnd(11)} HTTP ${p.httpError} (client name/version rejected?)`)
			continue
		}
		const status = p.playabilityStatus?.status
		const reason = p.playabilityStatus?.reason ?? ''
		const formats = p.streamingData?.adaptiveFormats ?? []
		const audio = formats.filter((f) => f.mimeType?.startsWith('audio/'))
		const direct = audio.find((f) => f.itag === 140 && f.url) ?? audio.find((f) => f.url)
		const ciphered = audio.filter((f) => f.signatureCipher).length
		console.log(
			`  ${client.name.padEnd(11)} status=${status} audio=${audio.length} direct=${direct ? 'yes' : 'no'} signatureCipher=${ciphered} sabr=${'serverAbrStreamingUrl' in (p.streamingData ?? {})} ${reason && `reason="${reason}"`}`
		)
		if (status === 'OK' && direct) {
			const hasN = new URL(direct.url).searchParams.has('n')
			console.log(`              n-param=${hasN}  CDN(range 1MB, client UA): ${await cdnRange(direct.url, client.userAgent)}`)
			return 'ok'
		}
		if (status === 'LOGIN_REQUIRED') return 'bot-check'
	}
	return 'failed'
}

async function probePlaylist() {
	const res = await fetch(`https://www.youtube.com/playlist?list=${PLAYLIST_ID}`, {
		headers: { 'User-Agent': CLIENTS[0].userAgent, Cookie: SOCS },
	})
	const html = await res.text()
	const start = html.indexOf('var ytInitialData = ')
	const json = html.slice(start + 'var ytInitialData = '.length, html.indexOf(';</script>', start))
	const data = JSON.parse(json)
	const items =
		data.contents?.twoColumnBrowseResultsRenderer?.tabs?.[0]?.tabRenderer?.content?.sectionListRenderer?.contents?.[0]
			?.itemSectionRenderer?.contents ?? []
	const legacy = items.flatMap((i) => i.playlistVideoListRenderer?.contents ?? []).filter((c) => c.playlistVideoRenderer)
	const lockups = items.filter((i) => i.lockupViewModel?.contentType === 'LOCKUP_CONTENT_TYPE_VIDEO')
	const header = data.header ?? {}
	const headerKind = Object.keys(header)[0]
	console.log(
		`playlist ${PLAYLIST_ID}: header=${headerKind} playlistVideoRenderer=${legacy.length} lockupViewModel=${lockups.length} otherItems=${items.length - legacy.length - lockups.length}`
	)
	return legacy.length + lockups.length > 0 ? 'ok' : 'failed'
}

async function probeDiscogs() {
	const res = await fetch(`https://api.discogs.com/releases/${DISCOGS_RELEASE}`, { headers: { 'User-Agent': 'CrateApp/0.1' } })
	if (!res.ok) {
		console.log(`discogs release ${DISCOGS_RELEASE}: HTTP ${res.status}`)
		return 'failed'
	}
	const d = await res.json()
	const yt = (d.videos ?? []).filter((v) => /youtube\.com|youtu\.be/.test(v.uri ?? ''))
	console.log(
		`discogs release ${DISCOGS_RELEASE}: "${d.title}" tracks=${d.tracklist?.length ?? 0} youtubeVideos=${yt.length} ratelimit=${res.headers.get('x-discogs-ratelimit-remaining')}/${res.headers.get('x-discogs-ratelimit')}`
	)
	return d.tracklist?.length && yt.length ? 'ok' : 'failed'
}

const videos = process.argv.slice(2).length ? process.argv.slice(2) : DEFAULT_VIDEOS
const results = {}
try {
	const visitorData = await establishSession()
	console.log(`session: visitorData=${visitorData.slice(0, 24)}… cookies=${[...cookies.keys()].join(',')}`)
	for (const [i, id] of videos.entries()) {
		if (i > 0) await sleep(1500)
		console.log(`video ${id}`)
		results[id] = await probeVideo(id, visitorData)
	}
} catch (e) {
	console.log(`session bootstrap failed: ${e.message}`)
}
results.playlist = await probePlaylist()
results.discogs = await probeDiscogs()
console.log('\nsummary:', results)
const failed = Object.values(results).filter((r) => r === 'failed').length
const botChecks = Object.values(results).filter((r) => r === 'bot-check').length
if (failed) {
	console.log('RESULT: BROKEN — compare YT_CLIENTS with yt-dlp INNERTUBE_CLIENTS')
	process.exit(1)
}
if (botChecks === videos.length) {
	console.log('RESULT: INCONCLUSIVE — every video hit the bot check from this IP')
	process.exit(2)
}
console.log('RESULT: OK')
