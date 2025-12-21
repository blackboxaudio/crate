#!/usr/bin/env node
/**
 * Version bump script for Crate
 * Synchronizes version across package.json, Cargo.toml, and tauri.conf.json
 *
 * Usage:
 *   node scripts/version.js <major|minor|patch>              # Standard version bump
 *   node scripts/version.js <major|minor|patch> <alpha|beta> # Start prerelease
 *   node scripts/version.js prerelease                       # Increment prerelease number
 *   node scripts/version.js stage                            # Promote to next channel
 */

import { readFileSync, writeFileSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const ROOT = join(__dirname, '..')

const STANDARD_BUMPS = ['major', 'minor', 'patch']
const PRERELEASE_CHANNELS = ['alpha', 'beta']

function parseVersion(version) {
	const match = version.match(/^(\d+)\.(\d+)\.(\d+)(?:-([a-zA-Z]+)\.(\d+))?$/)
	if (!match) {
		throw new Error(`Invalid version format: ${version}`)
	}
	return {
		major: parseInt(match[1], 10),
		minor: parseInt(match[2], 10),
		patch: parseInt(match[3], 10),
		channel: match[4] || null,
		prerelease: match[5] ? parseInt(match[5], 10) : null,
	}
}

function formatVersion({ major, minor, patch, channel, prerelease }) {
	const base = `${major}.${minor}.${patch}`
	if (channel && prerelease !== null) {
		return `${base}-${channel}.${prerelease}`
	}
	return base
}

function bumpBase(parsed, type) {
	switch (type) {
		case 'major':
			return { ...parsed, major: parsed.major + 1, minor: 0, patch: 0 }
		case 'minor':
			return { ...parsed, minor: parsed.minor + 1, patch: 0 }
		case 'patch':
			return { ...parsed, patch: parsed.patch + 1 }
		default:
			throw new Error(`Invalid bump type: ${type}`)
	}
}

function bumpVersion(version, bumpType, channel = null) {
	const parsed = parseVersion(version)

	// Standard bumps (major, minor, patch)
	if (STANDARD_BUMPS.includes(bumpType)) {
		if (parsed.channel) {
			throw new Error(
				`Cannot use '${bumpType}' on prerelease version ${version}. Use 'stage' to promote to next channel.`
			)
		}

		// If channel specified, start a prerelease
		if (channel) {
			if (!PRERELEASE_CHANNELS.includes(channel)) {
				throw new Error(`Invalid channel: ${channel}. Use: ${PRERELEASE_CHANNELS.join(', ')}`)
			}
			const bumped = bumpBase(parsed, bumpType)
			return formatVersion({ ...bumped, channel, prerelease: 1 })
		}

		// Standard bump to stable
		const bumped = bumpBase(parsed, bumpType)
		return formatVersion({ ...bumped, channel: null, prerelease: null })
	}

	// Prerelease increment
	if (bumpType === 'prerelease') {
		if (!parsed.channel) {
			throw new Error(`Not on prerelease. Use 'minor alpha' or 'minor beta' to start a prerelease.`)
		}
		return formatVersion({ ...parsed, prerelease: parsed.prerelease + 1 })
	}

	// Stage: auto-promote to next channel
	if (bumpType === 'stage') {
		if (!parsed.channel) {
			throw new Error(`Not on prerelease. Use 'minor alpha' or 'minor beta' to start a prerelease.`)
		}

		const currentIndex = PRERELEASE_CHANNELS.indexOf(parsed.channel)

		// If on last channel (beta), promote to stable
		if (currentIndex === PRERELEASE_CHANNELS.length - 1) {
			return formatVersion({ ...parsed, channel: null, prerelease: null })
		}

		// Promote to next channel
		const nextChannel = PRERELEASE_CHANNELS[currentIndex + 1]
		return formatVersion({ ...parsed, channel: nextChannel, prerelease: 1 })
	}

	throw new Error(`Invalid bump type: ${bumpType}`)
}

function updateVersion(bumpType, channel = null) {
	// Validate bump type
	const validTypes = [...STANDARD_BUMPS, 'prerelease', 'stage']
	if (!validTypes.includes(bumpType)) {
		console.error(`Invalid bump type: ${bumpType}`)
		printUsage()
		process.exit(1)
	}

	// Read current version from package.json
	const packageJsonPath = join(ROOT, 'package.json')
	const packageJson = JSON.parse(readFileSync(packageJsonPath, 'utf-8'))
	const oldVersion = packageJson.version
	const newVersion = bumpVersion(oldVersion, bumpType, channel)

	// Update package.json
	packageJson.version = newVersion
	writeFileSync(packageJsonPath, JSON.stringify(packageJson, null, '\t') + '\n')
	console.log(`Updated package.json: ${oldVersion} -> ${newVersion}`)

	// Update Cargo.toml (regex handles optional prerelease suffix)
	const cargoTomlPath = join(ROOT, 'src-tauri', 'Cargo.toml')
	let cargoToml = readFileSync(cargoTomlPath, 'utf-8')
	cargoToml = cargoToml.replace(/^version = "[\d.]+(?:-[a-zA-Z]+\.\d+)?"$/m, `version = "${newVersion}"`)
	writeFileSync(cargoTomlPath, cargoToml)
	console.log(`Updated src-tauri/Cargo.toml: ${oldVersion} -> ${newVersion}`)

	// Update tauri.conf.json
	const tauriConfPath = join(ROOT, 'src-tauri', 'tauri.conf.json')
	const tauriConf = JSON.parse(readFileSync(tauriConfPath, 'utf-8'))
	tauriConf.version = newVersion
	writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, '  ') + '\n')
	console.log(`Updated src-tauri/tauri.conf.json: ${oldVersion} -> ${newVersion}`)

	console.log(`\nVersion bumped from ${oldVersion} to ${newVersion}`)
}

function printUsage() {
	console.error(`
Usage:
  yarn bump <major|minor|patch>              # Standard version bump
  yarn bump <major|minor|patch> <alpha|beta> # Start prerelease
  yarn bump prerelease                       # Increment prerelease number
  yarn bump stage                            # Promote to next channel (alpha -> beta -> stable)

Examples:
  yarn bump minor                # 0.1.0 -> 0.2.0
  yarn bump minor alpha          # 0.1.0 -> 0.2.0-alpha.1
  yarn bump prerelease           # 0.2.0-alpha.1 -> 0.2.0-alpha.2
  yarn bump stage                # 0.2.0-alpha.2 -> 0.2.0-beta.1
  yarn bump stage                # 0.2.0-beta.1 -> 0.2.0
`)
}

const bumpType = process.argv[2]
const channel = process.argv[3]

if (!bumpType) {
	printUsage()
	process.exit(1)
}

try {
	updateVersion(bumpType, channel)
} catch (error) {
	console.error(`Error: ${error.message}`)
	process.exit(1)
}
