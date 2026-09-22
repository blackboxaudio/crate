import { get } from 'svelte/store'
import { translate } from '../i18n'
import { toastStore } from '../stores/toast'
import type { AddToPlaylistResult } from '../types'

/**
 * Feedback for an add-to-playlist action, driven by what the backend actually inserted
 * rather than the selection size. The target playlist is usually not the open view, so a
 * re-add of existing tracks still gets a (neutral) toast — otherwise it is indistinguishable
 * from a drop that missed. `null` is the store's failure result.
 */
export function toastPlaylistAdd(result: AddToPlaylistResult | null, playlistName: string): void {
	const t = get(translate)
	if (!result) {
		toastStore.error(t('toast.failedToAddPlaylist'))
	} else if (result.added === 0) {
		toastStore.info(t('toast.alreadyInPlaylist', { values: { playlistName } }))
	} else if (result.alreadyPresent > 0) {
		toastStore.success(
			t('toast.trackAddedWithExisting', {
				values: { count: result.added, existing: result.alreadyPresent, playlistName },
			})
		)
	} else {
		toastStore.success(t('toast.trackAdded', { values: { count: result.added, playlistName } }))
	}
}
