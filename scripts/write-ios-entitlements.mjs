#!/usr/bin/env node
/**
 * Stamp the iOS app's entitlements for the given release channel. Every channel gets **Sign in with
 * Apple** (`com.apple.developer.applesignin`, App Store Guideline 4.8 — the native sign-in flow);
 * the **App Attest** (Firebase App Check, #139) entitlement is per channel:
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

// Per-channel App Attest environment (null = omit the App Attest entitlement entirely). Sign in
// with Apple is added for every channel regardless (see the dict assembly below).
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

// The explanatory comment documenting the entitlements, kept above the <dict> so the file stays
// self-describing whichever channel it's currently stamped for.
const comment = `<!--
  iOS entitlements, stamped per release channel by scripts/write-ios-entitlements.mjs (run before
  \`tauri ios dev|build\`). Edit CHANNELS / the script, NOT this generated file.

  - Sign in with Apple (com.apple.developer.applesignin): present on EVERY channel (App Store
    Guideline 4.8, the native sign-in flow). The App ID for each channel must enable the
    "Sign in with Apple" capability, or code signing fails.
  - App Check / App Attest (#139): the dev channel omits it (App Check uses a debug token there);
    staging/prod use the \`production\` App Attest environment, which needs the channel's App ID to
    permit App Attest and a matching provisioning profile.
-->`

// Sign in with Apple is required on every channel; App Attest is added for staging/prod only.
const entries = [
	'\t<key>com.apple.developer.applesignin</key>\n\t<array>\n\t\t<string>Default</string>\n\t</array>',
]
if (target.appAttestEnv !== null) {
	entries.push(
		`\t<key>com.apple.developer.devicecheck.appattest-environment</key>\n\t<string>${target.appAttestEnv}</string>`,
	)
}
const dictBody = `<dict>\n${entries.join('\n')}\n</dict>`

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
	`[write-ios-entitlements] channel "${channel}": Sign in with Apple enabled; App Attest ${
		target.appAttestEnv === null ? 'omitted (debug-token dev)' : `"${target.appAttestEnv}"`
	}`,
)
