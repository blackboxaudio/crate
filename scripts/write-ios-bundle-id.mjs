#!/usr/bin/env node
/**
 * Set the iOS app's bundle identifier (and product name) in `src-tauri/gen/apple/project.yml` for
 * the given release channel, so each channel ships a distinct bundle that matches its App Store
 * Connect app / provisioning profile / Firebase app:
 *   dev     -> com.bbx-audio.crate.dev      ("Crate Dev")     — local `yarn dev:ios` only
 *   staging -> com.bbx-audio.crate.staging  ("Crate Staging") — TestFlight
 *   prod    -> com.bbx-audio.crate          ("Crate")         — App Store
 *
 * Why a script: the committed `project.yml` is the XcodeGen source the iOS build consumes, and it
 * is NOT channel-aware on its own. CI builds iOS without passing a Tauri `--config`, so the bundle
 * id comes purely from this file — the per-channel Tauri configs (`tauri.<channel>.conf.json`) carry
 * the matching identifiers but don't reach the iOS build. This keeps iOS in sync with them. Mirrors
 * `write-mobile-icons.mjs` / `write-ios-plist.mjs`, run before `tauri ios dev|build`.
 *
 * The committed default stays `dev`, so a bare build / fresh clone is the dev bundle; staging/prod
 * builds run this first. Idempotent — it rewrites whatever `com.bbx-audio.crate*` value is present.
 *
 * Usage: node scripts/write-ios-bundle-id.mjs --channel <dev|staging|prod>
 */
import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const projectYmlPath = resolve(repoRoot, 'src-tauri/gen/apple/project.yml')

const CHANNELS = {
	dev: { bundleId: 'com.bbx-audio.crate.dev', productName: 'Crate Dev' },
	staging: { bundleId: 'com.bbx-audio.crate.staging', productName: 'Crate Staging' },
	prod: { bundleId: 'com.bbx-audio.crate', productName: 'Crate' },
}

const channelArgIndex = process.argv.indexOf('--channel')
const channel = channelArgIndex !== -1 ? process.argv[channelArgIndex + 1] : undefined
const target = channel && CHANNELS[channel]
if (!target) {
	console.error(
		`[write-ios-bundle-id] missing or invalid --channel (expected one of: ${Object.keys(CHANNELS).join(', ')})`,
	)
	process.exit(1)
}

let yml
try {
	yml = readFileSync(projectYmlPath, 'utf8')
} catch (err) {
	console.error(`[write-ios-bundle-id] could not read ${projectYmlPath}: ${err.message}`)
	process.exit(1)
}

// Replace each key's value, preserving the existing indentation. The bundle-id lines match any
// current `com.bbx-audio.crate*` value so the script is idempotent across repeated channel runs.
const replacements = [
	[/^([ \t]*bundleIdPrefix:[ \t]*).*$/m, `$1${target.bundleId}`],
	[/^([ \t]*PRODUCT_BUNDLE_IDENTIFIER:[ \t]*).*$/m, `$1${target.bundleId}`],
	[/^([ \t]*PRODUCT_NAME:[ \t]*).*$/m, `$1${target.productName}`],
]

for (const [pattern, replacement] of replacements) {
	if (!pattern.test(yml)) {
		console.error(`[write-ios-bundle-id] expected key not found in project.yml: ${pattern}`)
		process.exit(1)
	}
	yml = yml.replace(pattern, replacement)
}

writeFileSync(projectYmlPath, yml)
console.log(
	`[write-ios-bundle-id] set iOS bundle id to ${target.bundleId} ("${target.productName}") for channel "${channel}"`,
)
