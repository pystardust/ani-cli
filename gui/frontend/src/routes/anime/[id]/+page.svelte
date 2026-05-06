<!--
  Anime detail — editorial magazine spread. M3 design pass.
  Hero band (cover_image with blurred-poster fallback), poster hangs in,
  metadata pill row tinted by per-anime accent, drop-cap synopsis,
  manga-page divider, honest "Episodes coming in M2" placeholder.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { imageProxyUrl, kitsuAnimeDetail, type KitsuAnimeRef } from '$lib/api';
	import { accentFor } from '$lib/design/accent';

	let detail = $state<KitsuAnimeRef | null>(null);
	let error = $state<{ headline: string; detail: string | null } | null>(null);
	let scrollY = $state(0);

	const id = $derived(page.params.id ?? '');
	const accent = $derived(id ? accentFor(id) : 'var(--accent-ink)');

	onMount(async () => {
		if (!id) {
			error = { headline: 'No anime selected.', detail: 'URL is missing the id segment.' };
			return;
		}
		try {
			detail = await kitsuAnimeDetail(id);
		} catch (e) {
			error = describeError(e);
		}
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
		const offset = Math.min(y * 0.25, 80);
		const scale = isCover ? 1.02 : 1.15;
		return `translate3d(0, ${offset}px, 0) scale(${scale})`;
	}
</script>

<svelte:head>
	<title>{detail?.canonical_title ?? 'Loading'} · ani-gui</title>
</svelte:head>

<header class="topbar">
	<a class="back" href={resolve('/search')}>
		<span class="back-rule" aria-hidden="true"></span>
		<span class="back-label">Search</span>
	</a>
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

		{#if detail.synopsis}
			<section class="synopsis">
				<h2 class="section-eyebrow">Synopsis</h2>
				<p class="prose">{detail.synopsis}</p>
			</section>
		{/if}

		<hr class="manga-rule" aria-hidden="true" />

		<section class="episodes">
			<h2 class="section-eyebrow">Episodes</h2>
			<p class="placeholder">
				Episodes will surface once the allanime bridge lands (M2). Until then, paste an upstream HLS
				URL into <a class="inline-link" href={resolve('/play')}>Test Stream</a>
				to validate playback end-to-end.
			</p>
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
	.back {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-200);
	}
	.back-rule {
		inline-size: 1.25rem;
		block-size: 1px;
		background: var(--bone-300);
		transition: inline-size var(--dur-fast) var(--ease-out-soft);
	}
	.back:hover .back-rule {
		inline-size: 2rem;
	}

	.page {
		max-inline-size: var(--content-max);
		margin-inline: auto;
		padding-block-end: var(--space-9);
	}

	/* — Hero band. */
	.hero {
		position: relative;
		aspect-ratio: var(--hero-aspect);
		overflow: hidden;
		background: var(--ink-050);
		margin-block-end: var(--space-7);
	}
	.hero-img {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
		object-position: center 30%;
		will-change: transform;
		/* parallax handled via style:transform; this is the entrance. */
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
		/* SVG noise grain — gives the blurred-poster fallback a film-stock
		   texture so it doesn't read as "broken upscale". */
		position: absolute;
		inset: 0;
		opacity: 0.18;
		pointer-events: none;
		background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='160' height='160'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  0 0 0 0.6 0'/></filter><rect width='100%' height='100%' filter='url(%23n)'/></svg>");
		background-size: 160px 160px;
		mix-blend-mode: overlay;
	}

	/* — Masthead: asymmetric. Poster hangs into the hero band. */
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
		/* Italic on the magazine-spread pull is an editorial tell. */
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

	.meta-row {
		margin: 0;
		padding: var(--space-3) 0 0;
		list-style: none;
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-5) var(--space-6);
		border-block-start: 1px solid var(--accent);
		/* The "thin top border in accent" the brief calls for. */
	}
	.meta-pill {
		display: inline-flex;
		flex-direction: column;
		gap: 2px;
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

	/* — Synopsis: editorial column with drop cap. */
	.synopsis {
		margin: var(--space-7) auto 0;
		padding-inline: var(--space-6);
		max-inline-size: 65ch;
	}
	.section-eyebrow {
		margin: 0 0 var(--space-3);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
		font-weight: 500;
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

	/* — Manga-page divider: 1px solid + 1px hairline 4px below, both muted. */
	.manga-rule {
		margin: var(--space-8) var(--space-6) var(--space-6);
		border: 0;
		block-size: 1px;
		background: var(--ink-200);
		box-shadow: 0 5px 0 -4px var(--ink-200);
	}

	.episodes {
		padding-inline: var(--space-6);
		max-inline-size: 65ch;
	}
	.placeholder {
		margin: 0;
		color: var(--bone-300);
		font-size: var(--type-body);
		line-height: var(--leading-prose);
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
