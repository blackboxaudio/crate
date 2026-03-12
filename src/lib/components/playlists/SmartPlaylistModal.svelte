<script lang="ts">
	import Modal from '$lib/components/common/Modal.svelte'
	import Button from '$lib/components/common/Button.svelte'
	import Input from '$lib/components/common/Input.svelte'
	import Select from '$lib/components/common/Select.svelte'
	import Icon from '$lib/components/common/Icon.svelte'
	import Text from '$lib/components/common/Text.svelte'
	import Checkbox from '$lib/components/common/Checkbox.svelte'
	import { slide } from 'svelte/transition'
	import { translate } from '$lib/i18n'
	import { previewSmartRulesCount } from '$lib/api/playlists'
	import {
		getFieldsForContext,
		getFieldDefinition,
		getOperatorsForType,
		operatorRequiresValue,
		operatorRequiresSecondValue,
		createDefaultCondition,
		conditionHasValue,
		getSortFieldsForContext,
		findDeletedTagIds,
		type FieldDefinition,
	} from '$lib/utils/smartRules'
	import type {
		SmartRules,
		SmartCondition,
		MatchMode,
		TagCategory,
		SmartSortDirection,
		ActiveView,
		Playlist,
	} from '$lib/types'

	type Props = {
		open: boolean
		context: ActiveView
		playlist?: Playlist
		initialRules?: SmartRules
		tagCategories: TagCategory[]
		onSubmit: (name: string, rules: SmartRules) => void
		onCancel: () => void
	}

	let { open, context, playlist, initialRules, tagCategories, onSubmit, onCancel }: Props = $props()

	let name = $state('')
	let matchMode: MatchMode = $state('all')
	let conditions: SmartCondition[] = $state([])
	let limitEnabled = $state(false)
	let limitCount = $state(25)
	let limitSortField = $state('date_added')
	let limitSortDirection: SmartSortDirection = $state('descending')
	let previewCount: number | null = $state(null)
	let previewLoading = $state(false)
	let previewTimeout: ReturnType<typeof setTimeout> | undefined

	const isEditing = $derived(!!playlist)
	const title = $derived(isEditing ? $translate('smartPlaylist.editTitle') : $translate('smartPlaylist.createTitle'))
	const canSubmit = $derived(name.trim().length > 0 && conditions.length > 0 && conditions.every(conditionHasValue))

	// Reset state when modal opens
	$effect(() => {
		if (open) {
			if (playlist && initialRules) {
				name = playlist.name
				matchMode = initialRules.match_mode
				conditions = structuredClone(initialRules.conditions)
				limitEnabled = !!initialRules.limit
				if (initialRules.limit) {
					limitCount = initialRules.limit.count
					limitSortField = initialRules.limit.sort_field
					limitSortDirection = initialRules.limit.sort_direction
				}
			} else {
				name = ''
				matchMode = 'all'
				conditions = []
				limitEnabled = false
				limitCount = 25
				limitSortField = 'date_added'
				limitSortDirection = 'descending'
			}
			previewCount = null
		}
	})

	// Debounced preview count
	$effect(() => {
		// Track dependencies
		const _deps = [matchMode, JSON.stringify(conditions), limitEnabled, limitCount, limitSortField, limitSortDirection]
		void _deps

		if (previewTimeout) clearTimeout(previewTimeout)
		if (!open || conditions.length === 0) {
			previewCount = null
			return
		}

		previewTimeout = setTimeout(async () => {
			previewLoading = true
			try {
				const completeConditions = conditions.filter(conditionHasValue)
				if (completeConditions.length === 0) {
					previewCount = null
				} else {
					const rules: SmartRules = {
						...buildRules(),
						conditions: completeConditions,
					}
					previewCount = await previewSmartRulesCount(rules, context)
				}
			} catch {
				previewCount = null
			} finally {
				previewLoading = false
			}
		}, 300)
	})

	function buildRules(): SmartRules {
		return {
			match_mode: matchMode,
			conditions,
			limit: limitEnabled
				? { count: limitCount, sort_field: limitSortField, sort_direction: limitSortDirection }
				: undefined,
		}
	}

	function handleSubmit() {
		if (!canSubmit) return
		onSubmit(name.trim(), buildRules())
	}

	function handleCancel() {
		onCancel()
	}

	function addCondition() {
		const fields = getFieldsForContext(context)
		const firstField = fields[0]
		conditions = [...conditions, createDefaultCondition(firstField)]
	}

	function removeCondition(index: number) {
		conditions = conditions.filter((_, i) => i !== index)
	}

	function updateConditionField(index: number, fieldName: string) {
		const fieldDef = getFieldDefinition(fieldName, context)
		if (!fieldDef) return

		// If the field type is the same as the tag type, just update the field
		if (fieldDef.type === 'tags') {
			conditions[index] = { type: 'tags', operator: 'has_any', tag_ids: [] }
		} else {
			conditions[index] = createDefaultCondition(fieldDef)
		}
	}

	function getConditionField(condition: SmartCondition): string {
		if (condition.type === 'tags') return 'tags'
		return condition.field
	}

	function getConditionOperator(condition: SmartCondition): string {
		return condition.operator
	}

	function updateConditionOperator(index: number, operator: string) {
		const condition = conditions[index]
		conditions[index] = { ...condition, operator: operator as never }
	}

	function updateConditionValue(index: number, value: string) {
		const condition = conditions[index]
		if (condition.type === 'numeric') {
			conditions[index] = { ...condition, value: value ? parseFloat(value) : undefined }
		} else if (condition.type === 'tags') {
			// Tags handled separately
		} else {
			conditions[index] = { ...condition, value: value || undefined } as SmartCondition
		}
	}

	function updateConditionValue2(index: number, value: string) {
		const condition = conditions[index]
		if (condition.type === 'numeric') {
			conditions[index] = { ...condition, value2: value ? parseFloat(value) : undefined }
		}
	}

	function toggleTag(index: number, tagId: string) {
		const condition = conditions[index]
		if (condition.type !== 'tags') return
		const tagIds = condition.tag_ids.includes(tagId)
			? condition.tag_ids.filter((id) => id !== tagId)
			: [...condition.tag_ids, tagId]
		conditions[index] = { ...condition, tag_ids: tagIds }
	}

	function getFieldOptions(ctx: string) {
		return getFieldsForContext(ctx).map((f) => ({
			value: f.field,
			label: $translate(f.labelKey),
		}))
	}

	function getOperatorOptions(fieldType: string) {
		return getOperatorsForType(fieldType as FieldDefinition['type']).map((o) => ({
			value: o.value,
			label: $translate(o.labelKey),
		}))
	}

	function getConditionFieldType(condition: SmartCondition): string {
		return condition.type
	}

	function getEnumOptions(condition: SmartCondition) {
		if (condition.type !== 'enum') return []
		const fieldDef = getFieldDefinition(condition.field, context)
		return (fieldDef?.enumValues ?? []).map((ev) => ({
			value: ev.value,
			label: ev.labelKey.includes('.') ? $translate(ev.labelKey) : ev.labelKey,
		}))
	}
