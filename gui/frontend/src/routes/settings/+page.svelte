<!--
  Settings — editorial register, sectioned. Loads via settingsGet on mount,
  saves via settingsPut on blur (or on segmented-button click), debounced
  300ms for free-text inputs to avoid hammering the IPC channel. The
  oxblood "Clear history" button is the page's only destructive action.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import {
		appInfo,
		historyClear,
		imageCacheClear,
		settingsGet,
		settingsPut,
		type AppInfo,
		type Config
	} from '$lib/api';
	import { m } from '$lib/paraglide/messages';
	import { setLocale as paraglideSetLocale } from '$lib/paraglide/runtime';

	let cfg = $state<Config | null>(null);
	let info = $state<AppInfo | null>(null);
	let loadError = $state<string | null>(null);
	let saveError = $state<string | null>(null);
	let savedAt = $state<number | null>(null);
	let clearing = $state(false);
	let cleared = $state(false);
	let clearingImages = $state(false);
	let imagesCleared = $state(false);

	const QUALITIES: Array<{ key: string; label: string }> = [
		{ key: 'best', label: 'Best' },
		{ key: '1080', label: '1080' },
		{ key: '720', label: '720' },
		{ key: '480', label: '480' },
		{ key: 'worst', label: 'Worst' }
	];

	const LOCALES: Array<{ key: string; label: string; available: boolean }> = [
		{ key: 'en', label: 'English', available: true },
		{ key: 'pt-BR', label: 'Português (Brasil)', available: true },
		{ key: 'es-419', label: 'Español (Latinoamérica)', available: true },
		{ key: 'ru', label: 'Русский', available: true }
	];

	onMount(() => {
		void settingsGet()
			.then((c) => (cfg = c))
			.catch((e) => (loadError = describeError(e)));
		void appInfo()
			.then((i) => (info = i))
			.catch(() => {
				/* about section gracefully degrades if app info fails */
			});
	});

	function describeError(e: unknown): string {
		if (typeof e === 'object' && e !== null) {
			const obj = e as Record<string, unknown>;
			if (typeof obj.detail === 'string') return obj.detail;
			if (typeof obj.kind === 'string') return obj.kind;
		}
		return String(e);
	}

	let debounceHandle: ReturnType<typeof setTimeout> | null = null;
	async function persist(next: Config) {
		cfg = next;
		try {
			await settingsPut(next);
			savedAt = Date.now();
			saveError = null;
		} catch (e) {
			saveError = describeError(e);
		}
	}
	function persistDebounced(next: Config) {
		cfg = next; // optimistic
		if (debounceHandle) clearTimeout(debounceHandle);
		debounceHandle = setTimeout(() => void persist(next), 300);
	}

	function setMode(mode: 'sub' | 'dub') {
		if (!cfg) return;
		void persist({ ...cfg, mode });
	}
	function setQuality(q: string) {
		if (!cfg) return;
		void persist({ ...cfg, quality: q });
	}
	function setLocale(l: string) {
		if (!cfg) return;
		// Persist the picked locale FIRST, then ask Paraglide to flip
		// — paraglideSetLocale defaults to `reload: true`, which is
		// the only reliable way to re-render every `m.foo()` call in
		// the SPA (Paraglide v2's plain function calls don't trigger
		// Svelte reactivity on their own). Persisting first means the
		// reloaded page reads the new locale from both localStorage
		// (Paraglide's strategy) and config.toml (our backend).
		void persist({ ...cfg, locale: l }).then(() => {
			try {
				paraglideSetLocale(l as Parameters<typeof paraglideSetLocale>[0]);
			} catch {
				/* unknown locale — fall through; the picker's `disabled`
				   guard should keep this branch unreachable. */
			}
		});
	}
	function setExternalPlayer(value: string) {
		if (!cfg) return;
		persistDebounced({ ...cfg, external_player: value });
	}
	function setCacheCap(valueRaw: string) {
		if (!cfg) return;
		const n = Number.parseInt(valueRaw, 10);
		if (Number.isNaN(n) || n < 0) return;
		persistDebounced({ ...cfg, image_cache_cap_mb: n });
	}
	function setAutoPlayNext(value: boolean) {
		if (!cfg) return;
		void persist({ ...cfg, auto_play_next: value });
	}
	function setDownloadBottomBar(value: boolean) {
		if (!cfg) return;
		void persist({ ...cfg, download_bottom_bar_enabled: value });
	}
	function setAutoSkipOp(value: boolean) {
		if (!cfg) return;
		void persist({ ...cfg, auto_skip_op: value });
	}
	function setAutoSkipEd(value: boolean) {
		if (!cfg) return;
		void persist({ ...cfg, auto_skip_ed: value });
	}
	function setUseCustomPlayerControls(value: boolean) {
		if (!cfg) return;
		void persist({ ...cfg, use_custom_player_controls: value });
	}
	function setDisableAutoPipOnLeave(value: boolean) {
		if (!cfg) return;
		void persist({ ...cfg, disable_auto_pip_on_leave: value });
	}

	async function clearHistory() {
		clearing = true;
		try {
			await historyClear();
			cleared = true;
			setTimeout(() => (cleared = false), 2400);
		} catch (e) {
			saveError = describeError(e);
		} finally {
			clearing = false;
		}
	}

	async function clearImages() {
		clearingImages = true;
		try {
			await imageCacheClear();
			imagesCleared = true;
			setTimeout(() => (imagesCleared = false), 2400);
		} catch (e) {
			saveError = describeError(e);
		} finally {
			clearingImages = false;
		}
	}

	// "Saved" affordance — fades after 1.6s.
	let showSaved = $state(false);
	$effect(() => {
		if (savedAt === null) return;
		showSaved = true;
		const t = setTimeout(() => (showSaved = false), 1600);
		return () => clearTimeout(t);
	});
