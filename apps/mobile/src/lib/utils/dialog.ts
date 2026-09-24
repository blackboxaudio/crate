import { confirm } from '@tauri-apps/plugin-dialog'
import { get } from 'svelte/store'
import { translate } from '$shared/i18n'

export type ConfirmDialogOptions = {
	/** Dialog title — usually the question itself (e.g. "Delete release?"). Defaults to the app name. */
	title?: string
	/** Affirmative button label. Names the action, so it's always explicit (e.g. Delete, Remove, Discard). */
	confirmLabel: string
	/** Dismiss button label. Defaults to the localized `common.cancel`. */
	cancelLabel?: string
	/** Platform severity hint (desktop shows a matching icon). Defaults to `warning` for destructive prompts. */
	kind?: 'info' | 'warning' | 'error'
}

/**
 * Native OS confirm dialog (iOS UIAlertController / Android AlertDialog) via the Tauri dialog plugin.
 * Mobile prefers these over hand-built web modals for simple confirms: they match the platform, own their
 * own animation + accessibility, and need no custom UI. Resolves true only when the user taps the
 * affirmative button. Labels come from app i18n (not the OS) so the whole dialog stays in the app's
 * language. Fails closed — if the dialog can't be shown, resolves false so a destructive caller never
 * proceeds on its own.
 */
export async function confirmDialog(message: string, options: ConfirmDialogOptions): Promise<boolean> {
	try {
		return await confirm(message, {
			title: options.title,
			kind: options.kind ?? 'warning',
			okLabel: options.confirmLabel,
			cancelLabel: options.cancelLabel ?? get(translate)('common.cancel'),
		})
	} catch {
		return false
	}
}
