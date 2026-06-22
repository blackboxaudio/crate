<!-- Branded top bar: the accent-masked Crate logo + wordmark, pinned with the top safe-area inset so
     the status bar / notch never overlaps it. Navigation now lives in the bottom TabBar, so this bar
     is purely brand identity (no menu / settings buttons). -->
<header class="pt-safe fixed inset-x-0 top-0 z-30 border-b border-stroke-subtle bg-surface-1">
	<div class="flex h-14 items-center gap-2.5 px-4">
		<!-- The Crate logo, accent-masked so it tracks the active accent (mirrors the desktop Sidebar
		     and the splash brand treatment). The SVG wraps an alpha-only raster, so mask it rather than
		     rendering it as an <img>. -->
		<div
			class="h-7 w-7 bg-brand-primary"
			style="-webkit-mask-image: url('/crate-logo.svg'); -webkit-mask-size: contain; -webkit-mask-repeat: no-repeat; -webkit-mask-position: center; mask-image: url('/crate-logo.svg'); mask-size: contain; mask-repeat: no-repeat; mask-position: center;"
		></div>
		<span class="text-lg font-semibold tracking-tight text-text-primary">Crate</span>
	</div>
</header>
