#!/usr/bin/env node
/**
 * Side-load a freshly-built iOS app onto a connected, trusted iPhone — no Xcode UI, no App Store
 * Connect. This is the final step of `yarn build:staging:ios` (and any other `build:*:ios`) so a
 * single command produces a signed `.ipa` and installs it for untethered, on-the-go testing.
 *
 * Why this works without TestFlight: the `build:*:ios` scripts export a development-signed IPA
 * (`ExportOptions.plist` method `debugging`) under the paid developer team. On a registered device
 * that build runs untethered — survives reboots, needs no cable to launch — until the provisioning
 * profile expires (~1 year). The device is registered automatically the first time you run
 * `yarn dev:ios` against it, so a phone you've already dev-run on needs no extra setup here.
 *
 * The device is resolved automatically via `xcrun devicectl` (ships with Xcode 15+); the only
 * prerequisite is that the iPhone is plugged in over USB, unlocked, and has trusted this Mac.
 *
 * Channel resolution mirrors `write-mobile-icons.mjs`: `--channel <dev|staging|prod>` wins, else
 * `CRATE_ENV`, else `prod`. The channel picks the matching `tauri.<env>.conf.json`, whose
 * `productName` is also the name Tauri gives the exported `.ipa` (e.g. "Crate Staging.ipa").
 *
 * Usage:
 *   node scripts/install-ios-device.mjs [--channel dev|staging|prod]
 */
import { execFileSync } from 'node:child_process'
import { existsSync, mkdtempSync, readFileSync, readdirSync, statSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const buildDir = resolve(repoRoot, 'src-tauri/gen/apple/build')

/** Read the value following a `--flag` in argv, or null. */
function getArg(name) {
	const i = process.argv.indexOf(name)
	return i !== -1 && i + 1 < process.argv.length ? process.argv[i + 1] : null
}

function fail(message) {
	console.error(`[install-ios-device] ${message}`)
	process.exit(1)
}

// Accept both the short folder names and the CRATE_ENV names, so callers can pass either.
const CHANNEL_BY_KEY = {
	dev: 'dev',
	development: 'dev',
	staging: 'staging',
	prod: 'prod',
	production: 'prod',
}

const CONFIG_BY_CHANNEL = {
	dev: 'tauri.dev.conf.json',
	staging: 'tauri.staging.conf.json',
	prod: 'tauri.prod.conf.json',
}

/** Resolve the channel: explicit `--channel`, else CRATE_ENV, else `prod`. */
function resolveChannel() {
	const fromArg = getArg('--channel')
	if (fromArg) {
		const channel = CHANNEL_BY_KEY[fromArg.toLowerCase()]
		if (!channel) fail(`unknown --channel "${fromArg}" (expected dev|staging|prod)`)
		return channel
	}
	const fromEnv = process.env.CRATE_ENV
	if (fromEnv && CHANNEL_BY_KEY[fromEnv.toLowerCase()]) return CHANNEL_BY_KEY[fromEnv.toLowerCase()]
	return 'prod'
}

const channel = resolveChannel()

// The IPA is named after the channel config's `productName`. Fall back to the newest `.ipa` in the
// build dir if the expected name isn't there (covers any future productName drift).
const configPath = resolve(repoRoot, 'src-tauri', CONFIG_BY_CHANNEL[channel])
const productName = JSON.parse(readFileSync(configPath, 'utf8')).productName

// Tauri exports the IPA into an arch subdirectory (e.g. `build/arm64/Crate Staging.ipa`), so search
// the whole build tree. Match the exact productName only — NEVER fall back to a differently-named
// IPA: a stale `Crate Dev.ipa` left in `build/` would otherwise get installed over the dev app,
// which is exactly how a staging run can silently clobber the wrong app.
function findFiles(dir, fileName) {
	if (!existsSync(dir)) return []
	const out = []
	for (const entry of readdirSync(dir, { withFileTypes: true })) {
		const full = join(dir, entry.name)
		if (entry.isDirectory()) out.push(...findFiles(full, fileName))
		else if (entry.isFile() && entry.name === fileName) out.push(full)
	}
	return out
}

const matches = findFiles(buildDir, `${productName}.ipa`)
	.map((f) => ({ f, mtime: statSync(f).mtimeMs }))
	.sort((a, b) => b.mtime - a.mtime)
if (matches.length === 0) {
	fail(`no "${productName}.ipa" found under ${buildDir} — did the \`tauri ios build\` step succeed?`)
}
const ipaPath = matches[0].f

// Resolve a connected iOS device via devicectl's JSON output.
let devices = []
try {
	const out = join(mkdtempSync(join(tmpdir(), 'crate-ios-')), 'devices.json')
	execFileSync('xcrun', ['devicectl', 'list', 'devices', '--json-output', out], { stdio: 'ignore' })
	devices = JSON.parse(readFileSync(out, 'utf8'))?.result?.devices ?? []
} catch {
	fail('could not run `xcrun devicectl` — Xcode 15+ with command line tools is required')
}

const phones = devices.filter((d) => d?.hardwareProperties?.platform === 'iOS')
if (phones.length === 0) {
	fail('no iPhone detected — plug it in over USB, unlock it, and tap "Trust This Computer", then re-run')
}

// Prefer an actively-connected device; otherwise take the first detected iPhone and let devicectl
// establish the connection (it errors clearly if the device truly isn't reachable).
const connected = phones.filter((d) => d?.connectionProperties?.tunnelState === 'connected')
const device = connected[0] ?? phones[0]
const udid = device?.hardwareProperties?.udid
const name = device?.deviceProperties?.name ?? 'iPhone'
if (phones.length > 1) console.log(`[install-ios-device] ${phones.length} devices found; installing to "${name}"`)

console.log(`[install-ios-device] installing "${productName}" → ${name} (${udid})`)
try {
	execFileSync('xcrun', ['devicectl', 'device', 'install', 'app', '--device', udid, ipaPath], { stdio: 'inherit' })
} catch {
	fail(
		`install failed — make sure "${name}" is unlocked and trusted, then retry with:\n` +
			`  xcrun devicectl device install app --device ${udid} "${ipaPath}"`,
	)
}
console.log(`[install-ios-device] done — unplug and launch "${productName}" from the home screen`)
