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
		settingsGet,
		settingsPut,
		type AppInfo,
		type Config
	} from '$lib/api';
	import BackButton from '$lib/components/BackButton.svelte';

	let cfg = $state<Config | null>(null);
	let info = $state<AppInfo | null>(null);
	let loadError = $state<string | null>(null);
	let saveError = $state<string | null>(null);
	let savedAt = $state<number | null>(null);
	let clearing = $state(false);
	let cleared = $state(false);

	const QUALITIES: Array<{ key: string; label: string }> = [
		{ key: 'best', label: 'Best' },
		{ key: '1080', label: '1080' },
		{ key: '720', label: '720' },
		{ key: '480', label: '480' },
		{ key: 'worst', label: 'Worst' }
	];

	const LOCALES: Array<{ key: string; label: string; available: boolean }> = [
		{ key: 'en', label: 'English', available: true },
		{ key: 'pt-BR', label: 'Português (Brasil)', available: false },
		{ key: 'es-419', label: 'Español (Latinoamérica)', available: false },
		{ key: 'ru', label: 'Русский', available: false }
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
		void persist({ ...cfg, locale: l });
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
	<title>Settings · ani-gui</title>
</svelte:head>

<header class="topbar">
	<BackButton label="Back" fallback="/" />
	<div class="saved" class:visible={showSaved} aria-live="polite">
		<span class="saved-mark" aria-hidden="true">✓</span>
		<span>Saved</span>
	</div>
</header>

<main class="page">
	<header class="page-head">
		<p class="eyebrow">
			<span class="eyebrow-key">Settings</span>
			<span class="eyebrow-rule" aria-hidden="true"></span>
			<span class="eyebrow-value">persisted to <code>config.toml</code></span>
		</p>
		<h1 class="page-title">House rules.</h1>
	</header>

	{#if loadError}
		<div class="state state-error" role="alert">
			<p class="state-headline">Couldn't load settings.</p>
			<p class="state-detail">{loadError}</p>
		</div>
	{:else if cfg === null}
		<p class="loading">Loading…</p>
	{:else}
		<!-- PLAYBACK -->
		<section class="section">
			<h2 class="section-eyebrow">
				<span>Playback</span>
				<span class="section-eyebrow-faint">defaults applied to every stream</span>
			</h2>

			<div class="field">
				<div class="field-label">
					<span class="field-key">Audio</span>
					<span class="field-hint"
						>Sub matches Japanese audio with subtitles. Dub uses dubbed audio.</span
					>
				</div>
				<div class="seg" role="group" aria-label="Audio mode">
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
					<span class="field-key">Quality</span>
					<span class="field-hint"
						>"Best" lets the source pick. Lower values cap the resolution.</span
					>
				</div>
				<div class="seg seg-narrow" role="group" aria-label="Quality">
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
					<span class="field-key">External player</span>
					<span class="field-hint">
						Command launched by "Open in external player". Defaults to <code>mpv</code>.
					</span>
				</div>
				<input
					class="text-input"
					type="text"
					value={cfg.external_player}
					oninput={(e) => setExternalPlayer((e.currentTarget as HTMLInputElement).value)}
					placeholder="mpv"
					spellcheck="false"
					autocomplete="off"
					aria-label="External player command"
				/>
			</div>
		</section>

		<hr class="manga-rule" aria-hidden="true" />

		<!-- LIBRARY -->
		<section class="section">
			<h2 class="section-eyebrow">
				<span>Library</span>
				<span class="section-eyebrow-faint">locale, cache, history</span>
			</h2>

			<div class="field">
				<div class="field-label">
					<span class="field-key">Language</span>
					<span class="field-hint">UI strings. Title language is independent (M2.6).</span>
				</div>
				<div class="select-wrap">
					<select
						class="select-input"
						value={cfg.locale}
						onchange={(e) => setLocale((e.currentTarget as HTMLSelectElement).value)}
						aria-label="Locale"
					>
						{#each LOCALES as l (l.key)}
							<option value={l.key} disabled={!l.available}>
								{l.label}{l.available ? '' : '  — coming soon'}
							</option>
						{/each}
					</select>
					<span class="select-caret" aria-hidden="true">▾</span>
				</div>
			</div>

			<div class="field">
				<div class="field-label">
					<span class="field-key">Image cache cap</span>
					<span class="field-hint">
						Posters and banners cached to disk. Older entries evicted when the cap is hit.
					</span>
				</div>
				<div class="number-wrap">
					<input
						class="text-input number-input"
						type="number"
						min="50"
						step="50"
						value={cfg.image_cache_cap_mb}
						oninput={(e) => setCacheCap((e.currentTarget as HTMLInputElement).value)}
						aria-label="Image cache cap, megabytes"
					/>
					<span class="number-suffix" aria-hidden="true">MB</span>
				</div>
			</div>

			<div class="field field-stack">
				<div class="field-label">
					<span class="field-key">Continue Watching history</span>
					<span class="field-hint">
						Stored locally at <code>{info?.history_path ?? '~/.local/state/ani-cli/ani-hsts'}</code
						>. Clearing is permanent.
					</span>
				</div>
				<button type="button" class="btn-danger" onclick={clearHistory} disabled={clearing}>
					<span aria-hidden="true">×</span>
					<span>{clearing ? 'Clearing…' : cleared ? 'History cleared' : 'Clear history'}</span>
				</button>
			</div>
		</section>

		{#if saveError}
			<div class="state state-error" role="alert">
				<p class="state-headline">Couldn't save.</p>
				<p class="state-detail">{saveError}</p>
			</div>
		{/if}

		<hr class="manga-rule" aria-hidden="true" />

		<!-- ABOUT -->
		<section class="section about">
			<h2 class="section-eyebrow">
				<span>About</span>
				<span class="section-eyebrow-faint">credits, version, license</span>
			</h2>

			<dl class="about-list">
				<div class="about-row">
					<dt>Version</dt>
					<dd class="mono">{info?.version ?? '—'}</dd>
				</div>
				<div class="about-row">
					<dt>Built atop</dt>
					<dd>
						<a
							class="inline-link"
							href="https://github.com/pystardust/ani-cli"
							target="_blank"
							rel="noreferrer"
						>
							pystardust/ani-cli
						</a>
						<span class="about-foot"> — the 666-line POSIX-shell scraper this app drives. </span>
					</dd>
				</div>
				<div class="about-row">
					<dt>License</dt>
					<dd>
						GPL-3.0 — inherited from upstream.
						<a class="inline-link" href={resolve('/diagnostics')}>Diagnostics</a>
					</dd>
				</div>
				<div class="about-row">
					<dt>Disclaimer</dt>
					<dd class="about-foot">
						ani-gui scrapes public sources. Use only where local laws permit. The maintainers don't
						operate, host, or guarantee any of the upstream content.
					</dd>
				</div>
			</dl>
		</section>
	{/if}
</main>

<style>
	.topbar {
		position: sticky;
		inset-block-start: 0;
		z-index: 10;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-4);
		padding: var(--space-4) var(--space-6);
		background: color-mix(in oklab, var(--ink-000) 92%, transparent);
		border-block-end: 1px solid var(--ink-200);
		backdrop-filter: blur(8px);
	}

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
		padding: var(--space-7) var(--space-6) var(--space-9);
	}

	.page-head {
		margin-block-end: var(--space-7);
		padding-block-end: var(--space-5);
		border-block-end: 1px solid var(--ink-200);
		box-shadow: 0 5px 0 -4px var(--ink-200);
	}
	.eyebrow {
		margin: 0 0 var(--space-3);
		display: inline-flex;
		align-items: center;
		gap: var(--space-3);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.eyebrow-key {
		color: var(--bone-200);
	}
	.eyebrow-rule {
		inline-size: 2.5rem;
		block-size: 1px;
		background: var(--bone-400);
	}
	.eyebrow-value code {
		font-size: inherit;
		color: var(--bone-200);
	}
	.page-title {
		margin: 0;
		font-family: var(--font-display);
		font-style: italic;
		font-size: clamp(2rem, 4vw, var(--type-display-l));
		letter-spacing: var(--tracking-display);
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
		font-style: italic;
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
		font-family: var(--font-display);
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
		background: var(--accent-ink);
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
		border-block-end: 1px solid var(--accent-ink);
		padding-block-end: 1px;
		transition: color var(--dur-fast) var(--ease-out-soft);
	}
	.inline-link:hover {
		color: var(--accent-ink);
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
		font-family: var(--font-display);
		font-size: var(--type-body-l);
		color: var(--bone-100);
	}
	.state-detail {
		margin: 0;
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		color: var(--bone-300);
	}
</style>
