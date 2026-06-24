<script lang="ts">
	import { onMount } from 'svelte'
	import { get } from 'svelte/store'
	import { translate } from '$shared/i18n'
	import type { Tag, TagCategory } from '$shared/types'
	import { pickTagCategoryColor } from '$shared/types'
	import { tagsStore } from '$shared/stores/tags'
	import { accentColor } from '$shared/stores/settings'
	import { mobileUIStore } from '$lib/stores/mobileUI'
	import { lightTap, rigidTap } from '$lib/utils/haptics'
	import { confirmDialog } from '$lib/utils/dialog'
	import MobilePromptDialog from '$lib/components/common/MobilePromptDialog.svelte'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import ContextMenuItem from '$lib/components/common/ContextMenuItem.svelte'
	import TagColorPicker from './TagColorPicker.svelte'

	// Tags tab: browse + manage the user's tag categories and tags. Each category is a section of colored,
	// tappable tag chips; tapping a chip drills into a feed of the releases carrying that tag (see
	// TagDetailView). A toolbar "+" creates a category, a dashed "+" chip adds a tag, and long-pressing a chip
	// (rename / delete) or a category name (rename / set color / delete) opens an iOS context menu. All CRUD
	// goes through the shared `tagsStore` (Tauri commands), so categories/tags created here converge with
	// desktop via cloud sync.
	const MAX_CATEGORIES = 4 // matches desktop's cap

	onMount(() => {
		tagsStore.load()
	})

	const categories = $derived($tagsStore.categories)

	function openTag(tagId: string) {
		void lightTap()
		mobileUIStore.openTag(tagId)
	}

	// --- Create / rename / recolor / delete -------------------------------------------------------------
	let createCategoryOpen = $state(false)
	let createCategoryName = $state('')

	let createTagOpen = $state(false)
	let createTagName = $state('')
	let createTagCategoryId = $state<string | null>(null)

	type RenameTarget = { type: 'category'; category: TagCategory } | { type: 'tag'; tag: Tag }
	let renameOpen = $state(false)
	let renameName = $state('')
	let renameTarget = $state<RenameTarget | null>(null)
	const renameTitle = $derived(
		renameTarget?.type === 'category' ? $translate('modals.renameCategory.title') : $translate('modals.renameTag.title')
	)
	const renamePlaceholder = $derived(
		renameTarget?.type === 'category'
			? $translate('modals.renameCategory.placeholder')
			: $translate('modals.renameTag.placeholder')
	)

	let colorPickerOpen = $state(false)
	let colorTarget = $state<TagCategory | null>(null)

	function openCreateCategory() {
		if (categories.length >= MAX_CATEGORIES) return
		void lightTap()
		createCategoryName = ''
		createCategoryOpen = true
	}

	async function handleCreateCategory() {
		const name = createCategoryName.trim()
		if (!name) return
		createCategoryOpen = false
		// Auto-assign a color (next unused preset, seeded from the accent) — mirrors desktop's create flow.
		const color = pickTagCategoryColor(get(tagsStore).categories, get(accentColor))
		await tagsStore.createCategory(name, color)
		createCategoryName = ''
	}

	function openCreateTag(categoryId: string) {
		rowActionsOpen = false
		void lightTap()
		createTagCategoryId = categoryId
		createTagName = ''
		createTagOpen = true
	}

	async function handleCreateTag() {
		const name = createTagName.trim()
		if (!name || !createTagCategoryId) return
		createTagOpen = false
		await tagsStore.createTag(createTagCategoryId, name)
		createTagName = ''
		createTagCategoryId = null
	}

	function openRenameCategory(category: TagCategory) {
		rowActionsOpen = false
		renameTarget = { type: 'category', category }
		renameName = category.name
		renameOpen = true
	}

	function openRenameTag(tag: Tag) {
		rowActionsOpen = false
		renameTarget = { type: 'tag', tag }
		renameName = tag.name
		renameOpen = true
	}

	async function handleRename() {
		const name = renameName.trim()
		if (!renameTarget || !name) return
		renameOpen = false
		if (renameTarget.type === 'category') await tagsStore.updateCategory(renameTarget.category.id, name)
		else await tagsStore.updateTag(renameTarget.tag.id, name)
		renameTarget = null
	}

	function openColorPicker(category: TagCategory) {
		rowActionsOpen = false
		colorTarget = category
		colorPickerOpen = true
	}

	async function handleColorSelect(hex: string) {
		if (!colorTarget) return
		// name omitted → only the color is updated (the backend leaves the untouched column alone).
		await tagsStore.updateCategory(colorTarget.id, undefined, hex)
		colorTarget = null
	}

	async function handleDeleteCategory(category: TagCategory) {
		rowActionsOpen = false
		const t = get(translate)
		const ok = await confirmDialog(t('modals.confirm.deleteCategoryMessage'), {
			title: t('modals.confirm.deleteCategoryTitle'),
			confirmLabel: t('common.delete'),
		})
		if (!ok) return
		await tagsStore.deleteCategory(category.id)
	}

	async function handleDeleteTag(tag: Tag) {
		rowActionsOpen = false
		const t = get(translate)
		const ok = await confirmDialog(t('modals.confirm.deleteTagMessage'), {
			title: t('modals.confirm.deleteTagTitle'),
			confirmLabel: t('common.delete'),
		})
		if (!ok) return
		await tagsStore.deleteTag(tag.id)
	}

	// --- Long-press → context menu (rows are categories or tags) ----------------------------------------
	type LongPressTarget = { type: 'category'; category: TagCategory } | { type: 'tag'; tag: Tag; category: TagCategory }
	let longPressTimer = 0
	let longPressTarget = $state<LongPressTarget | null>(null)
	let rowActionsOpen = $state(false)
	// Viewport rect of the long-pressed row, so the context menu can lift it in place.
	let longPressRect = $state<{ top: number; left: number; width: number; height: number } | null>(null)
	// A stationary long-press also synthesizes a click on release; this latches so we can swallow that one.
	let suppressNextClick = false

	// Narrow the latched target in the script (so the template keys off plain nullable values rather than
	// relying on in-template discriminated-union narrowing).
	const lpCategory = $derived(longPressTarget?.type === 'category' ? longPressTarget.category : null)
	const lpTag = $derived(longPressTarget?.type === 'tag' ? longPressTarget : null)

	function startLongPress(e: PointerEvent, target: LongPressTarget) {
		suppressNextClick = false
		if (longPressTimer) clearTimeout(longPressTimer)
		const el = e.currentTarget as HTMLElement
		longPressTimer = window.setTimeout(() => {
			longPressTimer = 0
			const r = el?.getBoundingClientRect()
			longPressRect = r ? { top: r.top, left: r.left, width: r.width, height: r.height } : null
			suppressNextClick = true
			void rigidTap()
			longPressTarget = target
			rowActionsOpen = true
		}, 450)
		window.addEventListener('pointermove', cancelLongPress, { once: true, passive: true })
		window.addEventListener('pointerup', cancelLongPress, { once: true })
		window.addEventListener('pointercancel', cancelLongPress, { once: true })
	}

	function cancelLongPress() {
		if (longPressTimer) {
			clearTimeout(longPressTimer)
			longPressTimer = 0
		}
	}

	function onRowClickCapture(e: MouseEvent) {
		if (!suppressNextClick) return
		suppressNextClick = false
		e.preventDefault()
		e.stopPropagation()
	}

	// Menu actions read the latched target and narrow it (the items render per-type, but the handlers can't
	// assume that statically) — so each guards before dispatching.
	function menuAddTag() {
		const t = longPressTarget
		if (t?.type === 'category') openCreateTag(t.category.id)
	}
	function menuRename() {
		const t = longPressTarget
		if (t?.type === 'category') openRenameCategory(t.category)
		else if (t?.type === 'tag') openRenameTag(t.tag)
	}
	function menuSetColor() {
		const t = longPressTarget
		if (t?.type === 'category') openColorPicker(t.category)
	}
	function menuDelete() {
		const t = longPressTarget
		if (t?.type === 'category') void handleDeleteCategory(t.category)
		else if (t?.type === 'tag') void handleDeleteTag(t.tag)
	}
