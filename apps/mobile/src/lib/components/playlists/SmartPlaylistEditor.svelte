<script lang="ts">
	import { onMount } from 'svelte'
	import { slide } from 'svelte/transition'
	import { translate } from '$shared/i18n'
	import { tagsStore } from '$shared/stores/tags'
	import { previewSmartRulesCount } from '$shared/api/playlists'
	import {
		getFieldsForContext,
		getFieldDefinition,
		getOperatorsForType,
		operatorRequiresValue,
		operatorRequiresSecondValue,
		createDefaultCondition,
		conditionHasValue,
		parseSmartRules,
		type FieldDefinition,
	} from '$shared/utils/smartRules'
	import type { Playlist, SmartRules, SmartCondition, MatchMode, ActiveView } from '$shared/types'
	import { DEFAULT_TAG_COLOR } from '$shared/types'
	import FormSheet from '$lib/components/common/FormSheet.svelte'

	// Mobile smart-playlist rule editor. A focused, touch-first counterpart to the desktop SmartPlaylistModal:
	// name + match-mode + a stacked list of conditions (field / operator / value), with a live match-count
	// preview. Field / operator / enum pickers are native <select>s (iOS renders its wheel picker, the best
	// mobile UX and no custom dropdown to build).
	//
	// Presented on the shared FormSheet: Cancel / Create live in its top nav bar (above the keyboard), the
	// name field sits near the top so it's never covered when the keyboard rises, and dismissal is guarded
	// by the discard confirm while edits exist.
	type Props = {
		open: boolean
		context?: ActiveView
		/** When set, the editor opens prefilled to edit this smart playlist instead of creating one. */
		playlist?: Playlist | null
		onSubmit: (name: string, rules: SmartRules) => void
		onCancel: () => void
	}
	let { open, context = 'discovery', playlist = null, onSubmit, onCancel }: Props = $props()

	let name = $state('')
	let matchMode = $state<MatchMode>('all')
	let conditions = $state<SmartCondition[]>([])
	let previewCount = $state<number | null>(null)
	let previewLoading = $state(false)
	let previewTimer: ReturnType<typeof setTimeout> | undefined
	// The mobile editor has no limit UI; carry an existing (desktop-authored) limit through an edit
	// untouched instead of silently stripping it on save.
	let initialLimit: SmartRules['limit']

	const tagCategories = $derived($tagsStore.categories)
	const fields = $derived(getFieldsForContext(context))
	const isEditing = $derived(!!playlist)
	const title = $derived(isEditing ? $translate('smartPlaylist.editTitle') : $translate('smartPlaylist.createTitle'))
	const canSubmit = $derived(name.trim().length > 0 && conditions.length > 0 && conditions.every(conditionHasValue))

	// Keyboard clearance is handled by the shared FormSheet (fields sit high, actions in the top nav bar),
	// so no per-editor visualViewport spacer is needed here.
	onMount(() => {
		if (tagCategories.length === 0) tagsStore.load()
	})

	// Snapshot of the editable state, for the FormSheet's discard guard: dirty = anything moved since open.
	const snapshot = () => JSON.stringify({ name, matchMode, conditions })
	let initialSnapshot = $state('')
	const dirty = $derived(snapshot() !== initialSnapshot)

	// Reset (create) or prefill (edit) whenever the editor (re)opens.
	$effect(() => {
		if (open) {
			const parsed = playlist ? parseSmartRules(playlist.smart_rules) : null
			if (playlist && parsed) {
				name = playlist.name
				matchMode = parsed.match_mode
				// Clone before feeding $state — the per-index condition updates must not mutate the
				// store's parsed objects.
				conditions = structuredClone(parsed.conditions)
				initialLimit = parsed.limit
			} else {
				name = playlist?.name ?? ''
				matchMode = 'all'
				conditions = []
				initialLimit = undefined
			}
			previewCount = null
			initialSnapshot = snapshot()
		}
	})

	// Debounced live count of matching releases.
	$effect(() => {
		const _deps = [matchMode, JSON.stringify(conditions)]
		void _deps
		if (previewTimer) clearTimeout(previewTimer)
		if (!open || conditions.length === 0) {
			previewCount = null
			return
		}
		previewTimer = setTimeout(async () => {
			const complete = conditions.filter(conditionHasValue)
			if (complete.length === 0) {
				previewCount = null
				return
			}
			previewLoading = true
			try {
				previewCount = await previewSmartRulesCount({ match_mode: matchMode, conditions: complete }, context)
			} catch {
				previewCount = null
			} finally {
				previewLoading = false
			}
		}, 300)
		return () => clearTimeout(previewTimer)
	})

	function handleSubmit() {
		if (!canSubmit) return
		onSubmit(name.trim(), { match_mode: matchMode, conditions, ...(initialLimit ? { limit: initialLimit } : {}) })
	}

	function addCondition() {
		conditions = [...conditions, createDefaultCondition(fields[0])]
	}

	function removeCondition(index: number) {
		conditions = conditions.filter((_, i) => i !== index)
	}

	function updateField(index: number, fieldName: string) {
		const def = getFieldDefinition(fieldName, context)
		if (def) conditions[index] = createDefaultCondition(def)
	}

	function updateOperator(index: number, operator: string) {
		conditions[index] = { ...conditions[index], operator: operator as never }
	}

	function updateValue(index: number, value: string) {
		const c = conditions[index]
		if (c.type === 'numeric') {
			conditions[index] = { ...c, value: value ? parseFloat(value) : undefined }
		} else if (c.type !== 'tags') {
			conditions[index] = { ...c, value: value || undefined } as SmartCondition
		}
	}

	function updateValue2(index: number, value: string) {
		const c = conditions[index]
		if (c.type === 'numeric') conditions[index] = { ...c, value2: value ? parseFloat(value) : undefined }
	}

	function toggleTag(index: number, tagId: string) {
		const c = conditions[index]
		if (c.type !== 'tags') return
		const tag_ids = c.tag_ids.includes(tagId) ? c.tag_ids.filter((id) => id !== tagId) : [...c.tag_ids, tagId]
		conditions[index] = { ...c, tag_ids }
	}

	function fieldOf(c: SmartCondition): string {
		return c.type === 'tags' ? 'tags' : c.field
	}

	function operatorOptions(c: SmartCondition) {
		return getOperatorsForType(c.type as FieldDefinition['type'])
	}
