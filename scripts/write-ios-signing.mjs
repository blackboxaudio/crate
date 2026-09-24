#!/usr/bin/env node
/**
 * Configure manual distribution code-signing in the iOS Xcode project for CI release builds.
 *
 * Tauri uses the committed `project.pbxproj` as-is at `tauri ios build` — it does NOT regenerate it
 * from `project.yml` (a `project.yml` signing change has no effect). Tauri applies the bundle id and
 * switches manual-signing MODE on, but never selects a provisioning profile, so a non-interactive CI
 * archive fails with: "requires a provisioning profile. Select a provisioning profile in the Signing
 * & Capabilities editor." This rewrites the iOS target's signing to manual + Apple Distribution + the
 * channel's named profile.
 *
 * Robust to whichever automatic/development signing the committed pbxproj carries. Opening the
 * project in Xcode and touching signing rewrites this block — e.g. `CODE_SIGN_IDENTITY` flips from
 * "iPhone Developer" to "Apple Development", and `CODE_SIGN_STYLE = Automatic` /
 * `PROVISIONING_PROFILE_SPECIFIER = ""` get added — so this matches ANY identity value, forces
 * `CODE_SIGN_STYLE = Manual`, points the specifier at the profile, and injects the style/specifier
 * keys if an older single-line structure lacks them. `DEVELOPMENT_TEAM` is left untouched (the
 * distribution team is the same one the committed project already signs with).
 *
 * CI-ONLY: intentionally NOT wired into the local `dev:ios` / `build:*:ios` scripts, so the committed
 * pbxproj keeps automatic/development signing and local on-device `yarn dev:ios` is unaffected. The
 * edit happens on the CI runner's checkout and is never committed. Idempotent.
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

// Anchor on CODE_SIGN_IDENTITY — always present, once per build config — matching whatever value the
// committed project carries ("Apple Development", "iPhone Developer", …). It also captures the
// indentation used to inject the style/specifier keys when an older pbxproj structure lacks them.
const identityPattern = /^([\t ]*)CODE_SIGN_IDENTITY = "[^"]*";$/gm
const identityCount = (pbx.match(identityPattern) || []).length
if (identityCount === 0) {
	console.error(
		'[write-ios-signing] no `CODE_SIGN_IDENTITY = "...";` line in pbxproj — the project structure ' +
			'changed; update this script.',
	)
	process.exit(1)
}

// Detect the current structure BEFORE mutating (Xcode-written projects already carry these keys;
// older single-line ones don't and need them injected next to the identity line).
const hadStyle = /^[\t ]*CODE_SIGN_STYLE = \w+;$/m.test(pbx)
const hadSpecifier = /^[\t ]*PROVISIONING_PROFILE_SPECIFIER = "[^"]*";$/m.test(pbx)

// 1. Identity -> Apple Distribution (+ inject Manual style / profile specifier iff the structure
//    doesn't already have them, so we never create duplicate keys).
pbx = pbx.replace(identityPattern, (_match, indent) => {
	let out = `${indent}CODE_SIGN_IDENTITY = "${SIGN_IDENTITY}";`
	if (!hadStyle) out += `\n${indent}CODE_SIGN_STYLE = Manual;`
	if (!hadSpecifier) out += `\n${indent}PROVISIONING_PROFILE_SPECIFIER = "${profileName}";`
	return out
})

// 2. Force any existing CODE_SIGN_STYLE (Xcode writes `Automatic`) to Manual.
if (hadStyle) {
	pbx = pbx.replace(/^([\t ]*)CODE_SIGN_STYLE = \w+;$/gm, `$1CODE_SIGN_STYLE = Manual;`)
}

// 3. Point any existing PROVISIONING_PROFILE_SPECIFIER (Xcode writes an empty string) at the profile.
if (hadSpecifier) {
	pbx = pbx.replace(
		/^([\t ]*)PROVISIONING_PROFILE_SPECIFIER = "[^"]*";$/gm,
		`$1PROVISIONING_PROFILE_SPECIFIER = "${profileName}";`,
	)
}

writeFileSync(pbxprojPath, pbx)
console.log(
	`[write-ios-signing] configured manual signing (${SIGN_IDENTITY}, profile "${profileName}") in ${identityCount} build config(s)`,
)
