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

/** A medium impact — a firmer acknowledgement than {@link lightTap} (e.g. committing a selection). */
export async function mediumTap(): Promise<void> {
	try {
		await impactFeedback('medium')
	} catch {
		// Haptics unavailable — ignore.
	}
}

/**
 * A sharp, rigid impact — the closest match to iOS's context-menu "thump" when a long-press commits.
 * Fired the moment a context menu opens. Best-effort like the others.
 */
export async function rigidTap(): Promise<void> {
	try {
		await impactFeedback('rigid')
	} catch {
		// Haptics unavailable — ignore.
	}
}
