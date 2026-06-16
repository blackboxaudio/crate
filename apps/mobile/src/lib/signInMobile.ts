import { authenticate } from 'tauri-plugin-web-auth-api'
import { cloudSyncStore } from '$shared/stores/cloudSync'

/**
 * Run the native mobile OAuth sign-in for `providerId` (v1: `'google'`).
 *
 * This is the one place the `tauri-plugin-web-auth` import lives, so it never enters `shared/`
 * (or the desktop bundle). The actual flow — `begin_sign_in` → present the native auth session
 * (iOS `ASWebAuthenticationSession` / Android Custom Tabs) → `complete_sign_in` — is orchestrated
 * by the shared cloud-sync store, which receives `authenticate` by injection.
 */
export function signInMobile(providerId = 'google'): Promise<void> {
	return cloudSyncStore.signInMobile(providerId, authenticate)
}
