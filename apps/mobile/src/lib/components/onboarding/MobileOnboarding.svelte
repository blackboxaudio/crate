<script lang="ts">
	import { get } from 'svelte/store'
	import { fly, fade } from 'svelte/transition'
	import { cubicOut } from 'svelte/easing'
	import type { Theme, AccentColor } from '$shared/types'
	import { translate } from '$shared/i18n'
	import { isSyncAvailable, signingIn, cloudSyncError, isSignedIn, cloudSyncStore } from '$shared/stores/cloudSync'
	import { isIOS } from '$shared/utils/platform'
	import { settingsStore, theme, accentColor } from '$shared/stores/settings'
	import { signInMobile } from '$lib/signInMobile'
	import { onboardingComplete } from '$lib/stores/onboarding'
	import Spinner from '$lib/components/common/Spinner.svelte'

	// First-run onboarding: a one-time, dismissible carousel that introduces the standalone-first mobile
	// flow (add a release, preview it), offers an OPTIONAL sign-in to sync with the desktop, and lets the
	// user pick a theme + accent. No step is required — Skip / Get Started / Maybe later all complete it
	// without any sync setup. Mirrors the desktop OnboardingWizard's fly-between-steps pattern, mobile-styled.

	type Step = 'welcome' | 'discover' | 'sync' | 'appearance'

	// The sync step only appears when cloud sync is configured on this build. The appearance step is always
	// last EXCEPT when sign-in restores an existing account's settings from another device (see signIn()),
	// in which case we finish straight from the sync step so we don't ask the user to re-pick what they've
	// already chosen elsewhere.
	const steps: Step[] = $derived(
		$isSyncAvailable ? ['welcome', 'discover', 'sync', 'appearance'] : ['welcome', 'discover', 'appearance']
	)

	let current = $state(0)
	let direction = $state(1) // 1 = forward, -1 = back

	const step = $derived(steps[current])
	const isLast = $derived(current === steps.length - 1)

	const themeOptions: { value: Theme; labelKey: string }[] = [
		{ value: 'light', labelKey: 'settings.appearance.themeLight' },
		{ value: 'dark', labelKey: 'settings.appearance.themeDark' },
		{ value: 'system', labelKey: 'settings.appearance.themeSystem' },
	]

	const accentColors: { value: AccentColor; hex: string; labelKey: string }[] = [
		{ value: 'blue', hex: '#3b82f6', labelKey: 'colors.blue' },
		{ value: 'indigo', hex: '#6366f1', labelKey: 'colors.indigo' },
		{ value: 'violet', hex: '#8b5cf6', labelKey: 'colors.violet' },
		{ value: 'purple', hex: '#a855f7', labelKey: 'colors.purple' },
		{ value: 'pink', hex: '#ec4899', labelKey: 'colors.pink' },
		{ value: 'rose', hex: '#f43f5e', labelKey: 'colors.rose' },
		{ value: 'orange', hex: '#f97316', labelKey: 'colors.orange' },
		{ value: 'amber', hex: '#f59e0b', labelKey: 'colors.amber' },
		{ value: 'emerald', hex: '#10b981', labelKey: 'colors.emerald' },
		{ value: 'teal', hex: '#14b8a6', labelKey: 'colors.teal' },
	]

	function next() {
		if (current < steps.length - 1) {
			direction = 1
			current++
		} else {
			finish()
		}
	}

	function back() {
		if (current > 0) {
			direction = -1
			current--
		}
	}

	function finish() {
		onboardingComplete.complete()
	}

	// Kick off the native OAuth flow. On success the store's status carries an `onboarding` marker:
	// `restore` = this account already has a vault on another device, so its settings (theme/accent) sync
	// down — skip the appearance step and finish. `initial`/null = brand-new account (even if it's their
	// first-ever sign-in), so there's nothing to restore and we still let them pick a look. On failure we
	// stay on the sync step so they can retry or tap "Maybe later".
	async function signIn() {
		await signInMobile('google')
		const state = get(cloudSyncStore)
		if (state.error) return
		if (state.status.onboarding === 'restore') finish()
		else next()
	}

	// iOS-only: the same post-sign-in navigation as signIn(), but via the native Apple flow. A
	// dismissed Apple sheet clears the spinner without an error, so gate on `isSignedIn` (which also
	// covers the error case) rather than only `state.error`, so a cancel doesn't advance the step.
	async function signInWithApple() {
		await cloudSyncStore.signInApple()
		if (!get(isSignedIn)) return
		if (get(cloudSyncStore).status.onboarding === 'restore') finish()
		else next()
	}