</script>

<div class="flex h-full flex-col">
	<!-- Toolbar: a single "+" creates a category (capped at 4, matching desktop). Adding tags is done with the
	     dashed "+" chip in each category below. -->
	<div class="flex items-center justify-end px-2 py-2">
		<button
			type="button"
			class="flex h-10 w-10 items-center justify-center rounded-md text-text-secondary active:bg-surface-2 disabled:opacity-40"
			aria-label={$translate('tags.newCategory')}
			disabled={categories.length >= MAX_CATEGORIES}
			onclick={openCreateCategory}
		>
			<svg class="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M12 5v14M5 12h14" stroke-linecap="round" />
			</svg>
		</button>
	</div>

	<div class="min-h-0 flex-1 overflow-y-auto" style="padding-bottom: var(--mini-player-inset, 0px)">
		{#if categories.length === 0}
			<div class="px-4 py-12 text-center text-sm text-text-secondary">{$translate('tags.noTagCategoriesYet')}</div>
		{:else}
			{#each categories as category (category.id)}
				<div class="px-4 py-2">
					<!-- Category name (long-press → manage: rename / set color / delete). -->
					<h3
						class="mb-1.5 text-sm font-medium text-text-secondary"
						onpointerdown={(e) => startLongPress(e, { type: 'category', category })}
						onclickcapture={onRowClickCapture}
					>
						{category.name}
					</h3>

					<!-- Tappable tag chips (tap → drill into the tag's feed; long-press → rename / delete) plus a
					     dashed "+" chip to add a tag to this category. -->
					<div class="flex flex-wrap gap-2">
						{#each category.tags as tag (tag.id)}
							{@const color = tag.color ?? category.color ?? '#6366f1'}
							<div
								onpointerdown={(e) => startLongPress(e, { type: 'tag', tag, category })}
								onclickcapture={onRowClickCapture}
							>
								<button
									type="button"
									class="rounded-md px-3 py-2 text-sm font-medium active:opacity-70"
									style="background-color: {color}20; color: {color}; border: 1px solid {color}40;"
									onclick={() => openTag(tag.id)}
								>
									{tag.name}
								</button>
							</div>
						{/each}
						<button
							type="button"
							class="inline-flex items-center justify-center rounded-md border border-dashed border-stroke px-3 py-2 text-text-tertiary active:bg-surface-2"
							aria-label={$translate('tags.addTag')}
							onclick={() => openCreateTag(category.id)}
						>
							<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<path d="M12 5v14M5 12h14" stroke-linecap="round" />
							</svg>
						</button>
					</div>
				</div>
			{/each}
		{/if}
	</div>
</div>

<!-- Create category dialog -->
<MobilePromptDialog
	open={createCategoryOpen}
	bind:value={createCategoryName}
	title={$translate('modals.createCategory.title')}
	placeholder={$translate('modals.createCategory.placeholder')}
	confirmDisabled={!createCategoryName.trim()}
	onConfirm={handleCreateCategory}
	onCancel={() => (createCategoryOpen = false)}
/>

<!-- Create tag dialog (category implied by which section's "+" was tapped) -->
<MobilePromptDialog
	open={createTagOpen}
	bind:value={createTagName}
	title={$translate('modals.createTag.title')}
	placeholder={$translate('modals.createTag.tagPlaceholder')}
	confirmDisabled={!createTagName.trim()}
	onConfirm={handleCreateTag}
	onCancel={() => (createTagOpen = false)}
/>

<!-- Rename dialog (category or tag) -->
<MobilePromptDialog
	open={renameOpen}
	bind:value={renameName}
	title={renameTitle}
	placeholder={renamePlaceholder}
	confirmLabel={$translate('common.save')}
	confirmDisabled={!renameName.trim()}
	onConfirm={handleRename}
	onCancel={() => (renameOpen = false)}
/>

<!-- Category color picker -->
<TagColorPicker
	open={colorPickerOpen}
	current={colorTarget?.color ?? null}
	onSelect={handleColorSelect}
	onClose={() => (colorPickerOpen = false)}
/>

<!-- Row long-press context menu: category (add tag / rename / set color / delete) or tag (rename / delete). -->
<ContextMenu
	open={rowActionsOpen}
	anchorRect={longPressRect}
	onClose={() => (rowActionsOpen = false)}
	onClosed={() => {
		longPressTarget = null
		longPressRect = null
	}}
>
	{#snippet preview()}
		{#if lpCategory}
			<span class="min-w-0 flex-1 truncate text-sm font-semibold text-text-primary">{lpCategory.name}</span>
		{:else if lpTag}
			<span
				class="block h-3 w-3 flex-shrink-0 rounded-full"
				style="background-color: {lpTag.tag.color ?? lpTag.category.color ?? '#6366f1'}"
			></span>
			<span class="min-w-0 flex-1 truncate text-sm text-text-primary">{lpTag.tag.name}</span>
		{/if}
	{/snippet}

	{#if lpCategory}
		<ContextMenuItem onclick={menuAddTag}>
			{$translate('tags.addTag')}
			{#snippet icon()}
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M12 5v14M5 12h14" stroke-linecap="round" />
				</svg>
			{/snippet}
		</ContextMenuItem>
		<ContextMenuItem onclick={menuRename}>
			{$translate('common.rename')}
			{#snippet icon()}
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M17 3a2.828 2.828 0 114 4L7.5 20.5 2 22l1.5-5.5L17 3z" />
				</svg>
			{/snippet}
		</ContextMenuItem>
		<ContextMenuItem onclick={menuSetColor}>
			{$translate('contextMenu.setColor')}
			{#snippet icon()}
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M12 3s6 6.5 6 10.5a6 6 0 11-12 0C6 9.5 12 3 12 3z" stroke-linecap="round" stroke-linejoin="round" />
				</svg>
			{/snippet}
		</ContextMenuItem>
		<ContextMenuItem destructive onclick={menuDelete}>
			{$translate('common.delete')}
			{#snippet icon()}
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path
						d="M3 6h18M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2m2 0v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V6"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
				</svg>
			{/snippet}
		</ContextMenuItem>
	{:else if lpTag}
		<ContextMenuItem onclick={menuRename}>
			{$translate('common.rename')}
			{#snippet icon()}
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M17 3a2.828 2.828 0 114 4L7.5 20.5 2 22l1.5-5.5L17 3z" />
				</svg>
			{/snippet}
		</ContextMenuItem>
		<ContextMenuItem destructive onclick={menuDelete}>
			{$translate('common.delete')}
			{#snippet icon()}
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path
						d="M3 6h18M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2m2 0v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V6"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
				</svg>
			{/snippet}
		</ContextMenuItem>
	{/if}
</ContextMenu>
