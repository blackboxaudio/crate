#!/usr/bin/env node
/**
 * Write `src-tauri/cloud_sync.config.json` from `GCLOUD_*` environment variables.
 *
 * Used by CI **mobile** release builds: they ship no config file (it's gitignored) and run through
 * Xcode / Gradle, which strip shell env before `cargo` — so `build.rs` (which bakes the config in
 * via `option_env!`) needs the values on disk. This step materializes them from GitHub repository
 * secrets so `build.rs` can read them, exactly like local dev reads the developer's own file.
 *
 * Desktop CI builds do NOT need this — plain `cargo`/`tauri build` lets `option_env!` read the
 * ambient `GCLOUD_*` directly. Mobile compile-check jobs (`cargo check`) don't need it either.
 *
 * Safe by design:
 * - Refuses to overwrite an existing file (protects a developer's local config); pass `--force`.
 * - Exits 0 (not 1) when the 5 core secrets aren't all present, so PRs from forks — which don't
 *   receive secrets — still build (just without cloud sync) instead of failing the job.
 *
 * Usage: node scripts/write-cloud-config.mjs [--force]
 */
import { existsSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const outPath = resolve(repoRoot, 'src-tauri/cloud_sync.config.json')
const force = process.argv.includes('--force')

// config.json field -> env var. The 5 core fields gate `is_complete()` (config.rs); the 2 mobile
// client ids are optional but required for mobile *sign-in* to work.
const CORE = {
	project_id: 'GCLOUD_PROJECT_ID',
	web_api_key: 'GCLOUD_WEB_API_KEY',
	storage_bucket: 'GCLOUD_STORAGE_BUCKET',
	oauth_client_id: 'GCLOUD_OAUTH_CLIENT_ID',
	oauth_client_secret: 'GCLOUD_OAUTH_CLIENT_SECRET',
}
const MOBILE = {
	ios_oauth_client_id: 'GCLOUD_IOS_OAUTH_CLIENT_ID',
	android_oauth_client_id: 'GCLOUD_ANDROID_OAUTH_CLIENT_ID',
}

if (existsSync(outPath) && !force) {
	console.log(`[write-cloud-config] ${outPath} already exists; leaving it untouched (pass --force to overwrite)`)
	process.exit(0)
}

const config = {}
const missing = []
for (const [field, envVar] of Object.entries(CORE)) {
	const value = process.env[envVar]
	if (value && value.trim() !== '') config[field] = value
	else missing.push(envVar)
}

if (missing.length > 0) {
	console.warn(
		`[write-cloud-config] missing required secret(s): ${missing.join(', ')} — not writing config; ` +
			'cloud sync will be unavailable in this build',
	)
	process.exit(0)
}

for (const [field, envVar] of Object.entries(MOBILE)) {
	const value = process.env[envVar]
	if (value && value.trim() !== '') config[field] = value
}

writeFileSync(outPath, `${JSON.stringify(config, null, 2)}\n`)
console.log(`[write-cloud-config] wrote ${outPath} with field(s): ${Object.keys(config).join(', ')}`)
if (!config.ios_oauth_client_id) {
	console.warn('[write-cloud-config] GCLOUD_IOS_OAUTH_CLIENT_ID unset — iOS sign-in would fail with "mobile OAuth client id not configured"')
}
if (!config.android_oauth_client_id) {
	console.warn('[write-cloud-config] GCLOUD_ANDROID_OAUTH_CLIENT_ID unset — Android sign-in would fail with "mobile OAuth client id not configured"')
}
