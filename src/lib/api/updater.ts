import { check, type Update } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'

export type { Update }
export { relaunch }

export async function checkForUpdate(): Promise<Update | null> {
	return check()
}
