#!/usr/bin/env node
/**
 * Changelog management script for Crate
 *
 * Usage:
 *   node scripts/changelog.js prepare <version>   # Move Unreleased to version section
 *   node scripts/changelog.js graduate <version>  # Consolidate prereleases to stable
 */

import { readFileSync, writeFileSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const ROOT = join(__dirname, '..')
const CHANGELOG_PATH = join(ROOT, 'CHANGELOG.md')

const CATEGORIES = ['Added', 'Changed', 'Deprecated', 'Removed', 'Fixed', 'Security']

function getToday() {
	const now = new Date()
	return now.toISOString().split('T')[0]
}

function getBaseVersion(version) {
	return version.replace(/-.*$/, '')
}

function parseChangelog(content) {
	const lines = content.split('\n')
	const sections = []
	let current = null
	let currentCategory = null
	let headerLines = []
	let inHeader = true

	for (const line of lines) {
		// Match version headers like ## [0.1.0] or ## [Unreleased]
		const versionMatch = line.match(/^## \[([^\]]+)\](?:\s*-\s*(\d{4}-\d{2}-\d{2}))?/)
		if (versionMatch) {
			inHeader = false
			if (current) sections.push(current)
			current = {
				version: versionMatch[1],
				date: versionMatch[2] || null,
				categories: {},
				rawHeader: line,
			}
			currentCategory = null
			continue
		}

		if (inHeader) {
			headerLines.push(line)
			continue
		}

		if (current) {
			// Match category headers like ### Added
			const categoryMatch = line.match(/^### (Added|Changed|Deprecated|Removed|Fixed|Security)/)
			if (categoryMatch) {
				currentCategory = categoryMatch[1]
				if (!current.categories[currentCategory]) {
					current.categories[currentCategory] = []
				}
				continue
			}

			// Match list items
			if (line.startsWith('- ') && currentCategory) {
				current.categories[currentCategory].push(line)
			}
		}
	}

	if (current) sections.push(current)

	return { headerLines, sections }
}

function formatSection(section) {
	const lines = [section.rawHeader || `## [${section.version}]${section.date ? ` - ${section.date}` : ''}`]
	lines.push('')

	for (const category of CATEGORIES) {
		if (section.categories[category] && section.categories[category].length > 0) {
			lines.push(`### ${category}`)
			lines.push('')
			for (const item of section.categories[category]) {
				lines.push(item)
			}
			lines.push('')
		}
	}

	return lines.join('\n')
}

function formatChangelog(headerLines, sections, footerLinks) {
	const parts = []

	// Header
	parts.push(headerLines.join('\n'))

	// Sections
	for (const section of sections) {
		parts.push(formatSection(section))
	}

	// Footer links
	if (footerLinks) {
		parts.push(footerLinks)
	}

	return (
		parts
			.join('\n')
			.replace(/\n{3,}/g, '\n\n')
			.trim() + '\n'
	)
}

function extractFooterLinks(content) {
	const match = content.match(/(\n\[Unreleased\]:[\s\S]*$)/)
	return match ? match[1].trim() : null
}

function updateFooterLinks(footerLinks, version, repoUrl) {
	if (!footerLinks) {
		return `[Unreleased]: ${repoUrl}/compare/v${version}...HEAD\n[${version}]: ${repoUrl}/releases/tag/v${version}`
	}

	let lines = footerLinks.split('\n')

	// Find the previous version from the Unreleased link
	const unreleasedMatch = lines[0].match(/compare\/v([^.]+\.[^.]+\.[^.]+(?:-[^.]+\.\d+)?)\.\.\.HEAD/)
	const prevVersion = unreleasedMatch ? unreleasedMatch[1] : null

	// Update Unreleased link to compare against new version
	lines[0] = `[Unreleased]: ${repoUrl}/compare/v${version}...HEAD`

	// Add new version link
	const newLink = prevVersion
		? `[${version}]: ${repoUrl}/compare/v${prevVersion}...v${version}`
		: `[${version}]: ${repoUrl}/releases/tag/v${version}`

	lines.splice(1, 0, newLink)

	return lines.join('\n')
}

function prepareRelease(version) {
	const content = readFileSync(CHANGELOG_PATH, 'utf-8')
	const { headerLines, sections } = parseChangelog(content)
	let footerLinks = extractFooterLinks(content)

	// Find Unreleased section
	const unreleasedIndex = sections.findIndex((s) => s.version === 'Unreleased')
	if (unreleasedIndex === -1) {
		throw new Error('No [Unreleased] section found in CHANGELOG.md')
	}

	const unreleased = sections[unreleasedIndex]

	// Check if there's content to release
	const hasContent = Object.values(unreleased.categories).some((items) => items.length > 0)
	if (!hasContent) {
		throw new Error('No changes in [Unreleased] section to release')
	}

	// Create new version section
	const newSection = {
		version,
		date: getToday(),
		categories: { ...unreleased.categories },
		rawHeader: `## [${version}] - ${getToday()}`,
	}

	// Clear Unreleased section
	unreleased.categories = {}
	unreleased.rawHeader = '## [Unreleased]'

	// Insert new section after Unreleased
	sections.splice(unreleasedIndex + 1, 0, newSection)

	// Update footer links
	const repoUrl = 'https://github.com/blackboxaudio/crate'
	footerLinks = updateFooterLinks(footerLinks, version, repoUrl)

	// Write updated changelog
	const newContent = formatChangelog(headerLines, sections, footerLinks)
	writeFileSync(CHANGELOG_PATH, newContent)

	console.log(`Prepared changelog for version ${version}`)
}

function graduateRelease(stableVersion) {
	const content = readFileSync(CHANGELOG_PATH, 'utf-8')
	const { headerLines, sections } = parseChangelog(content)
	let footerLinks = extractFooterLinks(content)

	const baseVersion = getBaseVersion(stableVersion)

	// Find all prerelease sections for this base version
	const prereleasePattern = new RegExp(`^${baseVersion.replace(/\./g, '\\.')}-`)
	const prereleaseIndices = []
	const consolidatedCategories = {}

	for (let i = 0; i < sections.length; i++) {
		if (prereleasePattern.test(sections[i].version)) {
			prereleaseIndices.push(i)

			// Merge categories (oldest first, so items appear in chronological order)
			for (const [category, items] of Object.entries(sections[i].categories)) {
				if (!consolidatedCategories[category]) {
					consolidatedCategories[category] = []
				}
				// Add items that aren't already present (dedupe)
				for (const item of items) {
					if (!consolidatedCategories[category].includes(item)) {
						consolidatedCategories[category].push(item)
					}
				}
			}
		}
	}

	if (prereleaseIndices.length === 0) {
		throw new Error(`No prerelease sections found for ${baseVersion}`)
	}

	// Create stable version section
	const stableSection = {
		version: stableVersion,
		date: getToday(),
		categories: consolidatedCategories,
		rawHeader: `## [${stableVersion}] - ${getToday()}`,
	}

	// Remove prerelease sections (in reverse order to maintain indices)
	for (let i = prereleaseIndices.length - 1; i >= 0; i--) {
		sections.splice(prereleaseIndices[i], 1)
	}

	// Find Unreleased section and insert after it
	const unreleasedIndex = sections.findIndex((s) => s.version === 'Unreleased')
	sections.splice(unreleasedIndex + 1, 0, stableSection)

	// Update footer links - remove prerelease links, add stable link
	if (footerLinks) {
		const lines = footerLinks
			.split('\n')
			.filter((line) => !prereleasePattern.test(line.match(/^\[([^\]]+)\]/)?.[1] || ''))
		footerLinks = lines.join('\n')
	}

	const repoUrl = 'https://github.com/blackboxaudio/crate'
	footerLinks = updateFooterLinks(footerLinks, stableVersion, repoUrl)

	// Write updated changelog
	const newContent = formatChangelog(headerLines, sections, footerLinks)
	writeFileSync(CHANGELOG_PATH, newContent)

	console.log(`Graduated ${prereleaseIndices.length} prerelease(s) to ${stableVersion}`)
}

function printUsage() {
	console.error(`
Usage:
  yarn changelog:prepare <version>   # Move Unreleased content to version section
  yarn changelog:graduate <version>  # Consolidate prereleases to stable version

Examples:
  yarn changelog:prepare 0.2.0-staging.1   # Create staging release entry
  yarn changelog:graduate 0.2.0            # Consolidate all 0.2.0-staging.* entries to 0.2.0
`)
}

const command = process.argv[2]
const version = process.argv[3]

if (!command || !version) {
	printUsage()
	process.exit(1)
}

try {
	switch (command) {
		case 'prepare':
			prepareRelease(version)
			break
		case 'graduate':
			graduateRelease(version)
			break
		default:
			console.error(`Unknown command: ${command}`)
			printUsage()
			process.exit(1)
	}
} catch (error) {
	console.error(`Error: ${error.message}`)
	process.exit(1)
}
