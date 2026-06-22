<script lang="ts">
	// Spinning vinyl record — the mobile app's loading indicator (a nod to the crate-of-records logo).
	// The disc uses fixed "physical" colors: a record is black in either theme, so it must NOT invert,
	// and a neutral rim keeps it legible on dark surfaces. The centre label picks up the live accent via
	// --brand-primary. Concentric grooves are rotationally symmetric (so they wouldn't show motion) — the
	// off-centre groove tick and the shine wedge are what make the spin read. Decorative by default: the
	// surrounding control (button/row) owns the accessible label, so this is aria-hidden.
	type Props = {
		class?: string
	}
	let { class: className = 'h-4 w-4' }: Props = $props()
</script>

<svg class="{className} animate-spin" viewBox="0 0 100 100" aria-hidden="true">
	<circle cx="50" cy="50" r="47" fill="#1c1c1f" stroke="#52525b" stroke-width="4" />
	<circle cx="50" cy="50" r="38" fill="none" stroke="#ffffff" stroke-opacity="0.13" stroke-width="3" />
	<circle cx="50" cy="50" r="29" fill="none" stroke="#ffffff" stroke-opacity="0.13" stroke-width="3" />
	<path d="M50 50 L50 4 A46 46 0 0 1 81 16 Z" fill="#ffffff" fill-opacity="0.06" />
	<line
		x1="50"
		y1="22"
		x2="50"
		y2="5"
		stroke="#ffffff"
		stroke-opacity="0.5"
		stroke-width="3.5"
		stroke-linecap="round"
	/>
	<circle cx="50" cy="50" r="15" fill="var(--brand-primary)" />
	<circle cx="50" cy="50" r="15" fill="none" stroke="#ffffff" stroke-opacity="0.16" stroke-width="1.5" />
	<circle cx="50" cy="50" r="2.5" fill="#0a0a0a" />
</svg>
