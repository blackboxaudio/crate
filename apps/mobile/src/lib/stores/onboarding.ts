import { writable } from 'svelte/store'
import { getStoredBoolean, setStoredBoolean } from '$shared/utils/storage'

// First-run onboarding is gated on a DEVICE-LOCAL localStorage flag — deliberately NOT the shared
// `hasCompletedOnboarding` setting, which is desktop-oriented and cloud-syncs via `Bucket::Settings`.
// Reusing that would suppress mobile onboarding for anyone who onboarded on desktop, whereas the mobile
// first-run flow (add-release / preview / optional sign-in) is distinct and per-device.
const STORAGE_KEY = 'mobile-onboarding-complete'

function createOnboardingStore() {
	// Seed from storage so a returning user never sees the carousel again (and boot doesn't flash it).
	const { subscribe, set } = writable(getStoredBoolean(STORAGE_KEY, false))

	return {
		subscribe,
		// Mark onboarding done — persists across launches and flips the overlay off reactively.
		complete() {
			setStoredBoolean(STORAGE_KEY, true)
			set(true)
		},
	}
}

export const onboardingComplete = createOnboardingStore()
