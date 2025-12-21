import { invoke } from '@tauri-apps/api/core'
import type { Tag, TagCategory } from '$lib/types'

/**
 * Get all tag categories with their tags
 */
export async function getTagCategories(): Promise<TagCategory[]> {
	return invoke<TagCategory[]>('get_tag_categories')
}

/**
 * Create a new tag category (max 4 allowed)
 */
export async function createTagCategory(name: string, color?: string): Promise<TagCategory> {
	return invoke<TagCategory>('create_tag_category', { name, color: color ?? null })
}

/**
 * Update a tag category's name and/or color
 */
export async function updateTagCategory(id: string, name?: string, color?: string): Promise<TagCategory> {
	return invoke<TagCategory>('update_tag_category', {
		id,
		name: name ?? null,
		color: color ?? null,
	})
}

/**
 * Delete a tag category and all its tags
 */
export async function deleteTagCategory(id: string): Promise<void> {
	return invoke<void>('delete_tag_category', { id })
}

/**
 * Create a new tag within a category
 */
export async function createTag(categoryId: string, name: string, color?: string): Promise<Tag> {
	return invoke<Tag>('create_tag', { categoryId, name, color: color ?? null })
}

/**
 * Update a tag's name or color
 */
export async function updateTag(id: string, name?: string, color?: string): Promise<Tag> {
	return invoke<Tag>('update_tag', {
		id,
		name: name ?? null,
		color: color ?? null,
	})
}

/**
 * Delete a tag
 */
export async function deleteTag(id: string): Promise<void> {
	return invoke<void>('delete_tag', { id })
}

/**
 * Assign tags to tracks
 */
export async function assignTags(trackIds: string[], tagIds: string[]): Promise<void> {
	return invoke<void>('assign_tags', { trackIds, tagIds })
}

/**
 * Remove tags from tracks
 */
export async function removeTags(trackIds: string[], tagIds: string[]): Promise<void> {
	return invoke<void>('remove_tags', { trackIds, tagIds })
}
