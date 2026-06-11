import type { DiscoveryRelease } from '../types'

/**
 * The artist page for a release — its own Bandcamp subdomain or SoundCloud profile
 * (where the release itself lives). Always derivable for those platforms.
 */
export function deriveArtistUrl(release: Pick<DiscoveryRelease, 'url' | 'source_type'>): string | null {
	try {
		const u = new URL(release.url)
		if (release.source_type === 'bandcamp') return `${u.protocol}//${u.host}`
		if (release.source_type === 'soundcloud') {
			const seg = u.pathname.split('/').filter(Boolean)[0]
			return seg ? `${u.protocol}//${u.host}/${seg}` : null
		}
	} catch {
		return null
	}
	return null
}

/**
 * The label/catalog page a release was discovered from (`source_page_url`), when it's a
 * page distinct from the artist's own. Only known for releases imported from a label page
 * — a label's URL can't be recovered from a release otherwise. Null when there's none.
 */
export function deriveLabelUrl(
	release: Pick<DiscoveryRelease, 'url' | 'source_page_url' | 'source_type'>
): string | null {
	const page = release.source_page_url
	if (!page) return null
	const artist = deriveArtistUrl(release)
	if (artist && looseUrlEq(page, artist)) return null
	return page
}

/** True when a release's "artist" is a Various-Artists placeholder — i.e. a label
 *  compilation rather than a single artist's release. Mirrors the backend `is_compilation`. */
export function isCompilation(artist: string | null | undefined): boolean {
	if (!artist) return false
	return ['various artists', 'various', 'v.a.', 'v/a', 'va'].includes(artist.trim().toLowerCase())
}

/** Compare two URLs ignoring case and trailing slashes (loose follow-match). */
export function looseUrlEq(a: string, b: string): boolean {
	const n = (s: string) => s.toLowerCase().replace(/\/+$/, '')
	return n(a) === n(b)
}
