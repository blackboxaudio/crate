import type { DiscoverySourceType } from '../types'

/**
 * Human-facing platform name for a discovery release's source, used to label the "Open in {app}"
 * action on mobile. The button still opens the canonical https `release.url`, which the OS routes
 * into the installed native Bandcamp/SoundCloud/YouTube/Discogs app via Universal Links (iOS) /
 * App Links (Android), falling back to the browser. Returns null for 'other'/unknown sources, where
 * a generic "Open in Browser" label is used instead.
 */
export function getReleasePlatformName(sourceType: DiscoverySourceType): string | null {
	switch (sourceType) {
		case 'bandcamp':
			return 'Bandcamp'
		case 'soundcloud':
			return 'SoundCloud'
		case 'youtube':
			return 'YouTube'
		case 'discogs':
			return 'Discogs'
		default:
			return null
	}
}
