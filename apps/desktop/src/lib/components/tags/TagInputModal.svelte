<script lang="ts">
	import type { TagCategory } from '$shared/types'
	import { translate } from '$shared/i18n'
	import Modal from '$lib/components/common/Modal.svelte'
	import Select from '$lib/components/common/Select.svelte'
	import Input from '$lib/components/common/Input.svelte'
	import Button from '$lib/components/common/Button.svelte'
	import Text from '$lib/components/common/Text.svelte'

	type Props = {
		open: boolean
		categories: TagCategory[]
		onSubmit: (categoryId: string, tagName: string) => void
		onCancel: () => void
	}

	let { open, categories, onSubmit, onCancel }: Props = $props()

	let selectedCategoryId = $state('')
	let tagName = $state('')

	// Build select options from categories
	const categoryOptions = $derived(
		categories.map((c) => ({
			value: c.id,
			label: c.name,
		}))
	)

	// Reset values when modal opens
	$effect(() => {
		if (open) {
			selectedCategoryId = categories.length > 0 ? categories[0].id : ''
			tagName = ''
		}
	})

	const canSubmit = $derived(selectedCategoryId && tagName.trim())

	function handleSubmit() {
		if (canSubmit) {
			onSubmit(selectedCategoryId, tagName.trim())
			selectedCategoryId = ''
			tagName = ''
		}
	}

	function handleCancel() {
		selectedCategoryId = ''
		tagName = ''
		onCancel()
	}
</script>

<Modal {open} title={$translate('modals.createTag.title')} onClose={handleCancel} onSubmit={handleSubmit}>
	<div class="space-y-4">
		<div>
			<Text as="label" variant="body-2" color="secondary" class="mb-1.5 block" for="tag-category"
				>{$translate('modals.createTag.categoryLabel')}</Text
			>
			<Select
				bind:value={selectedCategoryId}
				options={categoryOptions}
				placeholder={$translate('modals.createTag.categoryPlaceholder')}
			/>
		</div>
		<div>
			<Text as="label" variant="body-2" color="secondary" class="mb-1.5 block" for="tag-name"
				>{$translate('modals.createTag.tagLabel')}</Text
			>
			<Input bind:value={tagName} placeholder={$translate('modals.createTag.tagPlaceholder')} autofocus />
		</div>
	</div>

	{#snippet footer()}
		<Button variant="ghost" onclick={handleCancel}>{$translate('common.cancel')}</Button>
		<Button variant="primary" onclick={handleSubmit} disabled={!canSubmit}>{$translate('common.create')}</Button>
	{/snippet}
</Modal>
