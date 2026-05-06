<!--
  Anime detail v2 — editorial spread, but no longer synopsis-dominant.
  Composition:
    - Topbar with the global BackButton (consistent across the app).
    - Hero band (cover_image with blurred-poster fallback) + parallax.
    - Masthead: poster hangs into the hero. Right column carries title +
      action row (Play / Download / External) + Sub-Dub + Quality + meta-row.
    - 2-column body: left = synopsis (≤60ch, drop cap kept), right =
      Episodes panel (12 placeholder tiles with per-anime accent on hover).
      Vertical accent-tinted hairline divides the columns.
  CLI features represented even when wiring is M2:
    Mode (sub/dub) and Quality persist via settingsGet/settingsPut today.
    Play / Download / External are wired to TODOs with inline notice.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import {
		imageProxyUrl,
		kitsuAnimeDetail,
		kitsuEpisodes,
		settingsGet,
		settingsPut,
		type Config,
		type KitsuAnimeRef,
		type KitsuEpisode
	} from '$lib/api';
	import { accentFor } from '$lib/design/accent';
	import BackButton from '$lib/components/BackButton.svelte';

	let detail = $state<KitsuAnimeRef | null>(null);
	let error = $state<{ headline: string; detail: string | null } | null>(null);
	let scrollY = $state(0);

	// Episode list — fetched in parallel with the detail. null while in-
	// flight, [] once we know the response was empty (e.g. an upcoming
	// title that hasn't aired yet).
	let episodes = $state<KitsuEpisode[] | null>(null);
	let episodesError = $state<string | null>(null);

	let config = $state<Config | null>(null);
	let configError = $state<string | null>(null);

	// Inline status banner when an action isn't wired yet (Play/Download/External
	// hit allanime, which is M2). Kept tight; not a modal.
	let actionNotice = $state<string | null>(null);

	const id = $derived(page.params.id ?? '');
	const accent = $derived(id ? accentFor(id) : 'var(--accent-ink)');

	const QUALITIES: Array<{ key: string; label: string }> = [
		{ key: 'best', label: 'Best' },
		{ key: '1080', label: '1080' },
		{ key: '720', label: '720' },
		{ key: '480', label: '480' },
		{ key: 'worst', label: 'Worst' }
	];

	onMount(async () => {
		if (!id) {
			error = { headline: 'No anime selected.', detail: 'URL is missing the id segment.' };
			return;
		}
		// Fire detail + settings + episodes in parallel.
		void kitsuAnimeDetail(id)
			.then((d) => (detail = d))
			.catch((e) => (error = describeError(e)));
		void settingsGet()
			.then((c) => (config = c))
			.catch((e) => (configError = describeErrorString(e)));
		void kitsuEpisodes(id)
			.then((e) => (episodes = e))
			.catch((e) => {
				episodes = [];
				episodesError = describeErrorString(e);
			});
	});

	$effect(() => {
		const onScroll = () => {
			scrollY = window.scrollY;
		};
		window.addEventListener('scroll', onScroll, { passive: true });
		onScroll();
		return () => window.removeEventListener('scroll', onScroll);
	});

	function describeError(e: unknown): { headline: string; detail: string | null } {
		if (typeof e === 'object' && e !== null) {
			const obj = e as Record<string, unknown>;
			const detail =
				typeof obj.detail === 'string'
					? obj.detail
					: typeof obj.kind === 'string'
						? obj.kind
						: null;
			return { headline: "Couldn't load this title.", detail };
		}
		return { headline: "Couldn't load this title.", detail: String(e) };
	}
	function describeErrorString(e: unknown): string {
		if (typeof e === 'object' && e !== null) {
			const obj = e as Record<string, unknown>;
			if (typeof obj.detail === 'string') return obj.detail;
			if (typeof obj.kind === 'string') return obj.kind;
		}
		return String(e);
	}

	function heroFor(d: KitsuAnimeRef): { url: string | null; isCover: boolean } {
		const cover = d.cover_image?.large ?? d.cover_image?.original ?? d.cover_image?.small ?? null;
		if (cover) return { url: imageProxyUrl(cover), isCover: true };
		const poster =
			d.poster_image?.large ?? d.poster_image?.original ?? d.poster_image?.medium ?? null;
		return { url: imageProxyUrl(poster), isCover: false };
	}
	function posterFor(d: KitsuAnimeRef): string | null {
		return imageProxyUrl(
			d.poster_image?.large ?? d.poster_image?.medium ?? d.poster_image?.original ?? null
		);
	}
	function ratingDisplay(r: number | null): string | null {
		if (r === null) return null;
		return (r / 10).toFixed(1);
	}
	function yearOf(d: KitsuAnimeRef): string | null {
		return d.start_date ? d.start_date.slice(0, 4) : null;
	}
	function statusLabel(s: string | null): string {
		if (!s) return '—';
		if (s === 'current') return 'Currently airing';
		if (s === 'finished') return 'Finished';
		if (s === 'upcoming') return 'Upcoming';
		return s;
	}
	function subtypeLabel(s: string | null): string {
		return (s ?? 'TV').toUpperCase();
	}
	function heroTransform(y: number, isCover: boolean): string {
		// Honor prefers-reduced-motion: when set, the hero doesn't translate
		// on scroll. Scale (which doesn't move) is kept for visual polish.
		if (
			typeof window !== 'undefined' &&
			window.matchMedia?.('(prefers-reduced-motion: reduce)').matches
		) {
			return `translate3d(0, 0, 0) scale(${isCover ? 1.02 : 1.15})`;
		}
		const offset = Math.min(y * 0.25, 80);
		const scale = isCover ? 1.02 : 1.15;
		return `translate3d(0, ${offset}px, 0) scale(${scale})`;
	}

	async function setMode(mode: 'sub' | 'dub') {
		if (!config || config.mode === mode) return;
		const next: Config = { ...config, mode };
		config = next;
		try {
			await settingsPut(next);
		} catch (e) {
			configError = describeErrorString(e);
		}
	}
	async function setQuality(q: string) {
		if (!config || config.quality === q) return;
		const next: Config = { ...config, quality: q };
		config = next;
		try {
			await settingsPut(next);
		} catch (e) {
			configError = describeErrorString(e);
		}
	}

	function notify(msg: string) {
		actionNotice = msg;
		// auto-dismiss after a beat so it doesn't stack.
		setTimeout(() => {
			if (actionNotice === msg) actionNotice = null;
		}, 4000);
	}

	function onPlay() {
		// TODO: M2 — resolve allanime episode 1 by Kitsu→allanime title match,
		// hand the stream URL into createSession, navigate to /play.
		notify('Playback wires up in M2 once the allanime bridge lands.');
	}
	function onDownload() {
		// TODO: M2 — invoke ani-cli -d for episode 1 via Tauri sidecar.
		notify('Downloads wire up alongside playback in M2.');
	}
	function onExternal() {
		// TODO: M2 — pass the resolved upstream URL to cmd_open_external_player.
		notify('External player launches once a stream resolves (M2).');
	}
	function onPickEpisode(n: number) {
		// TODO: M2 — needs Kitsu→allanime mapping.
		notify(`Episode ${n} will play once the allanime bridge lands (M2).`);
	}
