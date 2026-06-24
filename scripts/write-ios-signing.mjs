#!/usr/bin/env node
/**
 * Configure manual distribution code-signing in the iOS Xcode project for CI release builds.
 *
 * Tauri uses the committed `project.pbxproj` as-is at `tauri ios build` — it does NOT regenerate it
 * from `project.yml` (a `project.yml` change to the signing settings had no effect). Tauri applies
 * the bundle id and switches on manual signing MODE, but it never selects a provisioning profile,
 * so a non-interactive CI archive fails with: "requires a provisioning profile. Select a
 * provisioning profile in the Signing & Capabilities editor." This rewrites the iOS target's
 * signing in the pbxproj to manual + Apple Distribution + the channel's named profile.
 *
 * CI-ONLY: intentionally NOT wired into the local `dev:ios` / `build:*:ios` scripts, so the
 * committed pbxproj keeps automatic/development signing and local on-device `yarn dev:ios` is
 * unaffected. The edit happens on the CI runner's checkout and is never committed.
 *
 * Usage: IOS_PROVISIONING_PROFILE_NAME="<profile name>" node scripts/write-ios-signing.mjs
 */
import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const pbxprojPath = resolve(repoRoot, 'src-tauri/gen/apple/crate-app.xcodeproj/project.pbxproj')

const SIGN_IDENTITY = 'Apple Distribution'

const argIdx = process.argv.indexOf('--profile-name')
const profileName = (
	argIdx !== -1 ? process.argv[argIdx + 1] : process.env.IOS_PROVISIONING_PROFILE_NAME || ''
).trim()
if (profileName === '') {
	console.error(
		'[write-ios-signing] no provisioning profile name — set IOS_PROVISIONING_PROFILE_NAME (or pass ' +
			'--profile-name). Cannot configure manual signing.',
	)
	process.exit(1)
}

let pbx
try {
	pbx = readFileSync(pbxprojPath, 'utf8')
} catch (err) {
	console.error(`[write-ios-signing] could not read ${pbxprojPath}: ${err.message}`)
	process.exit(1)
}

// The committed project signs automatically with a development identity (`iPhone Developer`).
// Replace that line in each target build config (debug + release) with manual Apple Distribution
// signing + the named profile, reusing the captured indentation. CI archives the release config;
// debug gets the same settings, harmlessly.
const pattern = /^([\t ]*)CODE_SIGN_IDENTITY = "iPhone Developer";$/gm
const count = (pbx.match(pattern) || []).length
if (count === 0) {
	console.error(
		'[write-ios-signing] `CODE_SIGN_IDENTITY = "iPhone Developer";` not found in pbxproj — the ' +
			'project structure changed; update this script.',
	)
	process.exit(1)
}
pbx = pbx.replace(
	pattern,
	(_match, indent) =>
		`${indent}CODE_SIGN_IDENTITY = "${SIGN_IDENTITY}";\n` +
		`${indent}CODE_SIGN_STYLE = Manual;\n` +
		`${indent}PROVISIONING_PROFILE_SPECIFIER = "${profileName}";`,
)

writeFileSync(pbxprojPath, pbx)
console.log(
	`[write-ios-signing] configured manual signing (${SIGN_IDENTITY}, profile "${profileName}") in ${count} build config(s)`,
)