</script>

<FormSheet
	{open}
	{title}
	onClose={onCancel}
	onSubmit={handleSubmit}
	submitLabel={isEditing ? $translate('common.save') : $translate('common.create')}
	submitDisabled={!canSubmit}
	{dirty}
>
	<div class="flex flex-col gap-5 px-4 py-4">
		<!-- Name -->
		<input
			type="text"
			bind:value={name}
			placeholder={$translate('smartPlaylist.namePlaceholder')}
			autocapitalize="words"
			autocorrect="off"
			class="w-full rounded-lg border border-stroke bg-surface-1 px-3 py-2.5 text-sm text-text-primary placeholder:text-text-tertiary focus:border-brand-primary focus:outline-none"
		/>

		<!-- Match mode segmented control -->
		<div class="flex items-center justify-between gap-3">
			<span class="text-sm text-text-secondary">{$translate('smartPlaylist.matchLabel')}</span>
			<div class="relative flex rounded-lg bg-surface-2 p-0.5">
				<div
					class="absolute top-0.5 bottom-0.5 left-0.5 w-[calc(50%-0.125rem)] rounded-md bg-brand-primary transition-transform duration-200 ease-out"
					style="transform: translateX({matchMode === 'any' ? '100%' : '0%'})"
				></div>
				<button
					type="button"
					class="relative z-10 px-4 py-1.5 text-sm font-medium {matchMode === 'all'
						? 'text-white'
						: 'text-text-secondary'}"
					onclick={() => (matchMode = 'all')}
				>
					{$translate('smartPlaylist.matchAll')}
				</button>
				<button
					type="button"
					class="relative z-10 px-4 py-1.5 text-sm font-medium {matchMode === 'any'
						? 'text-white'
						: 'text-text-secondary'}"
					onclick={() => (matchMode = 'any')}
				>
					{$translate('smartPlaylist.matchAny')}
				</button>
			</div>
		</div>

		<!-- Conditions -->
		<div class="flex flex-col gap-3">
			{#if conditions.length === 0}
				<p class="rounded-lg border border-dashed border-stroke px-4 py-6 text-center text-sm text-text-tertiary">
					{$translate('smartPlaylist.noConditions')}
				</p>
			{/if}

			{#each conditions as condition, index (index)}
				<div
					class="flex flex-col gap-2 rounded-lg border border-stroke bg-surface-1 p-3"
					transition:slide={{ duration: 180 }}
				>
					<div class="flex items-center gap-2">
						<select
							value={fieldOf(condition)}
							onchange={(e) => updateField(index, e.currentTarget.value)}
							class="min-w-0 flex-1 rounded-md border border-stroke bg-surface-0 px-2 py-2 text-sm text-text-primary"
						>
							{#each fields as f (f.field)}
								<option value={f.field}>{$translate(f.labelKey)}</option>
							{/each}
						</select>
						<button
							type="button"
							class="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-md text-text-tertiary active:bg-surface-2"
							aria-label={$translate('smartPlaylist.removeCondition')}
							onclick={() => removeCondition(index)}
						>
							<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<path d="M18 6L6 18M6 6l12 12" stroke-linecap="round" />
							</svg>
						</button>
					</div>

					<select
						value={condition.operator}
						onchange={(e) => updateOperator(index, e.currentTarget.value)}
						class="w-full rounded-md border border-stroke bg-surface-0 px-2 py-2 text-sm text-text-primary"
					>
						{#each operatorOptions(condition) as op (op.value)}
							<option value={op.value}>{$translate(op.labelKey)}</option>
						{/each}
					</select>

					<!-- Value -->
					{#if operatorRequiresValue(condition.operator)}
						{#if condition.type === 'tags'}
							<div class="flex flex-wrap gap-1.5 pt-1">
								{#each tagCategories as category (category.id)}
									{#each category.tags as tag (tag.id)}
										{@const color = tag.color ?? category.color ?? DEFAULT_TAG_COLOR}
										{@const selected = condition.type === 'tags' && condition.tag_ids.includes(tag.id)}
										<button
											type="button"
											class="rounded-full px-3 py-1 text-xs font-medium transition-colors"
											style={selected
												? `background-color:${color};color:#fff;border:1px solid ${color};`
												: `background-color:${color}1a;color:${color};border:1px solid ${color}40;`}
											onclick={() => toggleTag(index, tag.id)}
										>
											{tag.name}
										</button>
									{/each}
								{/each}
							</div>
						{:else if condition.type === 'enum'}
							<select
								value={condition.value ?? ''}
								onchange={(e) => updateValue(index, e.currentTarget.value)}
								class="w-full rounded-md border border-stroke bg-surface-0 px-2 py-2 text-sm text-text-primary"
							>
								{#each getFieldDefinition(condition.field, context)?.enumValues ?? [] as ev (ev.value)}
									<option value={ev.value}>{ev.labelKey.includes('.') ? $translate(ev.labelKey) : ev.labelKey}</option>
								{/each}
							</select>
						{:else if condition.type === 'numeric'}
							<div class="flex items-center gap-2">
								<input
									type="number"
									inputmode="numeric"
									value={condition.value ?? ''}
									oninput={(e) => updateValue(index, e.currentTarget.value)}
									class="min-w-0 flex-1 rounded-md border border-stroke bg-surface-0 px-2 py-2 text-sm text-text-primary"
								/>
								{#if operatorRequiresSecondValue(condition.operator)}
									<span class="text-text-tertiary">–</span>
									<input
										type="number"
										inputmode="numeric"
										value={condition.value2 ?? ''}
										oninput={(e) => updateValue2(index, e.currentTarget.value)}
										class="min-w-0 flex-1 rounded-md border border-stroke bg-surface-0 px-2 py-2 text-sm text-text-primary"
									/>
								{/if}
							</div>
						{:else if condition.type === 'date' && (condition.operator === 'before' || condition.operator === 'after')}
							<input
								type="date"
								value={condition.value ?? ''}
								oninput={(e) => updateValue(index, e.currentTarget.value)}
								class="w-full rounded-md border border-stroke bg-surface-0 px-2 py-2 text-sm text-text-primary"
							/>
						{:else if condition.type === 'date'}
							<input
								type="number"
								inputmode="numeric"
								placeholder="30"
								value={condition.value ?? ''}
								oninput={(e) => updateValue(index, e.currentTarget.value)}
								class="w-full rounded-md border border-stroke bg-surface-0 px-2 py-2 text-sm text-text-primary"
							/>
						{:else}
							<input
								type="text"
								value={condition.type === 'text' ? (condition.value ?? '') : ''}
								oninput={(e) => updateValue(index, e.currentTarget.value)}
								class="w-full rounded-md border border-stroke bg-surface-0 px-2 py-2 text-sm text-text-primary"
							/>
						{/if}
					{/if}
				</div>
			{/each}

			<button
				type="button"
				class="flex items-center justify-center gap-1.5 rounded-lg border border-dashed border-stroke py-2.5 text-sm font-medium text-brand-primary active:bg-surface-2"
				onclick={addCondition}
			>
				<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M12 5v14M5 12h14" stroke-linecap="round" />
				</svg>
				{$translate('smartPlaylist.addCondition')}
			</button>
		</div>

		<!-- Live preview count -->
		{#if conditions.length > 0}
			<p class="text-center text-xs text-text-secondary">
				{#if previewLoading}
					…
				{:else if previewCount !== null}
					{$translate('smartPlaylist.preview', { values: { count: previewCount } })}
				{/if}
			</p>
		{/if}
	</div>
</FormSheet>