</script>

<Modal {open} {title} size="lg" onClose={handleCancel}>
	<div class="flex flex-col gap-4">
		<!-- Name input -->
		<div>
			<Text variant="caption" weight="medium" class="mb-1">{$translate('smartPlaylist.name')}</Text>
			<Input bind:value={name} placeholder={$translate('smartPlaylist.namePlaceholder')} autofocus />
		</div>

		<!-- Match mode -->
		<div class="flex items-center gap-2">
			<Text variant="body-2">{$translate('smartPlaylist.matchLabel')}</Text>
			<div class="relative flex rounded-md border border-stroke bg-surface-2">
				<div
					class="absolute top-0 left-0 h-full w-1/2 rounded-md bg-brand-primary transition-transform duration-200 ease-out"
					style="transform: translateX({matchMode === 'any' ? '100%' : '0%'})"
				></div>
				<button
					type="button"
					class="relative z-10 cursor-pointer px-3 py-1 text-sm transition-colors {matchMode === 'all'
						? 'text-white'
						: 'text-text-secondary hover:text-text-primary'}"
					onclick={() => (matchMode = 'all')}
				>
					{$translate('smartPlaylist.matchAll')}
				</button>
				<button
					type="button"
					class="relative z-10 cursor-pointer px-3 py-1 text-sm transition-colors {matchMode === 'any'
						? 'text-white'
						: 'text-text-secondary hover:text-text-primary'}"
					onclick={() => (matchMode = 'any')}
				>
					{$translate('smartPlaylist.matchAny')}
				</button>
			</div>
		</div>

		<!-- Conditions -->
		<div class="flex flex-col gap-2">
			{#if conditions.length === 0}
				<div class="rounded-md border border-dashed border-stroke px-4 py-6 text-center">
					<Text variant="body-2" class="text-text-tertiary">{$translate('smartPlaylist.noConditions')}</Text>
				</div>
			{/if}

			{#each conditions as condition, index (index)}
				{@const fieldType = getConditionFieldType(condition)}
				{@const conditionField = getConditionField(condition)}
				{@const conditionOperator = getConditionOperator(condition)}
				<div class="flex items-start gap-2 rounded-md border border-stroke bg-surface-2 p-2">
					<!-- Field select -->
					<Select
						value={conditionField}
						options={getFieldOptions(context)}
						class="w-36 shrink-0"
						onchange={(val) => updateConditionField(index, val)}
					/>

					<!-- Operator select -->
					<Select
						value={conditionOperator}
						options={getOperatorOptions(fieldType)}
						class="w-36 shrink-0"
						onchange={(val) => updateConditionOperator(index, val)}
					/>

					<!-- Value input -->
					{#if fieldType === 'tags'}
						{#if operatorRequiresValue(conditionOperator)}
							<div class="flex min-w-0 flex-1 flex-wrap gap-1">
								{#each tagCategories as category, i (i)}
									{#each category.tags as tag, i (i)}
										{@const isSelected = condition.type === 'tags' && condition.tag_ids.includes(tag.id)}
										{@const isDeleted =
											condition.type === 'tags' && findDeletedTagIds([tag.id], tagCategories).length > 0}
										<button
											type="button"
											class="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs transition-colors
												{isSelected ? 'bg-brand-primary text-white' : 'bg-surface-1 text-text-secondary hover:bg-surface-2'}"
											onclick={() => toggleTag(index, tag.id)}
										>
											{#if isDeleted}
												<Icon name="warning" class="h-3 w-3 text-amber-500" />
											{/if}
											{tag.name}
										</button>
									{/each}
								{/each}
							</div>
						{/if}
					{:else if fieldType === 'enum'}
						{#if operatorRequiresValue(conditionOperator)}
							<Select
								value={condition.type === 'enum' ? (condition.value ?? '') : ''}
								options={getEnumOptions(condition)}
								class="min-w-0 flex-1"
								onchange={(val) => updateConditionValue(index, val)}
							/>
						{/if}
					{:else if fieldType === 'numeric'}
						{#if operatorRequiresValue(conditionOperator)}
							<Input
								type="number"
								value={condition.type === 'numeric' ? String(condition.value ?? '') : ''}
								class="min-w-0 flex-1"
								oninput={(e) => updateConditionValue(index, (e.target as HTMLInputElement).value)}
							/>
							{#if operatorRequiresSecondValue(conditionOperator)}
								<Text variant="body-2" class="self-center text-text-tertiary">&ndash;</Text>
								<Input
									type="number"
									value={condition.type === 'numeric' ? String(condition.value2 ?? '') : ''}
									class="min-w-0 flex-1"
									oninput={(e) => updateConditionValue2(index, (e.target as HTMLInputElement).value)}
								/>
							{/if}
						{/if}
					{:else if operatorRequiresValue(conditionOperator)}
						<Input
							value={condition.type === 'text' || condition.type === 'date' ? (condition.value ?? '') : ''}
							class="min-w-0 flex-1"
							placeholder={fieldType === 'date' &&
							(conditionOperator === 'in_last_days' || conditionOperator === 'not_in_last_days')
								? '30'
								: ''}
							oninput={(e) => updateConditionValue(index, (e.target as HTMLInputElement).value)}
						/>
					{/if}

					<!-- Remove button -->
					<button
						type="button"
						class="shrink-0 self-center rounded p-1 text-text-tertiary transition-colors hover:cursor-pointer hover:bg-surface-1"
						onclick={() => removeCondition(index)}
						aria-label={$translate('smartPlaylist.removeCondition')}
					>
						<Icon name="x" class="h-4 w-4" />
					</button>
				</div>
			{/each}

			<!-- Add condition -->
			<Button variant="ghost" size="sm" class="w-full justify-center" onclick={addCondition}>
				<Icon name="plus" class="mr-1.5 h-3.5 w-3.5" />
				{$translate('smartPlaylist.addCondition')}
			</Button>
		</div>

		<!-- Limit -->
		<div class="flex flex-col gap-2 rounded-md border border-stroke p-3">
			<Checkbox bind:checked={limitEnabled} label={$translate('smartPlaylist.limit')} />
			{#if limitEnabled}
				<div class="flex items-center gap-2" transition:slide={{ duration: 200 }}>
					<div class="w-20 shrink-0">
						<Input
							type="number"
							value={String(limitCount)}
							oninput={(e) => {
								const val = parseInt((e.target as HTMLInputElement).value)
								if (!isNaN(val) && val > 0) limitCount = val
							}}
						/>
					</div>
					<Text variant="body-2" class="shrink-0 whitespace-nowrap text-text-secondary"
						>{$translate('smartPlaylist.limitItems')}</Text
					>
					<Text variant="body-2" class="shrink-0 whitespace-nowrap text-text-secondary"
						>{$translate('smartPlaylist.sortBy')}</Text
					>
					<Select
						value={limitSortField}
						options={getSortFieldsForContext(context).map((f) => ({
							value: f.value,
							label: $translate(f.labelKey),
						}))}
						class="min-w-0 flex-1"
						onchange={(val) => (limitSortField = val)}
					/>
					<Select
						value={limitSortDirection}
						options={[
							{ value: 'ascending', label: $translate('smartPlaylist.ascending') },
							{ value: 'descending', label: $translate('smartPlaylist.descending') },
						]}
						class="min-w-0 flex-1"
						onchange={(val) => (limitSortDirection = val as SmartSortDirection)}
					/>
				</div>
			{/if}
		</div>

		<!-- Preview count -->
		{#if conditions.length > 0}
			<div class="text-center">
				{#if previewLoading}
					<Text variant="caption" class="text-text-tertiary">...</Text>
				{:else if previewCount !== null}
					<Text variant="caption" class="text-text-secondary">
						{$translate('smartPlaylist.preview', { values: { count: previewCount } })}
					</Text>
				{/if}
			</div>
		{/if}
	</div>

	{#snippet footer()}
		<Button variant="ghost" onclick={handleCancel}>{$translate('common.cancel')}</Button>
		<Button variant="primary" onclick={handleSubmit} disabled={!canSubmit}>
			{isEditing ? $translate('common.save') : $translate('common.create')}
		</Button>
	{/snippet}
</Modal>
