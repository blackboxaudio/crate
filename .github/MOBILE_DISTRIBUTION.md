# Mobile Distribution

This document covers setting up signed mobile builds for iOS (TestFlight / App Store) and Android (signed APK on GitHub Releases). The CI pipeline in `cd.release.yml` handles building and uploading automatically on `v*` tags — this guide covers the one-time setup and the secrets you need to configure.

## Architecture

Mobile jobs run alongside the desktop build in `cd.release.yml`:

```
validate → build (macOS + Windows)  ─┐
         → android (signed APK)      ├→ release (draft GitHub Release)
         → ios (TestFlight upload)   ─┘
```

Both mobile jobs use `continue-on-error: true` so they can never block a desktop release. If signing secrets are absent, the jobs skip cleanly with a warning.

## Android

### One-time setup

#### 1. Generate a release keystore

```bash
keytool -genkeypair \
  -v \
  -keystore crate-release.keystore \
  -alias crate \
  -keyalg RSA \
  -keysize 2048 \
  -validity 10000 \
  -storepass <STORE_PASSWORD> \
  -keypass <KEY_PASSWORD> \
  -dname "CN=Crate, OU=bbx-audio, O=bbx-audio, L=, S=, C=US"
```

Keep this keystore safe — losing it means you cannot update the app on any store that uses it for signing.

#### 2. Add GitHub secrets

| Secret | Value |
|--------|-------|
| `ANDROID_KEYSTORE_BASE64` | `base64 -i crate-release.keystore` |
| `ANDROID_KEYSTORE_PASSWORD` | The store password from step 1 |
| `ANDROID_KEY_ALIAS` | `crate` (or whatever alias you used) |
| `ANDROID_KEY_PASSWORD` | The key password from step 1 |

### How it works

The CI job decodes the keystore to `$RUNNER_TEMP`, writes `gen/android/key.properties` (gitignored), and `build.gradle.kts` picks it up via a guarded `signingConfigs.release`. Without `key.properties`, Gradle still configures successfully (unsigned path).

The built APK is renamed to `Crate_<version>_android.apk` with a `.sha256` checksum and attached to the GitHub Release.

### Local signing (optional)

To sign locally, create `src-tauri/gen/android/key.properties`:

```properties
storeFile=/path/to/crate-release.keystore
storePassword=<STORE_PASSWORD>
keyAlias=crate
keyPassword=<KEY_PASSWORD>
```

Then build: `yarn build:android:apk`

## iOS

### One-time setup

#### 1. Apple Developer Program

Ensure your team (`883V548CV2`) has an active Apple Developer Program membership.

#### 2. Create an App Store Distribution certificate