</script>

<svelte:head>
	<title>{detail?.canonical_title ?? 'Loading'} · ani-gui</title>
</svelte:head>

<header class="topbar">
	<BackButton label="Back" fallback="/" />
</header>

<main class="page" style:--accent={accent}>
	{#if error}
		<div class="state state-error" role="alert">
			<p class="state-headline">{error.headline}</p>
			{#if error.detail}<p class="state-detail">{error.detail}</p>{/if}
		</div>
	{:else if detail === null}
		<!-- Skeleton -->
		<section class="hero hero-skeleton" aria-busy="true">
			<div class="hero-img hero-skeleton-img"></div>
		</section>
		<section class="masthead">
			<div class="poster-frame poster-skeleton"></div>
			<div class="masthead-text">
				<div class="line line-skeleton" style="inline-size: 70%; block-size: 2.5rem;"></div>
				<div class="line line-skeleton" style="inline-size: 40%; block-size: 1rem;"></div>
				<div class="line line-skeleton" style="inline-size: 90%; block-size: 0.8rem;"></div>
				<div class="line line-skeleton" style="inline-size: 80%; block-size: 0.8rem;"></div>
				<div class="line line-skeleton" style="inline-size: 60%; block-size: 0.8rem;"></div>
			</div>
		</section>
	{:else}
		{@const hero = heroFor(detail)}
		{@const poster = posterFor(detail)}
		{@const rating = ratingDisplay(detail.average_rating)}
		{@const year = yearOf(detail)}

		<section class="hero" class:hero-fallback={!hero.isCover}>
			{#if hero.url}
				<img
					class="hero-img"
					src={hero.url}
					alt=""
					style:transform={heroTransform(scrollY, hero.isCover)}
				/>
			{/if}
			<div class="hero-scrim" aria-hidden="true"></div>
			{#if !hero.isCover}
				<div class="hero-grain" aria-hidden="true"></div>
			{/if}
		</section>

		<section class="masthead">
			<div class="poster-frame">
				{#if poster}
					<img class="poster-img" src={poster} alt="" />
				{:else}
					<span class="poster-placeholder" aria-hidden="true">
						<span class="poster-placeholder-title">{detail.canonical_title}</span>
					</span>
				{/if}
			</div>

			<div class="masthead-text">
				<p class="eyebrow">
					<span class="eyebrow-key">{subtypeLabel(detail.subtype)}</span>
					<span class="eyebrow-rule" aria-hidden="true"></span>
					<span class="eyebrow-value">{statusLabel(detail.status)}</span>
				</p>

				<h1 class="title">{detail.canonical_title}</h1>

				<!-- Action row: primary play, secondary download, ghost external -->
				<div class="actions" aria-label="Title actions">
					<button type="button" class="btn btn-primary" onclick={onPlay}>
						<span aria-hidden="true">▸</span>
						<span>Play episode 1</span>
					</button>
					<button type="button" class="btn btn-outline" onclick={onDownload}>
						<span aria-hidden="true">↓</span>
						<span>Download</span>
					</button>
					<button type="button" class="btn btn-ghost" onclick={onExternal}>
						<span>External player</span>
						<span aria-hidden="true">↗</span>
					</button>
				</div>

				<!-- Sub/Dub + Quality controls. Reads/writes ani-gui config. -->
				<div class="controls">
					<div class="seg-group" role="group" aria-label="Audio mode">
						<span class="seg-label">Audio</span>
						<div class="seg">
							{#each ['sub', 'dub'] as mode (mode)}
								<button
									type="button"
									class="seg-btn"
									class:active={config?.mode === mode}
									aria-pressed={config?.mode === mode}
									disabled={!config}
									onclick={() => setMode(mode as 'sub' | 'dub')}
								>
									{mode.toUpperCase()}
								</button>
							{/each}
						</div>
					</div>

					<div class="seg-group" role="group" aria-label="Quality">
						<span class="seg-label">Quality</span>
						<div class="seg seg-narrow">
							{#each QUALITIES as q (q.key)}
								<button
									type="button"
									class="seg-btn"
									class:active={config?.quality === q.key}
									aria-pressed={config?.quality === q.key}
									disabled={!config}
									onclick={() => setQuality(q.key)}
								>
									{q.label}
								</button>
							{/each}
						</div>
					</div>

					{#if configError}
						<span class="seg-error" role="alert">Settings: {configError}</span>
					{/if}
				</div>

				<ul class="meta-row" aria-label="Title metadata">
					{#if year}
						<li class="meta-pill">
							<span class="meta-key">Year</span>
							<span class="meta-val num">{year}</span>
						</li>
					{/if}
					{#if detail.episode_count}
						<li class="meta-pill meta-pill-feature">
							<span class="meta-key">Episodes</span>
							<span class="meta-val num num-xl">{detail.episode_count}</span>
						</li>
					{/if}
					{#if rating}
						<li class="meta-pill">
							<span class="meta-key">Rating</span>
							<span class="meta-val num">
								<span class="star" aria-hidden="true">★</span>{rating}<span class="meta-faint"
									>/10</span
								>
							</span>
						</li>
					{/if}
					{#if detail.age_rating}
						<li class="meta-pill">
							<span class="meta-key">Age</span>
							<span class="meta-val">{detail.age_rating}</span>
						</li>
					{/if}
					{#if detail.popularity_rank}
						<li class="meta-pill">
							<span class="meta-key">Rank</span>
							<span class="meta-val num">#{detail.popularity_rank}</span>
						</li>
					{/if}
				</ul>
			</div>
		</section>

		{#if actionNotice}
			<div class="action-notice" role="status">
				<span class="action-notice-key">Note</span>
				<span class="action-notice-rule" aria-hidden="true"></span>
				<span>{actionNotice}</span>
			</div>
		{/if}

		<!-- 2-column body: synopsis + episodes panel -->
		<section class="body">
			<div class="body-col body-col-prose">
				<h2 class="section-eyebrow">Synopsis</h2>
				{#if detail.synopsis}
					<p class="prose">{detail.synopsis}</p>
				{:else}
					<p class="prose-empty">No synopsis on file at Kitsu.</p>
				{/if}
			</div>

			<div class="body-divider" aria-hidden="true"></div>

			<div class="body-col body-col-episodes">
				<h2 class="section-eyebrow">
					<span>Episodes</span>
					<span class="section-eyebrow-faint">
						{#if episodes && episodes.length > 0 && detail.episode_count}
							{episodes.length} of {detail.episode_count}
						{:else if episodes && episodes.length > 0}
							{episodes.length} loaded
						{:else if episodesError}
							unavailable
						{:else}
							loading
						{/if}
					</span>
				</h2>
				<ul class="ep-grid">
					{#if episodes === null}
						{#each Array.from({ length: 6 }, (_, k) => k) as i (i)}
							<li>
								<div class="ep-tile ep-tile-skel" aria-hidden="true">
									<div class="ep-thumb ep-thumb-skel"></div>
									<div class="ep-foot-skel"></div>
								</div>
							</li>
						{/each}
					{:else if episodes.length === 0}
						<li class="ep-empty">
							{episodesError
								? "Couldn't load episodes from Kitsu."
								: 'No episodes on file at Kitsu yet.'}
						</li>
					{:else}
						{#each episodes as ep (ep.id)}
							{@const thumb = imageProxyUrl(ep.thumbnail?.original ?? null)}
							{@const num = ep.number ?? ep.relative_number ?? null}
							<li>
								<button type="button" class="ep-tile" onclick={() => onPickEpisode(num ?? 0)}>
									<span class="ep-thumb">
										{#if thumb}
											<img src={thumb} alt="" loading="lazy" decoding="async" />
										{:else}
											<span class="ep-thumb-placeholder" aria-hidden="true">
												{num ? num.toString().padStart(2, '0') : '·'}
											</span>
										{/if}
										<span class="ep-tag" aria-hidden="true">
											<span class="ep-tag-key">Ep</span>
											<span class="ep-tag-num">{num ?? '?'}</span>
										</span>
									</span>
									<span class="ep-foot">
										<span class="ep-title">
											{ep.canonical_title ?? `Episode ${num ?? ''}`}
										</span>
										<span class="ep-meta">
											{#if ep.length}<span>{ep.length}m</span>{/if}
										</span>
									</span>
								</button>
							</li>
						{/each}
					{/if}
				</ul>
				<p class="ep-grid-foot">
					Tap to play once the allanime bridge lands. Thumbnails + titles via Kitsu.
				</p>
			</div>
		</section>
	{/if}
</main>

<style>
	.topbar {
		position: sticky;
		inset-block-start: 0;
		z-index: 10;
		padding: var(--space-4) var(--space-6);
		background: color-mix(in oklab, var(--ink-000) 92%, transparent);
		backdrop-filter: blur(8px); /* purposeful: top bar over hero. */
		border-block-end: 1px solid var(--ink-200);
	}

	.page {
		max-inline-size: var(--content-max-wide);
		margin-inline: auto;
		padding-block-end: var(--space-9);
		/* Page-enter animation — a soft fade + lift so navigating into a
		   detail page doesn't feel like a hard cut. The reduced-motion
		   token already zeroes --dur-slow so this is inert when the user
		   opts out. */
		animation: detail-page-enter var(--dur-slow) var(--ease-out-soft) both;
	}
	@keyframes detail-page-enter {
		from {
			opacity: 0;
			transform: translateY(8px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	/* — Hero band. */
	.hero {
		position: relative;
		aspect-ratio: var(--hero-aspect);
		overflow: hidden;
		background: var(--ink-050);
		margin-block-end: var(--space-7);
		/* Hero scales up subtly on entry — feels like the cover comes
		   forward to the screen, per user's M3.7-era request. */
		animation: detail-hero-enter var(--dur-slow) var(--ease-out-soft) both;
	}
	@keyframes detail-hero-enter {
		from {
			transform: scale(1.04);
			filter: brightness(0.7);
		}
		to {
			transform: scale(1);
			filter: brightness(1);
		}
	}
	.hero-img {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
		object-position: center 30%;
		will-change: transform;
		animation: hero-in var(--dur-slow) var(--ease-out-soft) both;
	}
	@keyframes hero-in {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}
	.hero-fallback .hero-img {
		filter: blur(28px) brightness(0.55) saturate(0.85);
	}
	.hero-scrim {
		position: absolute;
		inset: 0;
		background:
			linear-gradient(
				180deg,
				color-mix(in oklab, var(--ink-000) 25%, transparent) 0%,
				color-mix(in oklab, var(--ink-000) 0%, transparent) 35%,
				color-mix(in oklab, var(--ink-000) 60%, transparent) 75%,
				var(--ink-000) 100%
			),
			linear-gradient(
				90deg,
				color-mix(in oklab, var(--ink-000) 60%, transparent) 0%,
				color-mix(in oklab, var(--ink-000) 0%, transparent) 35%
			);
		pointer-events: none;
	}
	.hero-grain {
		position: absolute;
		inset: 0;
		opacity: 0.18;
		pointer-events: none;
		background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='160' height='160'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  0 0 0 0.6 0'/></filter><rect width='100%' height='100%' filter='url(%23n)'/></svg>");
		background-size: 160px 160px;
		mix-blend-mode: overlay;
	}

	/* — Masthead: poster hangs into hero. */
	.masthead {
		display: grid;
		grid-template-columns: minmax(12rem, 16rem) 1fr;
		gap: var(--space-7);
		padding-inline: var(--space-6);
		margin-block-start: calc(-1 * var(--space-9));
		align-items: end;
		position: relative;
	}
	@media (max-inline-size: 720px) {
		.masthead {
			grid-template-columns: 1fr;
			margin-block-start: calc(-1 * var(--space-7));
		}
	}
	.poster-frame {
		position: relative;
		aspect-ratio: var(--poster-aspect);
		background: var(--ink-100);
		border-radius: var(--radius-card);
		overflow: hidden;
		box-shadow:
			0 1px 0 1px color-mix(in oklab, var(--accent) 40%, transparent),
			var(--shadow-card-hover);
		transform: translateY(calc(-1 * var(--space-6)));
		animation: poster-in var(--dur-slow) var(--ease-out-elastic) both;
	}
	@keyframes poster-in {
		from {
			opacity: 0;
			transform: translateY(0) scale(0.96);
		}
		to {
			opacity: 1;
			transform: translateY(calc(-1 * var(--space-6))) scale(1);
		}
	}
	.poster-img {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
	}
	.poster-placeholder {
		position: absolute;
		inset: 0;
		display: grid;
		place-items: center;
		padding: var(--space-4);
		background: linear-gradient(180deg, var(--ink-100), var(--ink-050));
	}
	.poster-placeholder-title {
		font-family: var(--font-display);
		font-style: italic;
		font-size: var(--type-display-m);
		color: var(--bone-200);
		text-align: center;
	}

	.masthead-text {
		padding-block-end: var(--space-2);
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
	.eyebrow-rule {
		inline-size: 2.5rem;
		block-size: 1px;
		background: var(--accent);
	}
	.eyebrow-value {
		color: var(--bone-200);
	}

	.title {
		margin: 0 0 var(--space-5);
		font-family: var(--font-display);
		font-size: clamp(2rem, 4.4vw, var(--type-display-xl));
		line-height: var(--leading-tight);
		letter-spacing: var(--tracking-display);
		color: var(--bone-100);
		font-style: italic;
		animation: text-in var(--dur-med) var(--ease-out-soft) both;
		animation-delay: 60ms;
	}
	@keyframes text-in {
		from {
			opacity: 0;
			transform: translateY(6px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	/* — Action row. */
	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-3);
		margin-block-end: var(--space-5);
	}
	.btn {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-3) var(--space-5);
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-100);
		border: 1px solid var(--ink-300);
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft),
			transform var(--dur-fast) var(--ease-out-soft);
	}
	.btn:hover {
		transform: translateY(-1px);
	}
	.btn-primary {
		background: var(--accent);
		color: var(--ink-000);
		border-color: var(--accent);
	}
	.btn-primary:hover {
		background: color-mix(in oklab, var(--accent) 88%, var(--bone-100));
	}
	.btn-outline {
		color: var(--bone-100);
		border-color: var(--bone-300);
	}
	.btn-outline:hover {
		border-color: var(--bone-100);
	}
	.btn-ghost {
		color: var(--bone-300);
		border-color: transparent;
		padding-inline: var(--space-2);
	}
	.btn-ghost:hover {
		color: var(--bone-100);
	}

	/* — Segmented controls (sub/dub + quality). */
	.controls {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-5);
		align-items: center;
		margin-block-end: var(--space-5);
	}
	.seg-group {
		display: inline-flex;
		flex-direction: column;
		gap: var(--space-2);
	}
	.seg-label {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
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
	.seg-btn:hover:not(:disabled):not(.active) {
		color: var(--bone-100);
	}
	.seg-btn.active {
		background: var(--accent);
		color: var(--ink-000);
	}
	.seg-btn:disabled {
		opacity: 0.5;
		cursor: progress;
	}
	.seg-narrow .seg-btn {
		padding-inline: var(--space-3);
	}
	.seg-error {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		color: var(--accent-oxblood);
	}

	/* — Meta row. */
	.meta-row {
		margin: 0;
		padding: var(--space-3) 0 0;
		list-style: none;
		display: flex;
		flex-wrap: wrap;
		/* Larger horizontal gap so the per-pill hairline divider has air. */
		gap: var(--space-5) var(--space-7);
		border-block-start: 1px solid var(--accent);
	}
	.meta-pill {
		display: inline-flex;
		flex-direction: column;
		gap: 2px;
		/* Vertical hairline between pills — subtle but enough that year /
		   rating / episodes don't feel cramped together. The last pill in
		   each visual row drops the rule via the :last-child selector
		   (acceptable trade-off when items wrap). */
		padding-inline-end: var(--space-7);
		border-inline-end: 1px solid var(--ink-200);
	}
	.meta-pill:last-child {
		padding-inline-end: 0;
		border-inline-end: 0;
	}
	.meta-key {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.meta-val {
		font-family: var(--font-display);
		font-size: var(--type-body-l);
		color: var(--bone-100);
		letter-spacing: var(--tracking-display);
	}
	.meta-val.num {
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		letter-spacing: 0;
	}
	.meta-val.num-xl {
		font-size: var(--type-numeral-xl);
		line-height: 1;
		color: var(--bone-100);
	}
	.meta-faint {
		color: var(--bone-400);
		font-size: var(--type-meta);
		margin-inline-start: 2px;
	}
	.star {
		color: var(--accent);
		margin-inline-end: 4px;
	}

	/* — Action notice (inline status banner under masthead). */
	.action-notice {
		margin: var(--space-6) var(--space-6) 0;
		padding: var(--space-3) var(--space-4);
		display: inline-flex;
		align-items: center;
		gap: var(--space-3);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-200);
		background: color-mix(in oklab, var(--accent) 6%, var(--ink-050));
		border-inline-start: 2px solid var(--accent);
		animation: text-in var(--dur-med) var(--ease-out-soft) both;
	}
	.action-notice-key {
		color: var(--bone-300);
	}
	.action-notice-rule {
		inline-size: 1.5rem;
		block-size: 1px;
		background: var(--bone-400);
	}

	/* — 2-column body (synopsis + episodes). Capped tighter than the
	     hero so the page doesn't feel "all horizontal" on widescreen.
	     Hero stays full-bleed; only the prose + episode block narrows.
	     The episode column is fixed-width so it doesn't keep spreading. */
	.body {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 1px auto;
		gap: var(--space-8);
		align-items: start;
		max-inline-size: var(--content-max);
		margin: var(--space-7) auto 0;
		padding-inline: var(--space-8);
	}
	@media (max-inline-size: 900px) {
		.body {
			grid-template-columns: 1fr;
		}
		.body-divider {
			display: none;
		}
	}
	.body-divider {
		align-self: stretch;
		inline-size: 1px;
		background: linear-gradient(
			180deg,
			transparent 0%,
			color-mix(in oklab, var(--accent) 50%, var(--ink-200)) 12%,
			color-mix(in oklab, var(--accent) 50%, var(--ink-200)) 88%,
			transparent 100%
		);
	}

	.body-col-prose {
		max-inline-size: 60ch;
	}
	.section-eyebrow {
		margin: 0 0 var(--space-4);
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
		font-variant-numeric: tabular-nums lining-nums;
		letter-spacing: var(--tracking-meta);
	}
	.prose {
		margin: 0;
		font-family: var(--font-display);
		font-size: var(--type-body-l);
		line-height: 1.65;
		color: var(--bone-200);
	}
	.prose::first-letter {
		font-family: var(--font-display);
		float: inline-start;
		font-size: 3.4em;
		line-height: 0.9;
		padding-inline-end: 0.08em;
		padding-block-start: 0.06em;
		color: var(--bone-100);
		font-style: italic;
	}
	.prose-empty {
		margin: 0;
		font-family: var(--font-display);
		font-style: italic;
		color: var(--bone-300);
	}

	/* — Episodes panel. Fixed 3-column grid so 12 tiles render as 4
	     vertical rows — keeps the column tall and proportional with the
	     synopsis next to it instead of spreading flat across the page. */
	.body-col-episodes {
		inline-size: clamp(18rem, 24rem, 28rem);
	}
	@media (max-inline-size: 900px) {
		.body-col-episodes {
			inline-size: 100%;
		}
	}
	.ep-grid {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: var(--space-3);
	}
	/* Tile is now thumbnail-led: 16:9 image at top, mono-numeral overlay
	   tag in the corner, title + duration in the foot. Forward-compatible
	   with future episodes with no thumbnail (placeholder gradient
	   showing the episode number). */
	.ep-tile {
		inline-size: 100%;
		display: grid;
		grid-template-rows: auto 1fr;
		gap: 0;
		padding: 0;
		text-align: start;
		background: var(--ink-050);
		border: 1px solid var(--ink-200);
		border-radius: var(--radius-card);
		overflow: hidden;
		transition:
			border-color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft),
			transform var(--dur-fast) var(--ease-out-soft);
	}
	.ep-tile:hover {
		transform: translateY(-1px);
		border-color: color-mix(in oklab, var(--accent) 70%, var(--ink-300));
	}
	.ep-tile:hover .ep-thumb img {
		filter: brightness(1);
	}
	.ep-thumb {
		position: relative;
		display: block;
		aspect-ratio: 16 / 9;
		overflow: hidden;
		background: var(--ink-100);
	}
	.ep-thumb img {
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
		filter: brightness(0.85);
		transition: filter var(--dur-med) var(--ease-out-soft);
	}
	.ep-thumb-placeholder {
		display: grid;
		place-items: center;
		inline-size: 100%;
		block-size: 100%;
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: var(--type-display-m);
		color: var(--bone-300);
		background: linear-gradient(
			135deg,
			var(--ink-100),
			color-mix(in oklab, var(--accent) 18%, var(--ink-100))
		);
	}
	.ep-tag {
		position: absolute;
		inset-block-start: var(--space-2);
		inset-inline-start: var(--space-2);
		display: inline-flex;
		align-items: baseline;
		gap: var(--space-1);
		padding: 2px var(--space-2);
		background: color-mix(in oklab, var(--ink-000) 78%, transparent);
		backdrop-filter: blur(4px);
		border: 1px solid color-mix(in oklab, var(--accent) 40%, var(--bone-400));
	}
	.ep-tag-key {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.ep-tag-num {
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: var(--type-meta);
		color: var(--bone-100);
	}
	.ep-foot {
		display: grid;
		gap: var(--space-1);
		padding: var(--space-3);
		min-block-size: 4rem;
	}
	.ep-title {
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
		font-family: var(--font-display);
		font-size: var(--type-meta);
		line-height: var(--leading-tight);
		color: var(--bone-100);
	}
	.ep-meta {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-meta);
		color: var(--bone-400);
	}

	/* Skeleton + empty states for the episodes panel. */
	.ep-tile-skel {
		cursor: default;
	}
	.ep-thumb-skel {
		aspect-ratio: 16 / 9;
		background: var(--ink-100);
		animation: pulse 1.6s var(--ease-in-out) infinite;
	}
	.ep-foot-skel {
		block-size: 4rem;
		background: var(--ink-050);
	}
	.ep-empty {
		grid-column: 1 / -1;
		padding: var(--space-5);
		font-family: var(--font-display);
		font-style: italic;
		color: var(--bone-300);
		text-align: center;
		border: 1px dashed var(--ink-300);
		border-radius: var(--radius-card);
	}
	@media (prefers-reduced-motion: reduce) {
		.ep-thumb-skel {
			animation: none;
		}
	}

	/* Old tile foot caption — kept for the placeholder note below the grid. */
	.ep-grid-foot {
		margin-block-start: var(--space-4);
		margin-block-end: 0;
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-meta);
		text-transform: uppercase;
		color: var(--bone-400);
	}

	/* — Skeletons. */
	.hero-skeleton-img {
		inline-size: 100%;
		block-size: 100%;
		background: var(--ink-100);
		animation: pulse 1.6s var(--ease-in-out) infinite;
	}
	.poster-skeleton {
		aspect-ratio: var(--poster-aspect);
		background: var(--ink-100);
		border-radius: var(--radius-card);
		transform: translateY(calc(-1 * var(--space-6)));
		animation: pulse 1.6s var(--ease-in-out) infinite;
	}
	.line {
		block-size: 0.75rem;
		background: var(--ink-100);
		border-radius: 2px;
		margin-block-start: var(--space-3);
		animation: pulse 1.6s var(--ease-in-out) infinite;
	}
	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.55;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.hero-skeleton-img,
		.poster-skeleton,
		.line {
			animation: none;
		}
	}

	/* — States. */
	.state {
		margin: var(--space-7) var(--space-6) 0;
		padding: var(--space-6);
		border-inline-start: 2px solid var(--ink-300);
	}
	.state-error {
		border-inline-start-color: var(--accent-oxblood);
	}
	.state-headline {
		margin: 0 0 var(--space-2);
		font-family: var(--font-display);
		font-size: var(--type-display-m);
		color: var(--bone-100);
		letter-spacing: var(--tracking-display);
	}
	.state-detail {
		margin: 0;
		font-family: var(--font-body);
		font-size: var(--type-body);
		color: var(--bone-300);
		max-inline-size: 60ch;
	}
</style>
