import type { DiscoveryRelease, DiscoveryTrack, Tag, TagCategory } from '../types'
import { formatDurationCompact } from './format'

export type DiscoveryExportShape = 'curated' | 'raw'
export type DiscoveryExportTrackDetail = 'all' | 'liked' | 'none'
export type DiscoveryExportScope = 'collection' | 'selection'

export interface DiscoveryExportOptions {
	shape: DiscoveryExportShape
	trackDetail: DiscoveryExportTrackDetail
	scope: DiscoveryExportScope
	/** ISO 8601 timestamp stamped into the envelope. */
	generatedAt: string
	appVersion?: string
}

export interface CuratedDiscoveryTrack {
	position: number
	name: string
	duration?: string
	liked: boolean
	url?: string
	tags?: string[]
}

export interface CuratedDiscoveryRelease {
	id: string
	artist?: string
	title?: string
	label?: string
	release_date?: string
	source: DiscoveryRelease['source_type']
	url: string
	added: string
	tags?: string[]
	notes?: string
	track_count: number
	liked_track_count: number
	/** Present only when `tracks` is a playlist-filtered subset of the release. */
	tracks_partial?: true
	tracks?: CuratedDiscoveryTrack[]
}

export interface DiscoveryExportEnvelope {
	schema: 'crate.discovery.export'
	schema_version: 1
	shape: DiscoveryExportShape
	scope: DiscoveryExportScope
	track_detail: DiscoveryExportTrackDetail
	generated_at: string
	app_version?: string
	release_count: number
	releases: CuratedDiscoveryRelease[] | DiscoveryRelease[]
}

const SCHEMA = 'crate.discovery.export'
const SCHEMA_VERSION = 1
const ESTIMATE_SAMPLE_SIZE = 25

type CategoryNameById = Map<string, string>

function categoryNames(categories: TagCategory[]): CategoryNameById {
	return new Map(categories.map((c) => [c.id, c.name]))
}

/**
 * Flattens tag objects to "Category: Name" so a reader needs no id table. A tag whose
 * category is unknown (e.g. a stale sync) falls back to its bare name.
 */
function flattenTags(tags: Tag[] | undefined, names: CategoryNameById): string[] | undefined {
	if (!tags || tags.length === 0) return undefined
	return tags.map((t) => {
		const category = names.get(t.category_id)
		return category ? `${category}: ${t.name}` : t.name
	})
}

function present(value: string | null | undefined): string | undefined {
	return value ? value : undefined
}

function selectTracks(tracks: DiscoveryTrack[], detail: DiscoveryExportTrackDetail): DiscoveryTrack[] {
	switch (detail) {
		case 'all':
			return tracks
		case 'liked':
			return tracks.filter((t) => t.is_liked)
		case 'none':
			return []
	}
}

function curateTrack(track: DiscoveryTrack, names: CategoryNameById): CuratedDiscoveryTrack {
	const url = present(track.url)
	const tags = flattenTags(track.tags, names)
	return {
		position: track.position,
		name: track.name,
		...(track.duration_ms != null ? { duration: formatDurationCompact(track.duration_ms) } : {}),
		liked: track.is_liked,
		...(url ? { url } : {}),
		...(tags ? { tags } : {}),
	}
}

function curateRelease(
	release: DiscoveryRelease,
	names: CategoryNameById,
	detail: DiscoveryExportTrackDetail
): CuratedDiscoveryRelease {
	const artist = present(release.artist)
	const title = present(release.title)
	const label = present(release.label)
	const releaseDate = present(release.release_date)
	const notes = present(release.notes)
	const tags = flattenTags(release.tags, names)
	const partial = release.total_track_count != null && release.total_track_count > release.tracks.length
	const tracks = selectTracks(release.tracks, detail)

	// Key order is chosen for a human reader: identity first, bookkeeping last.
	return {
		id: release.id,
		...(artist ? { artist } : {}),
		...(title ? { title } : {}),
		...(label ? { label } : {}),
		...(releaseDate ? { release_date: releaseDate } : {}),
		source: release.source_type,
		url: release.url,
		added: release.date_added.split('T')[0],
		...(tags ? { tags } : {}),
		...(notes ? { notes } : {}),
		track_count: release.total_track_count ?? release.tracks.length,
		liked_track_count: release.tracks.filter((t) => t.is_liked).length,
		...(partial ? { tracks_partial: true as const } : {}),
		...(tracks.length > 0 ? { tracks: tracks.map((t) => curateTrack(t, names)) } : {}),
	}
}

function rawRelease(release: DiscoveryRelease, detail: DiscoveryExportTrackDetail): DiscoveryRelease {
	return detail === 'all' ? release : { ...release, tracks: selectTracks(release.tracks, detail) }
}

function buildReleases(
	releases: DiscoveryRelease[],
	names: CategoryNameById,
	options: DiscoveryExportOptions
): CuratedDiscoveryRelease[] | DiscoveryRelease[] {
	return options.shape === 'raw'
		? releases.map((r) => rawRelease(r, options.trackDetail))
		: releases.map((r) => curateRelease(r, names, options.trackDetail))
}

function envelope(
	releases: CuratedDiscoveryRelease[] | DiscoveryRelease[],
	releaseCount: number,
	options: DiscoveryExportOptions
): DiscoveryExportEnvelope {
	return {
		schema: SCHEMA,
		schema_version: SCHEMA_VERSION,
		shape: options.shape,
		scope: options.scope,
		track_detail: options.trackDetail,
		generated_at: options.generatedAt,
		...(options.appVersion ? { app_version: options.appVersion } : {}),
		release_count: releaseCount,
		releases,
	}
}

export function buildDiscoveryExport(
	releases: DiscoveryRelease[],
	categories: TagCategory[],
	options: DiscoveryExportOptions
): DiscoveryExportEnvelope {
	return envelope(buildReleases(releases, categoryNames(categories), options), releases.length, options)
}

function utf8Bytes(value: unknown): number {
	return new TextEncoder().encode(JSON.stringify(value, null, 2)).length
}

/**
 * Approximate pretty-printed size without stringifying the whole set: an evenly strided
 * sample of releases is built and measured inside a real envelope (so indentation matches),
 * then scaled up. Cheap enough to run on every control change in the export dialog.
 */
export function estimateDiscoveryExportBytes(
	releases: DiscoveryRelease[],
	categories: TagCategory[],
	options: DiscoveryExportOptions
): number {
	const envelopeBytes = utf8Bytes(envelope([], releases.length, options))
	if (releases.length === 0) return envelopeBytes

	const sampleSize = Math.min(ESTIMATE_SAMPLE_SIZE, releases.length)
	const stride = releases.length / sampleSize
	const sample: DiscoveryRelease[] = []
	for (let i = 0; i < sampleSize; i++) sample.push(releases[Math.floor(i * stride)])

	const sampleReleases = buildReleases(sample, categoryNames(categories), options)
	const sampleBytes = utf8Bytes(envelope(sampleReleases, releases.length, options)) - envelopeBytes
	return Math.round(envelopeBytes + (sampleBytes * releases.length) / sample.length)
}

export function buildDiscoveryExportFilename(scope: DiscoveryExportScope, now: Date = new Date()): string {
	const date = now.toISOString().split('T')[0]
	return scope === 'selection' ? `crate-discovery-selection-${date}.json` : `crate-discovery-${date}.json`
}
