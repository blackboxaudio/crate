import { impactFeedback } from '@tauri-apps/plugin-haptics'

/**
 * Fire a light haptic tap to acknowledge a touch (e.g. tapping a track to play). Best-effort: the
 * haptics plugin is mobile-only and can be unavailable (simulator, denied permission, older device),
 * so any failure is swallowed — a missing buzz must never break the interaction.
 */
export async function lightTap(): Promise<void> {
	try {
		await impactFeedback('light')
	} catch {
		// Haptics unavailable — ignore.
	}
}
