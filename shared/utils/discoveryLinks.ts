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

export function isSupportedDiscoveryUrl(input: string): boolean {
	const lower = input.toLowerCase()
	return (
		lower.includes('bandcamp.com') ||
		lower.includes('soundcloud.com') ||
		lower.includes('youtube.com') ||
		lower.includes('youtu.be') ||
		lower.includes('discogs.com')
	)
}

export function isDiscoveryPageUrl(input: string): boolean {
	const lower = input.toLowerCase()
	if (lower.includes('bandcamp.com') && !lower.includes('/album/') && !lower.includes('/track/')) {
		return true
	}
	if (lower.includes('discogs.com') && (lower.includes('/artist/') || lower.includes('/label/'))) {
		return true
	}
	return false
}

export function detectSourceType(url: string): DiscoverySourceType {
	const lower = url.toLowerCase()
	if (lower.includes('bandcamp.com')) return 'bandcamp'
	if (lower.includes('soundcloud.com')) return 'soundcloud'
	if (lower.includes('youtube.com') || lower.includes('youtu.be')) return 'youtube'
	if (lower.includes('discogs.com')) return 'discogs'
	return 'other'
}
