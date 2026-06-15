# Mobile cloud-sync OAuth setup

Native mobile sign-in (iOS/Android) uses the platform's secure web-auth session
([`tauri-plugin-web-auth`](https://crates.io/crates/tauri-plugin-web-auth) — iOS
`ASWebAuthenticationSession`, Android Custom Tabs) instead of the desktop loopback flow. The
Rust auth core is shared; only the browser presentation + callback capture differ.

Flow: `begin_sign_in` (Rust builds the consent URL + PKCE secrets, stashed server-side) →
frontend `authenticate({ url, callbackScheme })` → `complete_sign_in(code, state)` (Rust
exchanges the code, signs in to Firebase, persists the refresh token). The PKCE verifier never
leaves Rust; only the single-use authorization code transits the webview.

The code is complete, but the following **one-time setup** is required before mobile sign-in
works on a device. None of it can be scripted here — it needs the Google Cloud Console and the
generated Android project.

## 1. Create the mobile Google OAuth clients (issue #49)

In the same Firebase / GCP project the desktop client uses, create **two** OAuth client IDs
(APIs & Services → Credentials). Neither has a client secret (they are public clients; PKCE is
the proof).

- **iOS** client — bundle id `com.bbx-audio.crate`.
- **Android** client — package name `com.bbx-audio.crateapp` (the id CI uses; the desktop id
  `com.bbx-audio.crate` ends in the Rust keyword `crate` and can't be the Android app id) plus
  the signing certificate SHA-1 fingerprint.

The OAuth **callback scheme** is the *reversed client id*, derived automatically at runtime
(`com.googleusercontent.apps.<id-head>`), e.g. client id
`1234-abc.apps.googleusercontent.com` → scheme `com.googleusercontent.apps.1234-abc`.

## 2. Add the client ids to config

For local dev, add to `src-tauri/cloud_sync.config.json` (see `cloud_sync.config.example.json`):

```json
"ios_oauth_client_id": "<ios-client-id>.apps.googleusercontent.com",
"android_oauth_client_id": "<android-client-id>.apps.googleusercontent.com"
```

For release/CI builds, add the env vars to the CD workflow + GitHub secrets:

- `GCLOUD_IOS_OAUTH_CLIENT_ID`
- `GCLOUD_ANDROID_OAUTH_CLIENT_ID`

Both are optional: a desktop config that omits them is still valid (`is_complete()` ignores
them); mobile sign-in just fails with a clear "mobile OAuth client id not configured" error
until they're set.

## 3. iOS

No `Info.plist` change is required — `ASWebAuthenticationSession` captures the callback for the
given scheme internally. (Verify on first device run.)

## 4. Android — register the callback intent-filter

`src-tauri/gen/android` is not generated yet. After running `yarn tauri android init` once, add
the callback intent-filter to the launcher activity in
`src-tauri/gen/android/app/src/main/AndroidManifest.xml`, inside `<activity android:name=".MainActivity" …>`:

```xml
<intent-filter>
    <action android:name="android.intent.action.VIEW" />
    <category android:name="android.intent.category.DEFAULT" />
    <category android:name="android.intent.category.BROWSABLE" />
    <!-- Must equal reversed_client_id(android_oauth_client_id) — see config.rs -->
    <data android:scheme="com.googleusercontent.apps.YOUR_ANDROID_CLIENT_ID_HEAD" />
</intent-filter>
```

Then **commit `src-tauri/gen/android`** (it is intentionally *not* in `.gitignore`, unlike
`gen/apple`) so the manifest edit is durable. Re-running `tauri android init` later produces a
reviewable diff rather than silently dropping the intent-filter.

## 5. Install JS deps

`apps/mobile/package.json` now depends on `@tauri-apps/api` and `tauri-plugin-web-auth-api`.
Run `yarn install` at the repo root to regenerate `yarn.lock` (CI uses `--frozen-lockfile`).

## Security dependency (#134)

The Firebase refresh token persists to the SQLCipher `sync_state` table — the same path as
desktop. On mobile, the encrypted-DB key provider is issue #134's responsibility. Mobile sign-in
is *functional* without #134, but confirm the mobile DB is actually encrypted at rest before
shipping sign-in, or the refresh token is less protected than on desktop.
