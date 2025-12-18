import { writable, derived } from 'svelte/store';
import type { TagCategory, Tag } from '$lib/types';
import * as tagsApi from '$lib/api/tags';

// =============================================================================
// State
// =============================================================================

interface TagsState {
  categories: TagCategory[];
  loading: boolean;
  error: string | null;
}

const initialState: TagsState = {
  categories: [],
  loading: false,
  error: null
};

// =============================================================================
// Store
// =============================================================================

function createTagsStore() {
  const { subscribe, set, update } = writable<TagsState>(initialState);

  return {
    subscribe,

    /**
     * Load all tag categories and tags
     */
    async load() {
      update((state) => ({ ...state, loading: true, error: null }));

      try {
        const categories = await tagsApi.getTagCategories();
        update((state) => ({
          ...state,
          categories,
          loading: false
        }));
      } catch (error) {
        update((state) => ({
          ...state,
          loading: false,
          error: error instanceof Error ? error.message : 'Failed to load tags'
        }));
      }
    },

    /**
     * Create a new tag category
     */
    async createCategory(name: string) {
      try {
        const category = await tagsApi.createTagCategory(name);
        update((state) => ({
          ...state,
          categories: [...state.categories, category]
        }));
        return category;
      } catch (error) {
        update((state) => ({
          ...state,
          error: error instanceof Error ? error.message : 'Failed to create category'
        }));
        return null;
      }
    },

    /**
     * Update a tag category
     */
    async updateCategory(id: string, name: string) {
      try {
        const updated = await tagsApi.updateTagCategory(id, name);
        update((state) => ({
          ...state,
          categories: state.categories.map((c) => (c.id === id ? updated : c))
        }));
        return updated;
      } catch (error) {
        update((state) => ({
          ...state,
          error: error instanceof Error ? error.message : 'Failed to update category'
        }));
        return null;
      }
    },

    /**
     * Delete a tag category
     */
    async deleteCategory(id: string) {
      try {
        await tagsApi.deleteTagCategory(id);
        update((state) => ({
          ...state,
          categories: state.categories.filter((c) => c.id !== id)
        }));
      } catch (error) {
        update((state) => ({
          ...state,
          error: error instanceof Error ? error.message : 'Failed to delete category'
        }));
      }
    },

    /**
     * Create a new tag
     */
    async createTag(categoryId: string, name: string, color?: string) {
      try {
        const tag = await tagsApi.createTag(categoryId, name, color);
        update((state) => ({
          ...state,
          categories: state.categories.map((c) => {
            if (c.id === categoryId) {
              return { ...c, tags: [...c.tags, tag] };
            }
            return c;
          })
        }));
        return tag;
      } catch (error) {
        update((state) => ({
          ...state,
          error: error instanceof Error ? error.message : 'Failed to create tag'
        }));
        return null;
      }
    },

    /**
     * Update a tag
     */
    async updateTag(id: string, name?: string, color?: string) {
      try {
        const updated = await tagsApi.updateTag(id, name, color);
        update((state) => ({
          ...state,
          categories: state.categories.map((c) => ({
            ...c,
            tags: c.tags.map((t) => (t.id === id ? updated : t))
          }))
        }));
        return updated;
      } catch (error) {
        update((state) => ({
          ...state,
          error: error instanceof Error ? error.message : 'Failed to update tag'
        }));
        return null;
      }
    },

    /**
     * Delete a tag
     */
    async deleteTag(id: string) {
      try {
        await tagsApi.deleteTag(id);
        update((state) => ({
          ...state,
          categories: state.categories.map((c) => ({
            ...c,
            tags: c.tags.filter((t) => t.id !== id)
          }))
        }));
      } catch (error) {
        update((state) => ({
          ...state,
          error: error instanceof Error ? error.message : 'Failed to delete tag'
        }));
      }
    },

    /**
     * Assign tags to tracks
     */
    async assignTags(trackIds: string[], tagIds: string[]) {
      try {
        await tagsApi.assignTags(trackIds, tagIds);
      } catch (error) {
        update((state) => ({
          ...state,
          error: error instanceof Error ? error.message : 'Failed to assign tags'
        }));
      }
    },

    /**
     * Remove tags from tracks
     */
    async removeTags(trackIds: string[], tagIds: string[]) {
      try {
        await tagsApi.removeTags(trackIds, tagIds);
      } catch (error) {
        update((state) => ({
          ...state,
          error: error instanceof Error ? error.message : 'Failed to remove tags'
        }));
      }
    },

    /**
     * Reset store to initial state
     */
    reset() {
      set(initialState);
    }
  };
}

export const tagsStore = createTagsStore();

// =============================================================================
// Derived Stores
// =============================================================================

/**
 * All tags flattened
 */
export const allTags = derived(tagsStore, ($tags) =>
  $tags.categories.flatMap((c) => c.tags)
);

/**
 * Get tag by ID
 */
export function getTagById(tags: Tag[], id: string): Tag | undefined {
  return tags.find((t) => t.id === id);
}

/**
 * Get category by ID
 */
export function getCategoryById(
  categories: TagCategory[],
  id: string
): TagCategory | undefined {
  return categories.find((c) => c.id === id);
}
