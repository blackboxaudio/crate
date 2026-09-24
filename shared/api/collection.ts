import { invoke } from '@tauri-apps/api/core'
import type {
	AccountRefreshResult,
	CollectionAccount,
	CollectionGapItem,
	CollectionItem,
	CollectionOwnership,
	CollectionRefreshSummary,
} from '../types'

/** Link a pasted Bandcamp fan-page URL (backend fetches the page to validate + name it). */
export async function linkCollectionAccount(url: string): Promise<CollectionAccount> {
	return invoke<CollectionAccount>('link_collection_account', { url })
}

export async function unlinkCollectionAccount(id: string): Promise<void> {
	return invoke<void>('unlink_collection_account', { id })
}

export async function setCollectionAccountEnabled(id: string, enabled: boolean): Promise<CollectionAccount> {
	return invoke<CollectionAccount>('set_collection_account_enabled', { id, enabled })
}

export async function getCollectionAccounts(): Promise<CollectionAccount[]> {
	return invoke<CollectionAccount[]>('get_collection_accounts')
}

/** The union of owned items across enabled accounts, newest purchases first. */
export async function getCollectionItems(): Promise<CollectionItem[]> {
	return invoke<CollectionItem[]>('get_collection_items')
}

/** Derived ownership id-sets for badge/filter rendering. */
export async function getCollectionOwnership(): Promise<CollectionOwnership> {
	return invoke<CollectionOwnership>('get_collection_ownership')
}

/** Incremental refresh of one account (stops at already-known items). */
export async function refreshCollectionAccount(id: string): Promise<AccountRefreshResult> {
	return invoke<AccountRefreshResult>('refresh_collection_account', { id })
}

export async function refreshAllCollectionAccounts(): Promise<CollectionRefreshSummary> {
	return invoke<CollectionRefreshSummary>('refresh_all_collection_accounts')
}

/** "Purchased but not in library" cross-reference (desktop-only command). */
export async function getCollectionLibraryGap(): Promise<CollectionGapItem[]> {
	return invoke<CollectionGapItem[]>('get_collection_library_gap')
}