</script>

<svelte:head>
	<title>{m.app_page_title_settings()}</title>
</svelte:head>

<main class="page">
	<header class="page-head">
		<p class="eyebrow">
			<span class="eyebrow-key">{m.settings_eyebrow_key()}</span>
			<span class="eyebrow-rule" aria-hidden="true"></span>
			<span class="eyebrow-value"
				>{m.settings_eyebrow_value_prefix()}<code>{m.settings_eyebrow_value_path()}</code></span
			>
			<!-- Saved indicator now lives inline with the eyebrow rather
			     than in a per-route topbar (the global topbar from
			     +layout.svelte already owns the back button). -->
			<span class="saved" class:visible={showSaved} aria-live="polite">
				<span class="saved-mark" aria-hidden="true">✓</span>
				<span>{m.settings_saved_label()}</span>
			</span>
		</p>
		<h1 class="page-title">{m.settings_title()}</h1>
	</header>

	{#if loadError}
		<div class="state state-error" role="alert">
			<p class="state-headline">{m.settings_error_headline()}</p>
			<p class="state-detail">{loadError}</p>
		</div>
	{:else if cfg === null}
		<p class="loading">{m.settings_loading()}</p>
	{:else}
		<!-- PLAYBACK -->
		<section class="section">
			<h2 class="section-eyebrow">
				<span>{m.settings_section_playback_title()}</span>
				<span class="section-eyebrow-faint">{m.settings_section_playback_hint()}</span>
			</h2>

			<div class="field">
				<div class="field-label">
					<span class="field-key">{m.settings_field_audio_key()}</span>
					<span class="field-hint">{m.settings_field_audio_hint()}</span>
				</div>
				<div class="seg" role="group" aria-label={m.settings_audio_group_aria_label()}>
					{#each ['sub', 'dub'] as mode (mode)}
						<button
							type="button"
							class="seg-btn"
							class:active={cfg.mode === mode}
							aria-pressed={cfg.mode === mode}
							onclick={() => setMode(mode as 'sub' | 'dub')}
						>
							{mode.toUpperCase()}
						</button>
					{/each}
				</div>
			</div>

			<div class="field">
				<div class="field-label">
					<span class="field-key">{m.settings_field_quality_key()}</span>
					<span class="field-hint">{m.settings_field_quality_hint()}</span>
				</div>
				<div class="seg seg-narrow" role="group" aria-label={m.settings_quality_group_aria_label()}>
					{#each QUALITIES as q (q.key)}
						<button
							type="button"
							class="seg-btn"
							class:active={cfg.quality === q.key}
							aria-pressed={cfg.quality === q.key}
							onclick={() => setQuality(q.key)}
						>
							{q.label}
						</button>
					{/each}
				</div>
			</div>

			<div class="field">
				<div class="field-label">
					<span class="field-key">{m.settings_field_external_player_key()}</span>
					<span class="field-hint">
						{m.settings_field_external_player_hint_prefix()}<code
							>{m.settings_field_external_player_default()}</code
						>{m.settings_field_external_player_hint_suffix()}
					</span>
				</div>
				<input
					class="text-input"
					type="text"
					value={cfg.external_player}
					oninput={(e) => setExternalPlayer((e.currentTarget as HTMLInputElement).value)}
					placeholder={m.settings_field_external_player_default()}
					spellcheck="false"
					autocomplete="off"
					aria-label={m.settings_field_external_player_key()}
				/>
			</div>

			<div class="field">
				<div class="field-label">
					<span class="field-key">{m.settings_field_auto_play_next_key()}</span>
					<span class="field-hint">{m.settings_field_auto_play_next_hint()}</span>
				</div>
				<label class="switch">
					<input
						type="checkbox"
						checked={cfg.auto_play_next}
						onchange={(e) => setAutoPlayNext((e.currentTarget as HTMLInputElement).checked)}
						aria-label={m.settings_auto_play_next_aria_label()}
					/>
					<span class="switch-track" aria-hidden="true">
						<span class="switch-thumb"></span>
					</span>
					<span class="switch-state"
						>{cfg.auto_play_next
							? m.settings_switch_state_on()
							: m.settings_switch_state_off()}</span
					>
				</label>
			</div>

			<div class="field">
				<div class="field-label">
					<span class="field-key">{m.settings_field_download_bar_key()}</span>
					<span class="field-hint">{m.settings_field_download_bar_hint()}</span>
				</div>
				<label class="switch">
					<input
						type="checkbox"
						checked={cfg.download_bottom_bar_enabled}
						onchange={(e) => setDownloadBottomBar((e.currentTarget as HTMLInputElement).checked)}
						aria-label={m.settings_download_bar_aria_label()}
					/>
					<span class="switch-track" aria-hidden="true">
						<span class="switch-thumb"></span>
					</span>
					<span class="switch-state"
						>{cfg.download_bottom_bar_enabled
							? m.settings_switch_state_on()
							: m.settings_switch_state_off()}</span
					>
				</label>
			</div>

			<div class="field">
				<div class="field-label">
					<span class="field-key">{m.settings_field_auto_skip_op_key()}</span>
					<span class="field-hint">{m.settings_field_auto_skip_op_hint()}</span>
				</div>
				<label class="switch">
					<input
						type="checkbox"
						checked={cfg.auto_skip_op}
						onchange={(e) => setAutoSkipOp((e.currentTarget as HTMLInputElement).checked)}
						aria-label={m.settings_auto_skip_op_aria_label()}
					/>
					<span class="switch-track" aria-hidden="true">
						<span class="switch-thumb"></span>
					</span>
					<span class="switch-state"
						>{cfg.auto_skip_op ? m.settings_switch_state_on() : m.settings_switch_state_off()}</span
					>
				</label>
			</div>

			<div class="field">
				<div class="field-label">
					<span class="field-key">{m.settings_field_auto_skip_ed_key()}</span>
					<span class="field-hint">{m.settings_field_auto_skip_ed_hint()}</span>
				</div>
				<label class="switch">
					<input
						type="checkbox"
						checked={cfg.auto_skip_ed}
						onchange={(e) => setAutoSkipEd((e.currentTarget as HTMLInputElement).checked)}
						aria-label={m.settings_auto_skip_ed_aria_label()}
					/>
					<span class="switch-track" aria-hidden="true">
						<span class="switch-thumb"></span>
					</span>
					<span class="switch-state"
						>{cfg.auto_skip_ed ? m.settings_switch_state_on() : m.settings_switch_state_off()}</span
					>
				</label>
			</div>

			<div class="field">
				<div class="field-label">
					<span class="field-key">{m.settings_field_custom_controls_key()}</span>
					<span class="field-hint">{m.settings_field_custom_controls_hint()}</span>
				</div>
				<label class="switch">
					<input
						type="checkbox"
						checked={cfg.use_custom_player_controls}
						onchange={(e) =>
							setUseCustomPlayerControls((e.currentTarget as HTMLInputElement).checked)}
						aria-label={m.settings_custom_controls_aria_label()}
					/>
					<span class="switch-track" aria-hidden="true">
						<span class="switch-thumb"></span>
					</span>
					<span class="switch-state"
						>{cfg.use_custom_player_controls
							? m.settings_switch_state_on()
							: m.settings_switch_state_off()}</span
					>
				</label>
			</div>

			<div class="field">
				<div class="field-label">
					<span class="field-key">{m.settings_field_disable_pip_key()}</span>
					<span class="field-hint">{m.settings_field_disable_pip_hint()}</span>
				</div>
				<label class="switch">
					<input
						type="checkbox"
						checked={cfg.disable_auto_pip_on_leave}
						onchange={(e) =>
							setDisableAutoPipOnLeave((e.currentTarget as HTMLInputElement).checked)}
						aria-label={m.settings_disable_pip_aria_label()}
					/>
					<span class="switch-track" aria-hidden="true">
						<span class="switch-thumb"></span>
					</span>
					<span class="switch-state"
						>{cfg.disable_auto_pip_on_leave
							? m.settings_switch_state_on()
							: m.settings_switch_state_off()}</span
					>
				</label>
			</div>
		</section>

		<hr class="manga-rule" aria-hidden="true" />

		<!-- LIBRARY -->
		<section class="section">
			<h2 class="section-eyebrow">
				<span>{m.settings_section_library_title()}</span>
				<span class="section-eyebrow-faint">{m.settings_section_library_hint()}</span>
			</h2>

			<div class="field">
				<div class="field-label">
					<span class="field-key">{m.settings_field_language_key()}</span>
					<span class="field-hint">{m.settings_field_language_hint()}</span>
				</div>
				<div class="select-wrap">
					<select
						class="select-input"
						value={cfg.locale}
						onchange={(e) => setLocale((e.currentTarget as HTMLSelectElement).value)}
						aria-label={m.settings_language_aria_label()}
					>
						{#each LOCALES as l (l.key)}
							<option value={l.key} disabled={!l.available}>
								{l.label}{l.available ? '' : m.settings_locale_unavailable_suffix()}
							</option>
						{/each}
					</select>
					<span class="select-caret" aria-hidden="true">▾</span>
				</div>
			</div>

			<div class="field">
				<div class="field-label">
					<span class="field-key">{m.settings_field_image_cache_cap_key()}</span>
					<span class="field-hint">{m.settings_field_image_cache_cap_hint()}</span>
				</div>
				<div class="number-wrap">
					<input
						class="text-input number-input"
						type="number"
						min="50"
						step="50"
						value={cfg.image_cache_cap_mb}
						oninput={(e) => setCacheCap((e.currentTarget as HTMLInputElement).value)}
						aria-label={m.settings_image_cache_cap_aria_label()}
					/>
					<span class="number-suffix" aria-hidden="true">{m.settings_image_cache_suffix()}</span>
				</div>
			</div>

			<div class="field field-stack">
				<div class="field-label">
					<span class="field-key">{m.settings_field_clear_images_key()}</span>
					<span class="field-hint">
						{m.settings_field_clear_images_hint_prefix()}<code
							>{m.settings_field_clear_images_hint_path()}</code
						>{m.settings_field_clear_images_hint_suffix()}
					</span>
				</div>
				<button type="button" class="btn-danger" onclick={clearImages} disabled={clearingImages}>
					<span aria-hidden="true">×</span>
					<span
						>{clearingImages
							? m.settings_clear_images_clearing()
							: imagesCleared
								? m.settings_clear_images_cleared()
								: m.settings_clear_images_button()}</span
					>
				</button>
			</div>

			<div class="field field-stack">
				<div class="field-label">
					<span class="field-key">{m.settings_field_history_key()}</span>
					<span class="field-hint">
						{m.settings_field_history_hint_path_prefix()}<code
							>{info?.history_path ?? '~/.local/state/ani-cli/ani-hsts'}</code
						>{m.settings_field_history_hint_path_suffix()}
					</span>
				</div>
				<button type="button" class="btn-danger" onclick={clearHistory} disabled={clearing}>
					<span aria-hidden="true">×</span>
					<span
						>{clearing
							? m.settings_clear_history_clearing()
							: cleared
								? m.settings_clear_history_cleared()
								: m.settings_clear_history_button()}</span
					>
				</button>
			</div>
		</section>

		{#if saveError}
			<div class="state state-error" role="alert">
				<p class="state-headline">{m.settings_error_save_headline()}</p>
				<p class="state-detail">{saveError}</p>
			</div>
		{/if}

		<hr class="manga-rule" aria-hidden="true" />

		<!-- ABOUT -->
		<section class="section about">
			<h2 class="section-eyebrow">
				<span>{m.settings_section_about_title()}</span>
				<span class="section-eyebrow-faint">{m.settings_section_about_hint()}</span>
			</h2>

			<dl class="about-list">
				<div class="about-row">
					<dt>{m.settings_about_version_label()}</dt>
					<dd class="mono">{info?.version ?? '—'}</dd>
				</div>
				<div class="about-row">
					<dt>{m.settings_about_built_label()}</dt>
					<dd>
						<a
							class="inline-link"
							href="https://github.com/pystardust/ani-cli"
							target="_blank"
							rel="noreferrer"
						>
							{m.settings_about_ani_cli_link()}
						</a>
						<span class="about-foot">{m.settings_about_ani_cli_description()}</span>
					</dd>
				</div>
				<div class="about-row">
					<dt>{m.settings_about_license_label()}</dt>
					<dd>
						{m.settings_about_license_text()}
						<a class="inline-link" href={resolve('/diagnostics')}
							>{m.settings_about_diagnostics_link()}</a
						>
					</dd>
				</div>
				<div class="about-row">
					<dt>{m.settings_about_disclaimer_label()}</dt>
					<dd class="about-foot">
						{m.settings_about_disclaimer_text()}
					</dd>
				</div>
			</dl>
		</section>
	{/if}
</main>

<style>
	.saved {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-200);
		opacity: 0;
		transition: opacity var(--dur-med) var(--ease-out-soft);
	}
	.saved.visible {
		opacity: 1;
	}
	.saved-mark {
		display: inline-grid;
		place-items: center;
		inline-size: 1.1rem;
		block-size: 1.1rem;
		border: 1px solid var(--accent-jade);
		color: var(--accent-jade);
		border-radius: 999px;
		font-size: 0.7rem;
	}

	.page {
		max-inline-size: 56rem;
		margin-inline: auto;
		padding: var(--space-7) var(--space-6) var(--space-8);
		/* Settings has no per-show context, but the global default
		   accent (--accent-ink) is a muted blue that read as a stray
		   default highlight on the segmented buttons. Pin the page's
		   accent to the brand persimmon so highlights match the
		   editorial register the rest of the app uses. */
		--accent: var(--accent-persimmon);
	}

	.page-head {
		margin-block-end: var(--space-7);
		padding-block-end: var(--space-5);
		border-block-end: 1px solid var(--ink-200);
		box-shadow: 0 5px 0 -4px var(--ink-200);
	}
	.eyebrow {
		margin: 0 0 var(--space-3);
	}
	.eyebrow-value code {
		font-size: inherit;
		color: var(--bone-200);
	}
	.page-title {
		margin: 0;
		font-family: var(--font-body);
		font-weight: 600;
		font-size: clamp(2rem, 4vw, var(--type-display-l));
		letter-spacing: -0.02em;
		color: var(--bone-100);
	}

	.section {
		display: grid;
		gap: var(--space-5);
		padding-block: var(--space-5) var(--space-6);
	}
	.section-eyebrow {
		margin: 0 0 var(--space-2);
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-3);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
		font-weight: 500;
	}
	.section-eyebrow-faint {
		color: var(--bone-400);
		text-transform: none;
		letter-spacing: var(--tracking-meta);
	}

	.field {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: var(--space-5);
		align-items: center;
		padding-block: var(--space-3);
		border-block-start: 1px solid var(--ink-200);
	}
	.field-stack {
		grid-template-columns: 1fr;
		justify-items: start;
	}
	@media (max-inline-size: 640px) {
		.field {
			grid-template-columns: 1fr;
			justify-items: start;
		}
	}
	.field-label {
		display: grid;
		gap: 2px;
	}
	.field-key {
		font-family: var(--font-body);
		font-weight: 500;
		font-size: var(--type-body);
		color: var(--bone-100);
	}
	.field-hint {
		font-family: var(--font-body);
		font-size: var(--type-meta);
		color: var(--bone-300);
		max-inline-size: 52ch;
	}
	.field-hint code {
		font-family: var(--font-mono);
		font-size: 0.95em;
		color: var(--bone-200);
	}

	/* Segmented control. */
	.seg {
		display: inline-flex;
		border: 1px solid var(--ink-300);
		border-radius: 2px;
		overflow: hidden;
	}
	.seg-btn {
		padding: var(--space-2) var(--space-4);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
		background: transparent;
		border: 0;
		border-inline-end: 1px solid var(--ink-300);
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft);
	}
	.seg-btn:last-child {
		border-inline-end: 0;
	}
	.seg-btn:hover:not(.active) {
		color: var(--bone-100);
	}
	.seg-btn.active {
		background: var(--accent);
		color: var(--ink-000);
	}
	.seg-narrow .seg-btn {
		padding-inline: var(--space-3);
	}

	/* Text input. */
	.text-input {
		min-inline-size: 14rem;
		padding: var(--space-2) var(--space-3);
		background: var(--ink-050);
		border: 1px solid var(--ink-300);
		color: var(--bone-100);
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		border-radius: 2px;
		transition: border-color var(--dur-fast) var(--ease-out-soft);
	}
	.text-input:focus {
		outline: none;
		border-color: var(--bone-300);
	}
	.text-input::placeholder {
		color: var(--bone-400);
	}

	/* Number + MB suffix. */
	.number-wrap {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
	}
	.number-input {
		inline-size: 7rem;
		min-inline-size: 0;
		text-align: end;
	}
	.number-suffix {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}

	/* Select. */
	.select-wrap {
		position: relative;
		display: inline-block;
	}
	.select-input {
		appearance: none;
		min-inline-size: 16rem;
		padding: var(--space-2) calc(var(--space-3) + 1.25rem) var(--space-2) var(--space-3);
		background: var(--ink-050);
		color: var(--bone-100);
		font-family: var(--font-body);
		font-size: var(--type-body);
		border: 1px solid var(--ink-300);
		border-radius: 2px;
	}
	.select-input:focus {
		outline: none;
		border-color: var(--bone-300);
	}
	.select-caret {
		position: absolute;
		inset-block: 0;
		inset-inline-end: var(--space-2);
		display: grid;
		place-items: center;
		font-family: var(--font-mono);
		color: var(--bone-300);
		pointer-events: none;
	}

	/* Danger button. */
	.btn-danger {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-4);
		color: var(--accent-oxblood);
		border: 1px solid var(--accent-oxblood);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft);
	}
	.btn-danger:hover:not(:disabled) {
		background: var(--accent-oxblood);
		color: var(--ink-000);
	}
	.btn-danger:disabled {
		opacity: 0.6;
		cursor: progress;
	}

	.manga-rule {
		margin-block: var(--space-6);
		border: 0;
		block-size: 1px;
		background: var(--ink-200);
		box-shadow: 0 5px 0 -4px var(--ink-200);
	}

	.about {
		gap: var(--space-3);
	}
	.about-list {
		margin: 0;
		display: grid;
		gap: var(--space-3);
	}
	.about-row {
		display: grid;
		grid-template-columns: 8rem 1fr;
		gap: var(--space-4);
		padding-block: var(--space-2);
		border-block-start: 1px solid var(--ink-200);
	}
	@media (max-inline-size: 640px) {
		.about-row {
			grid-template-columns: 1fr;
		}
	}
	.about-row dt {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.about-row dd {
		margin: 0;
		font-family: var(--font-body);
		font-size: var(--type-body);
		color: var(--bone-100);
	}
	.about-row dd.mono {
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		color: var(--bone-100);
	}
	.about-foot {
		color: var(--bone-300);
		font-size: var(--type-meta);
	}

	.inline-link {
		color: var(--bone-100);
		border-block-end: 1px solid var(--accent);
		padding-block-end: 1px;
		transition: color var(--dur-fast) var(--ease-out-soft);
	}
	.inline-link:hover {
		color: var(--accent);
	}

	.loading {
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		color: var(--bone-300);
	}

	.state {
		margin-block-start: var(--space-5);
		padding: var(--space-5);
		border-inline-start: 2px solid var(--accent-oxblood);
	}
	.state-headline {
		margin: 0 0 var(--space-2);
		font-family: var(--font-body);
		font-weight: 600;
		font-size: var(--type-body-l);
		color: var(--bone-100);
	}
	.state-detail {
		margin: 0;
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		color: var(--bone-300);
	}

	.switch {
		display: inline-flex;
		align-items: center;
		gap: var(--space-3);
		cursor: pointer;
		user-select: none;
	}
	.switch input {
		position: absolute;
		opacity: 0;
		pointer-events: none;
	}
	.switch-track {
		position: relative;
		display: inline-block;
		inline-size: 2.5rem;
		block-size: 1.375rem;
		background: var(--ink-200);
		border-radius: 999px;
		transition:
			background var(--dur-fast) var(--ease-out-soft),
			box-shadow var(--dur-fast) var(--ease-out-soft);
	}
	.switch-thumb {
		position: absolute;
		inset-block-start: 2px;
		inset-inline-start: 2px;
		inline-size: calc(1.375rem - 4px);
		block-size: calc(1.375rem - 4px);
		background: var(--bone-100);
		border-radius: 999px;
		transition: inset-inline-start var(--dur-fast) var(--ease-out-soft);
	}
	.switch input:checked + .switch-track {
		background: var(--accent);
	}
	.switch input:checked + .switch-track .switch-thumb {
		inset-inline-start: calc(2.5rem - (1.375rem - 4px) - 2px);
	}
	.switch input:focus-visible + .switch-track {
		box-shadow: 0 0 0 2px color-mix(in oklab, var(--accent) 60%, transparent);
	}
	.switch-state {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
</style>
