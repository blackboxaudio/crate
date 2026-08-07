<script lang="ts">
	import { onMount } from 'svelte'
	import { get } from 'svelte/store'
	import { fade, scale, slide } from 'svelte/transition'
	import { translate } from '$shared/i18n'
	import type { Tag, TagCategory } from '$shared/types'
	import { DEFAULT_TAG_COLOR, pickTagCategoryColor } from '$shared/types'
	import { tagsStore } from '$shared/stores/tags'
	import { discoveryStore } from '$shared/stores/discovery'
	import { discoveryPlaylistStore } from '$shared/stores/discoveryPlaylist'
	import { toastStore } from '$shared/stores/toast'
	import { accentColor } from '$shared/stores/settings'
	import { mobileUIStore, scrollTopNonce } from '$lib/stores/mobileUI'
	import { easeFluid } from '$lib/easing'
	import { lightTap } from '$lib/utils/haptics'
	import { confirmDialog } from '$lib/utils/dialog'
	import { longPress, type LongPressRect } from '$lib/actions/longPress'
	import MobilePromptDialog from '$lib/components/common/MobilePromptDialog.svelte'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import ContextMenuItem from '$lib/components/common/ContextMenuItem.svelte'
	import TagCategoryPickerSheet from './TagCategoryPickerSheet.svelte'
	import TagColorPicker from './TagColorPicker.svelte'

	// Tags tab: browse + manage the user's tag categories and tags. Each category is a section of colored,
	// tappable tag chips; tapping a chip drills into a feed of the releases carrying that tag (see
	// TagDetailView). A toolbar "+" creates a category and a dashed "+" chip adds a tag; management lives in
	// an iOS context menu per row — a category's (add tag / rename / set color / delete) opens from its "…"
	// button or a long-press on its name, a tag's (rename / move to category / delete) from a long-press on
	// the chip. All CRUD goes through the shared `tagsStore` (Tauri commands), so categories/tags created
	// here converge with desktop via cloud sync.
	const MAX_CATEGORIES = 4 // matches desktop's cap

	onMount(() => {
		tagsStore.load()
	})

	const categories = $derived($tagsStore.categories)

	// The scroll container, for the tab re-tap scroll-to-top.
	let scrollEl = $state<HTMLElement | null>(null)
	// iOS "re-tap the active tab to scroll to top". Ignore the initial nonce so a normal mount doesn't scroll.
	let seenScrollNonce = get(mobileUIStore).scrollTopNonce
	$effect(() => {
		const n = $scrollTopNonce
		if (n === seenScrollNonce) return
		seenScrollNonce = n
		scrollEl?.scrollTo({ top: 0, behavior: 'smooth' })
	})

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

	let movePickerOpen = $state(false)
	// Snapshotted at menu-tap time (like `renameTarget`), so the menu's `onClosed` clearing the long-press
	// target after its exit animation can't race the sheet.
	let moveTarget = $state<{ tag: Tag; category: TagCategory } | null>(null)

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
		void lightTap()
		renameTarget = { type: 'category', category }
		renameName = category.name
		renameOpen = true
	}

	function openRenameTag(tag: Tag) {
		rowActionsOpen = false
		void lightTap()
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
		void lightTap()
		colorTarget = category
		colorPickerOpen = true
	}

	async function handleColorSelect(hex: string) {
		if (!colorTarget) return
		// name omitted → only the color is updated (the backend leaves the untouched column alone).
		await tagsStore.updateCategory(colorTarget.id, undefined, hex)
		colorTarget = null
	}

	async function handleMoveSelect(categoryId: string) {
		const target = moveTarget
		if (!target || categoryId === target.category.id) return
		try {
			await tagsStore.moveTag(target.tag.id, categoryId)
			// Keep the tag copies embedded on releases in step (their category_id rides along) — mirrors desktop.
			discoveryStore.updateTagCategory(target.tag.id, categoryId)
			discoveryPlaylistStore.updateTagCategory(target.tag.id, categoryId)
		} catch (error) {
			const message = error instanceof Error ? error.message : get(translate)('errors.tagNameConflict')
			toastStore.error(message)
		}
		moveTarget = null
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
	let longPressTarget = $state<LongPressTarget | null>(null)
	let rowActionsOpen = $state(false)
	// Whether the menu was opened by a discrete tap (a category's "…" button) rather than a held long-press —
	// drives ContextMenu's `tapTriggered` arming and skips the lifted preview (button-anchored presentation).
	let menuByTap = $state(false)
	// Viewport rect of the long-pressed row, so the context menu can lift it in place.
	let longPressRect = $state<LongPressRect | null>(null)

	// Narrow the latched target in the script (so the template keys off plain nullable values rather than
	// relying on in-template discriminated-union narrowing).
	const lpCategory = $derived(longPressTarget?.type === 'category' ? longPressTarget.category : null)
	const lpTag = $derived(longPressTarget?.type === 'tag' ? longPressTarget : null)

	function openRowMenu(target: LongPressTarget, rect: LongPressRect) {
		longPressRect = rect
		menuByTap = false
		longPressTarget = target
		rowActionsOpen = true
	}

	// A category's "…" button: opens the same menu as a long-press, but anchored to the button (no lifted
	// preview, and no synthesized click to swallow — the tap is a real click).
	function openCategoryMenu(e: MouseEvent, category: TagCategory) {
		void lightTap()
		const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
		longPressRect = { top: r.top, left: r.left, width: r.width, height: r.height }
		menuByTap = true
		longPressTarget = { type: 'category', category }
		rowActionsOpen = true
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
	function menuMoveTag() {
		const t = longPressTarget
		if (t?.type !== 'tag') return
		rowActionsOpen = false
		void lightTap()
		moveTarget = { tag: t.tag, category: t.category }
		movePickerOpen = true
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

<div class="relative flex h-full flex-col">
	<div
		bind:this={scrollEl}
		class="min-h-0 flex-1 overflow-y-auto overscroll-y-none"
		style="padding-bottom: var(--mini-player-inset, 0px)"
	>
		<!-- Glass toolbar: a trailing "+" that creates a category (capped at 4, matching desktop); adding tags
		     is done with the dashed "+" chip in each category below. Tags has no search, so the add sits alone
		     (the section title lives in the fixed top bar), but the bar still gives this tab the same header
		     material + separating line as the others. Hidden while the roster is empty — the empty state
		     carries its own CTA (FollowingView precedent). -->
		{#if categories.length > 0}
			<div class="glass flex items-center justify-end gap-2 border-b border-stroke-subtle px-3 py-2">
				<button
					type="button"
					class="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-md text-text-secondary active:bg-surface-2 disabled:opacity-40"
					aria-label={$translate('tags.newCategory')}
					disabled={categories.length >= MAX_CATEGORIES}
					onclick={openCreateCategory}
				>
					<svg class="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M12 5v14M5 12h14" stroke-linecap="round" />
					</svg>
				</button>
			</div>
		{/if}

		{#if categories.length === 0}
			<!-- Empty state: icon + explanation + the first step as its own CTA (mirrors FollowingView). -->
			<div class="flex flex-col items-center gap-2 px-4 py-10 text-center">
				<svg
					class="h-8 w-8 text-text-tertiary"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
				>
					<path
						d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A2 2 0 013 12V7a4 4 0 014-4z"
					/>
				</svg>
				<div class="text-sm font-medium text-text-primary">{$translate('tags.noTagCategoriesYet')}</div>
				<div class="max-w-xs text-xs text-text-tertiary">{$translate('tags.emptyHint')}</div>
				<button
					type="button"
					class="mt-2 rounded-lg bg-brand-muted px-4 py-2 text-sm font-medium text-brand-primary active:opacity-80"
					onclick={openCreateCategory}
				>
					{$translate('tags.newCategory')}
				</button>
			</div>
		{:else}
			{#each categories as category (category.id)}
				<div class="px-4 py-2" transition:slide={{ duration: 200, easing: easeFluid }}>
					<!-- Category header: name (long-press → manage) + a visible "…" button opening the same menu. -->
					<div class="mb-1 flex items-center justify-between gap-2">
						<h3
							class="min-w-0 flex-1 truncate text-sm font-medium text-text-secondary"
							use:longPress={{ onLongPress: (rect) => openRowMenu({ type: 'category', category }, rect) }}
						>
							{category.name}
						</h3>
						<button
							type="button"
							class="-my-1.5 flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-md text-text-tertiary active:bg-surface-2"
							aria-label={$translate('common.more')}
							aria-haspopup="menu"
							aria-expanded={rowActionsOpen && menuByTap && lpCategory?.id === category.id}
							onclick={(e) => openCategoryMenu(e, category)}
						>
							<svg class="h-5 w-5" viewBox="0 0 24 24" fill="currentColor">
								<circle cx="5" cy="12" r="1.8" />
								<circle cx="12" cy="12" r="1.8" />
								<circle cx="19" cy="12" r="1.8" />
							</svg>
						</button>
					</div>

					<!-- Tappable tag chips (tap → drill into the tag's feed; long-press → rename / move / delete) plus
					     a dashed "+" chip to add a tag to this category. -->
					<div class="flex flex-wrap gap-2">
						{#each category.tags as tag (tag.id)}
							{@const color = tag.color ?? category.color ?? DEFAULT_TAG_COLOR}
							<div
								use:longPress={{ onLongPress: (rect) => openRowMenu({ type: 'tag', tag, category }, rect) }}
								in:scale={{ duration: 180, start: 0.85, easing: easeFluid }}
								out:scale={{ duration: 140, start: 0.85, easing: easeFluid }}
							>
								<button
									type="button"
									class="inline-flex min-h-[44px] items-center rounded-lg px-3.5 text-sm font-medium active:opacity-70"
									style="background-color: {color}20; color: {color}; border: 1px solid {color}40;"
									onclick={() => openTag(tag.id)}
								>
									{tag.name}
								</button>
							</div>
						{/each}
						<button
							type="button"
							class="inline-flex min-h-[44px] min-w-[44px] items-center justify-center rounded-lg border border-dashed border-stroke px-3 text-text-tertiary active:bg-surface-2"
							aria-label={$translate('tags.newTag')}
							onclick={() => openCreateTag(category.id)}
						>
							<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<path d="M12 5v14M5 12h14" stroke-linecap="round" />
							</svg>
						</button>
						{#if category.tags.length === 0}
							<span
								class="inline-flex min-h-[44px] items-center text-xs text-text-tertiary"
								transition:fade={{ duration: 120 }}
							>
								{$translate('tags.noTags')}
							</span>
						{/if}
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

<!-- Category picker for a tag's "Move to Category" -->
<TagCategoryPickerSheet
	open={movePickerOpen}
	{categories}
	currentCategoryId={moveTarget?.category.id ?? null}
	onSelect={handleMoveSelect}
	onClose={() => (movePickerOpen = false)}
/>

<!-- Row context menu: category (add tag / rename / set color / delete — via the "…" button or a long-press)
     or tag (rename / move to category / delete — via a long-press). A long-press lifts the row as a preview;
     the "…" tap anchors the platter to the button instead (no preview, dismissal armed immediately). -->
{#snippet rowPreview()}
	{#if lpCategory}
		<span class="min-w-0 flex-1 truncate text-sm font-semibold text-text-primary">{lpCategory.name}</span>
	{:else if lpTag}
		<span
			class="block h-3 w-3 flex-shrink-0 rounded-full"
			style="background-color: {lpTag.tag.color ?? lpTag.category.color ?? DEFAULT_TAG_COLOR}"
		></span>
		<span class="min-w-0 flex-1 truncate text-sm text-text-primary">{lpTag.tag.name}</span>
	{/if}
{/snippet}

<ContextMenu
	open={rowActionsOpen}
	anchorRect={longPressRect}
	tapTriggered={menuByTap}
	preview={menuByTap ? undefined : rowPreview}
	onClose={() => (rowActionsOpen = false)}
	onClosed={() => {
		longPressTarget = null
		longPressRect = null
		menuByTap = false
	}}
>
	{#if lpCategory}
		<ContextMenuItem onclick={menuAddTag}>
			{$translate('tags.newTag')}
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
		{#if categories.length > 1}
			<ContextMenuItem onclick={menuMoveTag}>
				{$translate('tags.moveToCategory')}
				{#snippet icon()}
					<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path
							d="M5 19a2 2 0 01-2-2V7a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1M5 19h14a2 2 0 002-2v-5a2 2 0 00-2-2H9a2 2 0 00-2 2v5a2 2 0 01-2 2z"
							stroke-linecap="round"
							stroke-linejoin="round"
						/>
					</svg>
				{/snippet}
			</ContextMenuItem>
		{/if}
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
