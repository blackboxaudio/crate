#!/usr/bin/env node
/**
 * Stamp the iOS app's entitlements for the given release channel, controlling the **App Attest**
 * (Firebase App Check, #139) entitlement per channel:
 *   dev     -> no App Attest entitlement (uses an App Check *debug token* instead; works on device
 *              and simulator with no attestation/provisioning setup, so `yarn dev:ios` signing is
 *              unchanged)
 *   staging -> com.apple.developer.devicecheck.appattest-environment = production  (TestFlight)
 *   prod    -> com.apple.developer.devicecheck.appattest-environment = production  (App Store)
 *
 * TestFlight and App Store builds BOTH use the `production` App Attest environment (only a build
 * run/installed directly from Xcode uses `development`), so staging and prod are identical here.
 *
 * Why a script: `src-tauri/gen/apple/crate-app_iOS/crate-app_iOS.entitlements` is the committed
 * XcodeGen entitlements file the iOS build consumes, and it is NOT channel-aware on its own. CI
 * builds iOS without passing a Tauri `--config`, so the entitlements come purely from this file.
 * Mirrors `write-ios-bundle-id.mjs` / `write-ios-plist.mjs`, run before `tauri ios dev|build`.
 *
 * The committed default stays `dev` (no App Attest entitlement), so a bare build / fresh clone
 * signs unchanged; staging/prod builds run this first. Idempotent — it rewrites the whole `<dict>`
 * body regardless of the file's prior channel, preserving the explanatory comment above `<plist>`.
 *
 * NOTE: `production` App Attest requires the channel's App ID to permit App Attest and a matching
 * provisioning profile (usually added automatically by App Store Connect signing). Validate on
 * staging/TestFlight before prod.
 *
 * Usage: node scripts/write-ios-entitlements.mjs --channel <dev|staging|prod>
 */
import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const entitlementsPath = resolve(repoRoot, 'src-tauri/gen/apple/crate-app_iOS/crate-app_iOS.entitlements')

// Per-channel App Attest environment (null = omit the entitlement entirely).
const CHANNELS = {
	dev: { appAttestEnv: null },
	staging: { appAttestEnv: 'production' },
	prod: { appAttestEnv: 'production' },
}

const channelArgIndex = process.argv.indexOf('--channel')
const channel = channelArgIndex !== -1 ? process.argv[channelArgIndex + 1] : undefined
const target = channel && CHANNELS[channel]
if (!target) {
	console.error(
		`[write-ios-entitlements] missing or invalid --channel (expected one of: ${Object.keys(CHANNELS).join(', ')})`,
	)
	process.exit(1)
}

// The explanatory comment documenting the App Attest entitlement, kept above the <dict> so the file
// stays self-describing whichever channel it's currently stamped for.
const comment = `<!--
  App Check / App Attest (#139): the App Attest entitlement is stamped per channel by
  scripts/write-ios-entitlements.mjs (run before \`tauri ios dev|build\`). The dev channel omits it
  (App Check uses a debug token there); staging/prod use the \`production\` App Attest environment.
  App Attest is a restricted entitlement — \`production\` needs the channel's App ID to permit App
  Attest and a matching provisioning profile, or code signing fails. Edit CHANNELS in the script,
  not this generated file.
-->`

const dictBody =
	target.appAttestEnv === null
		? '<dict/>'
		: `<dict>
	<key>com.apple.developer.devicecheck.appattest-environment</key>
	<string>${target.appAttestEnv}</string>
</dict>`

const entitlements = `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
${comment}
<plist version="1.0">
${dictBody}
</plist>
`

// Sanity-check the target exists (fail loud if the generated project layout moved).
try {
	readFileSync(entitlementsPath, 'utf8')
} catch (err) {
	console.error(`[write-ios-entitlements] could not read ${entitlementsPath}: ${err.message}`)
	process.exit(1)
}

writeFileSync(entitlementsPath, entitlements)
console.log(
	`[write-ios-entitlements] channel "${channel}": App Attest environment ${
		target.appAttestEnv === null ? 'omitted (debug-token dev)' : `"${target.appAttestEnv}"`
	}`,
)