1. Open Keychain Access → Certificate Assistant → Request a Certificate from a Certificate Authority
2. In [Apple Developer → Certificates](https://developer.apple.com/account/resources/certificates), create a new **Apple Distribution** certificate using the CSR
3. Download and install the `.cer` file
4. Export it as `.p12` from Keychain Access (right-click → Export)

#### 3. Register the App ID

In [Apple Developer → Identifiers](https://developer.apple.com/account/resources/identifiers), register:
- Bundle ID: `com.bbx-audio.crate` (production)
- Enable capabilities: none required beyond defaults

#### 4. Create a provisioning profile

In [Apple Developer → Profiles](https://developer.apple.com/account/resources/profiles):
- Type: **App Store Connect**
- App ID: `com.bbx-audio.crate`
- Certificate: the Distribution cert from step 2

Download the `.mobileprovision` file.

#### 5. Create an App Store Connect record

1. Go to [App Store Connect](https://appstoreconnect.apple.com/)
2. Create a new iOS app with bundle ID `com.bbx-audio.crate`
3. Fill in the required metadata (name, description, screenshots, etc.)
4. Enable all territories including EU and Japan

#### 6. Create an App Store Connect API key

1. In App Store Connect → Users and Access → Integrations → App Store Connect API
2. Generate a new key with **App Manager** role
3. Download the `.p8` file (only available once)
4. Note the Key ID and Issuer ID

#### 7. Set up TestFlight

After the first successful upload, the build appears in TestFlight automatically. To share with external testers:
1. Go to App Store Connect → TestFlight → External Testing
2. Create a group and add testers, or generate a **public link** (global — works for EU + Japan)

#### 8. Add GitHub secrets

| Secret | Value |
|--------|-------|
| `IOS_DIST_CERTIFICATE_P12_BASE64` | `base64 -i distribution.p12` |
| `IOS_DIST_CERTIFICATE_PASSWORD` | The .p12 export password |
| `IOS_PROVISIONING_PROFILE_BASE64` | `base64 -i profile.mobileprovision` |
| `IOS_KEYCHAIN_PASSWORD` | Any random password (for the ephemeral CI keychain) |
| `APP_STORE_CONNECT_API_KEY_ID` | Key ID from step 6 |
| `APP_STORE_CONNECT_API_ISSUER_ID` | Issuer ID from step 6 |
| `APP_STORE_CONNECT_API_KEY_P8_BASE64` | `base64 -i AuthKey_XXXX.p8` |

### How it works

The CI job creates an ephemeral keychain, imports the distribution certificate, installs the provisioning profile, builds with `tauri ios build --export-method app-store-connect`, and uploads the IPA via `xcrun altool`. The uploaded build lands in TestFlight automatically.

**Promotion to the public App Store ("Submit for Review") is always a manual action** in App Store Connect.

### Local builds

```bash
yarn build:ios:appstore   # App Store Connect (TestFlight / App Store)
yarn build:ios:adhoc      # Ad-hoc distribution (direct install)
yarn build:android:apk    # Android APK
yarn build:android:aab    # Android App Bundle (for Play Store)
```

## Existing secrets (already configured)

These secrets are reused from the desktop release pipeline — no action needed:

| Secret | Purpose |
|--------|---------|
| `GCLOUD_PROJECT_ID` | Cloud sync config (baked into mobile binary) |
| `GCLOUD_WEB_API_KEY` | Cloud sync config |
| `GCLOUD_STORAGE_BUCKET` | Cloud sync config |
| `GCLOUD_OAUTH_CLIENT_ID` | Cloud sync config |
| `GCLOUD_OAUTH_CLIENT_SECRET` | Cloud sync config |
| `GCLOUD_IOS_OAUTH_CLIENT_ID` | iOS OAuth redirect scheme |
| `GCLOUD_ANDROID_OAUTH_CLIENT_ID` | Android OAuth client |
| `APPLE_TEAM_ID` | Apple team identifier |

## Release flow

The mobile release flow is identical to desktop — it's triggered by the same `v*` tag:

1. `./scripts/tag.sh minor` (or `prerelease`, `stage`)
2. Tag push triggers `cd.release.yml`
3. Desktop, Android, and iOS jobs run in parallel
4. Android APK + checksum land on the draft GitHub Release
5. iOS IPA uploads to TestFlight automatically
6. Review and publish the GitHub Release
7. (Optional) Submit the TestFlight build for App Store review in App Store Connect

## App Store compliance note

Crate's preview playback obtains audio streams from third-party services (Bandcamp, SoundCloud, YouTube) by scraping their web pages. Apple Guideline **5.2.3** prohibits in-app playback of third-party content without authorization. The build submitted to the App Store includes this functionality. Be aware of the review risk — an accurate App Store description of in-app third-party streaming is what surfaces the guideline. TestFlight and Android sideload APKs are unaffected by this guideline.

## EU + Japan distribution

- **TestFlight**: global by default — EU and Japan testers use the same public link
- **App Store**: enable all territories in App Store Connect (no code change)
- **Android APK**: sideloading has no regional restrictions
- **AltStore PAL (EU, optional)**: requires accepting Apple's EU alternative distribution terms (Core Technology Fee). Not set up by default.
- **F-Droid**: not possible — Crate's PolyForm Shield license is not FOSS
