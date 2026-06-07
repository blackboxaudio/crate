import type { DiscoveryRelease } from '$lib/types'

/**
 * Best-effort artist/label page URL for a release, for one-click "follow this source".
 * Bandcamp → the artist/label subdomain root; SoundCloud → the profile; anything with a
 * `parent_url` (e.g. Discogs releases scanned from an artist/label page) → that page.
 * Returns null when no followable page can be derived (use the paste-URL flow instead).
 */
export function deriveFollowUrl(release: Pick<DiscoveryRelease, 'url' | 'parent_url' | 'source_type'>): string | null {
	if (release.parent_url) return release.parent_url
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

/** Compare two URLs ignoring case and trailing slashes (loose follow-match). */
export function looseUrlEq(a: string, b: string): boolean {
	const n = (s: string) => s.toLowerCase().replace(/\/+$/, '')
	return n(a) === n(b)
}