</script>

<div
	class="fixed inset-0 z-40 flex flex-col bg-surface-0"
	in:fade={{ duration: 300, easing: cubicOut }}
	out:fade={{ duration: 300, easing: cubicOut }}
>
	<!-- Skip (always available — onboarding is never required). -->
	<div class="pt-safe flex justify-end px-5">
		<button type="button" class="px-3 py-3 text-sm font-medium text-text-tertiary active:opacity-70" onclick={finish}>
			{$translate('onboarding.mobile.skip')}
		</button>
	</div>

	<!-- Step content -->
	<div class="relative flex flex-1 items-center justify-center overflow-hidden px-8">
		{#key current}
			<div
				class="absolute flex w-full max-w-sm flex-col items-center gap-6 text-center"
				in:fly={{ x: direction * 240, duration: 300, easing: cubicOut }}
				out:fly={{ x: direction * -240, duration: 300, easing: cubicOut }}
			>
				{#if step === 'welcome'}
					<div
						class="h-20 w-20 bg-brand-primary"
						style="-webkit-mask-image: url('/crate-logo.svg'); -webkit-mask-size: contain; -webkit-mask-repeat: no-repeat; -webkit-mask-position: center; mask-image: url('/crate-logo.svg'); mask-size: contain; mask-repeat: no-repeat; mask-position: center;"
					></div>
					<div class="space-y-2">
						<h1 class="text-2xl font-bold text-text-primary">
							{$translate('onboarding.mobile.welcome.title', { values: { appName: 'Crate' } })}
						</h1>
						<p class="text-base text-text-secondary">{$translate('onboarding.mobile.welcome.subtitle')}</p>
					</div>
				{:else if step === 'discover'}
					<div class="flex h-20 w-20 items-center justify-center rounded-full bg-surface-2 text-brand-primary">
						<svg viewBox="0 0 24 24" class="h-10 w-10" fill="currentColor">
							<path d="M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6zm-2 16a2 2 0 1 1 0-4 2 2 0 0 1 0 4z" />
						</svg>
					</div>
					<div class="space-y-2">
						<h1 class="text-2xl font-bold text-text-primary">{$translate('onboarding.mobile.discover.title')}</h1>
						<p class="text-base text-text-secondary">{$translate('onboarding.mobile.discover.description')}</p>
					</div>
				{:else if step === 'sync'}
					<div class="flex h-20 w-20 items-center justify-center rounded-full bg-surface-2 text-brand-primary">
						<svg viewBox="0 0 24 24" class="h-10 w-10" fill="none" stroke="currentColor" stroke-width="1.8">
							<path
								d="M7 18a4 4 0 0 1 0-8 5 5 0 0 1 9.6-1.5A3.5 3.5 0 0 1 17 18H7z"
								stroke-linecap="round"
								stroke-linejoin="round"
							/>
						</svg>
					</div>
					<div class="space-y-2">
						<h1 class="text-2xl font-bold text-text-primary">{$translate('onboarding.mobile.sync.title')}</h1>
						<p class="text-base text-text-secondary">{$translate('onboarding.mobile.sync.description')}</p>
					</div>
					<!--
						Google-branded sign-in button per Google Identity guidelines: white surface, gray border,
						the official 4-color "G", and the required "Sign in with Google" wording. Brand colors are
						hard-coded (not theme tokens) on purpose — this is a third-party brand element. Mirrors
						SyncPanel.svelte.
					-->
					<button
						type="button"
						class="mt-1 flex h-11 w-full items-center justify-center gap-3 rounded-md border border-[#747775] bg-white px-3 text-sm font-medium text-[#1f1f1f] active:opacity-80 disabled:opacity-60"
						style="font-family: 'Roboto', system-ui, sans-serif;"
						onclick={signIn}
						disabled={$signingIn}
					>
						{#if $signingIn}
							<Spinner class="h-5 w-5 text-[#1f1f1f]" />
						{:else}
							<svg class="h-5 w-5" viewBox="0 0 18 18" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
								<path
									fill="#EA4335"
									d="M9 3.48c1.69 0 2.83.73 3.48 1.34l2.54-2.48C13.46.89 11.43 0 9 0 5.48 0 2.44 2.02.96 4.96l2.91 2.26C4.6 5.05 6.62 3.48 9 3.48Z"
								/>
								<path
									fill="#4285F4"
									d="M17.64 9.2c0-.74-.06-1.28-.19-1.84H9v3.34h4.96c-.1.83-.64 2.08-1.84 2.92l2.84 2.2c1.7-1.57 2.68-3.88 2.68-6.62Z"
								/>
								<path
									fill="#FBBC05"
									d="M3.88 10.78A5.54 5.54 0 0 1 3.58 9c0-.62.11-1.22.29-1.78L.96 4.96A9 9 0 0 0 0 9c0 1.45.35 2.82.96 4.04l2.92-2.26Z"
								/>
								<path
									fill="#34A853"
									d="M9 18c2.43 0 4.47-.8 5.96-2.18l-2.84-2.2c-.76.53-1.78.9-3.12.9-2.38 0-4.4-1.57-5.12-3.74L.96 13.04C2.44 15.98 5.48 18 9 18Z"
								/>
							</svg>
						{/if}
						<span>{$translate('cloudSync.signIn.button', { values: { provider: 'Google' } })}</span>
					</button>
					{#if isIOS()}
						<!-- Sign in with Apple (App Store Guideline 4.8), HIG-styled black button; mirrors SyncPanel.svelte. iOS-only. -->
						<button
							type="button"
							class="flex h-11 w-full items-center justify-center gap-2 rounded-md bg-black px-3 text-sm font-medium text-white active:opacity-80 disabled:opacity-60"
							onclick={signInWithApple}
							disabled={$signingIn}
						>
							{#if $signingIn}
								<Spinner class="h-5 w-5 text-white" />
							{:else}
								<svg class="h-4 w-4" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
									<path
										d="M11.182.008C11.148-.03 9.923.023 8.857 1.18c-1.066 1.156-.902 2.482-.878 2.516.024.034 1.52.087 2.475-1.258.955-1.345.762-2.391.728-2.43zm3.314 11.733c-.048-.096-2.325-1.234-2.113-3.422.212-2.189 1.675-2.789 1.698-2.854.023-.065-.597-.79-1.254-1.157a3.692 3.692 0 0 0-1.563-.434c-.108-.003-.483-.095-1.254.116-.508.139-1.653.589-1.968.607-.316.018-1.256-.522-2.267-.665-.647-.125-1.333.131-1.824.328-.49.196-1.422.754-2.074 2.237-.652 1.482-.311 3.83-.067 4.56.244.729.625 1.924 1.273 2.796.576.984 1.34 1.667 1.659 1.899.319.232 1.219.386 1.843.067.502-.308 1.408-.485 1.766-.472.357.013 1.061.154 1.782.539.571.197 1.111.115 1.652-.105.541-.221 1.324-1.059 2.238-2.758.347-.79.505-1.217.473-1.282z"
									/>
								</svg>
							{/if}
							<span>{$translate('cloudSync.signIn.button', { values: { provider: 'Apple' } })}</span>
						</button>
					{/if}
					{#if $cloudSyncError}
						<p class="text-xs text-danger">{$cloudSyncError}</p>
					{/if}
				{:else}
					<div class="space-y-2">
						<h1 class="text-2xl font-bold text-text-primary">{$translate('onboarding.mobile.appearance.title')}</h1>
						<p class="text-base text-text-secondary">{$translate('onboarding.mobile.appearance.description')}</p>
					</div>

					<!-- Theme -->
					<div class="w-full space-y-2 text-left">
						<p class="px-1 text-xs font-semibold tracking-wide text-text-tertiary uppercase">
							{$translate('settings.appearance.theme')}
						</p>
						<div class="flex gap-2">
							{#each themeOptions as option (option.value)}
								<button
									type="button"
									class="flex flex-1 flex-col items-center gap-2 rounded-xl border-2 p-3 transition-colors {$theme ===
									option.value
										? 'border-brand-primary bg-brand-muted'
										: 'border-stroke active:border-text-tertiary'}"
									onclick={() => settingsStore.setTheme(option.value)}
								>
									{#if option.value === 'light'}
										<svg
											class="h-5 w-5 text-text-primary"
											viewBox="0 0 24 24"
											fill="none"
											stroke="currentColor"
											stroke-width="1.8"
										>
											<circle cx="12" cy="12" r="4" />
											<path
												d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"
												stroke-linecap="round"
											/>
										</svg>
									{:else if option.value === 'dark'}
										<svg
											class="h-5 w-5 text-text-primary"
											viewBox="0 0 24 24"
											fill="none"
											stroke="currentColor"
											stroke-width="1.8"
										>
											<path
												d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z"
												stroke-linecap="round"
												stroke-linejoin="round"
											/>
										</svg>
									{:else}
										<svg
											class="h-5 w-5 text-text-primary"
											viewBox="0 0 24 24"
											fill="none"
											stroke="currentColor"
											stroke-width="1.8"
										>
											<rect x="3" y="4" width="18" height="12" rx="1.5" />
											<path d="M8 20h8M12 16v4" stroke-linecap="round" />
										</svg>
									{/if}
									<span class="text-xs text-text-secondary">{$translate(option.labelKey)}</span>
								</button>
							{/each}
						</div>
					</div>

					<!-- Accent color -->
					<div class="w-full space-y-2 text-left">
						<p class="px-1 text-xs font-semibold tracking-wide text-text-tertiary uppercase">
							{$translate('settings.appearance.accentColor')}
						</p>
						<div class="grid grid-cols-5 gap-3">
							{#each accentColors as color (color.value)}
								<button
									type="button"
									class="flex items-center justify-center rounded-lg p-1 active:opacity-80"
									onclick={() => settingsStore.setAccentColor(color.value)}
									aria-label={$translate(color.labelKey)}
								>
									<span
										class="h-8 w-8 rounded-full transition-transform {$accentColor === color.value
											? 'ring-2 ring-text-primary ring-offset-2 ring-offset-surface-0'
											: ''}"
										style="background-color: {color.hex};"
									></span>
								</button>
							{/each}
						</div>
					</div>
				{/if}
			</div>
		{/key}
	</div>

	<!-- Step indicator + navigation -->
	<div class="pb-safe flex items-center justify-between px-8 pt-4">
		<button
			type="button"
			class="min-w-[64px] py-2 text-left text-sm font-medium text-text-secondary active:opacity-70 disabled:invisible"
			onclick={back}
			disabled={current === 0}
		>
			{$translate('common.back')}
		</button>

		<!-- Dots -->
		<div class="flex items-center gap-2">
			{#each steps as stepName, i (stepName)}
				<span
					class="h-2 rounded-full transition-all duration-200 {i === current
						? 'w-5 bg-brand-primary'
						: 'w-2 bg-stroke'}"
				></span>
			{/each}
		</div>

		<div class="flex min-w-[64px] justify-end">
			{#if step === 'sync'}
				<!-- Sync is optional: "Maybe later" advances to the appearance step without signing in. -->
				<button
					type="button"
					class="py-2 text-right text-sm font-semibold text-brand-primary active:opacity-70"
					onclick={next}
				>
					{$translate('onboarding.mobile.maybeLater')}
				</button>
			{:else}
				<button
					type="button"
					class="py-2 text-right text-sm font-semibold text-brand-primary active:opacity-70"
					onclick={next}
				>
					{isLast ? $translate('onboarding.getStarted') : $translate('common.next')}
				</button>
			{/if}
		</div>
	</div>
</div>
