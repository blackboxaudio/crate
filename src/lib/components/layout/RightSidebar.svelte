<script lang="ts">
	import type { Track } from '$lib/types'
	import { ResizeHandle } from '$lib/components/common'
	import { TrackEditor } from '$lib/components/editor'

	type Props = {
		selectedTracks: Track[]
		isVisible: boolean
		width: number
		onResize: (delta: number) => void
	}

	let { selectedTracks, isVisible, width, onResize }: Props = $props()

	// Resize state for disabling transitions during resize
	let isResizing = $state(false)

	// Snapshot of selected tracks that persists during close transition
	let editorTracks = $state<Track[]>([])

	// Whether sidebar should be open (visible and has tracks)
	let sidebarOpen = $derived(isVisible && selectedTracks.length > 0)

	// Sync editor tracks with selection when sidebar is open
	$effect(() => {
		if (sidebarOpen && selectedTracks.length > 0) {
			editorTracks = selectedTracks
		}
	})
</script>

<div
	class="flex h-full flex-shrink-0 overflow-hidden ease-out"
	class:transition-[width]={!isResizing}
	class:duration-250={!isResizing}
	class:animate-[fade-in_250ms_ease-out]={sidebarOpen}
	style="width: {sidebarOpen ? width : 0}px"
	ontransitionend={(e) => {
		if (e.propertyName === 'width' && !sidebarOpen) {
			editorTracks = []
		}
	}}
>
	<ResizeHandle
		onResize={(delta) => onResize(-delta)}
		onResizeStart={() => (isResizing = true)}
		onResizeEnd={() => (isResizing = false)}
	/>
	<div style="width: {width}px">
		<TrackEditor selectedTracks={editorTracks} />
	</div>
</div>
