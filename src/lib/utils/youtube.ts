/**
 * Build a YouTube search URL from the given query parts.
 *
 * Falsy parts (null, undefined, empty strings) are skipped, then the remaining
 * parts are space-joined and URL-encoded. Used by the discovery context menus to
 * search for a track ({artist} {track name}) or a release ({artist} {title}).
 */
export function buildYouTubeSearchUrl(...parts: (string | null | undefined)[]): string {
	const query = parts.filter(Boolean).join(' ')
	return `https://www.youtube.com/results?search_query=${encodeURIComponent(query)}`
}
