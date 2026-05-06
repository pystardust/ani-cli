<!--
  Home — the front door. Editorial repertory cinema, programmed by a
  librarian. Composition:
    1. Hero: kitsuTrending()[0], full-bleed cover with parallax + scrim,
       "View" call. NO autoplay, NO carousel.
    2. Continue Watching: hidden when historyList() is empty; otherwise a
       horizontal strip whose cards lean on an oversized mono episode number.
    3. Trending strip: rest of kitsuTrending() (1..n).
    4. Top Rated: kitsuTopRated().
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import {
		historyList,
		imageProxyUrl,
		kitsuEpisodes,
		kitsuSearch,
		kitsuTopRated,
		kitsuTrending,
		type HistoryEntry,
		type KitsuAnimeRef,
		type KitsuEpisode
	} from '$lib/api';
	import { accentFor } from '$lib/design/accent';
	import Strip from '$lib/components/Strip.svelte';
	import PosterCard from '$lib/components/PosterCard.svelte';

	let trending = $state<KitsuAnimeRef[] | null>(null);
	let topRated = $state<KitsuAnimeRef[] | null>(null);
	let history = $state<HistoryEntry[] | null>(null);
	// Per-history-entry Kitsu match, keyed by allanime id. Populated lazily
	// after history loads so Continue Watching cards can show posters.
	let historyMatches = $state<Record<string, KitsuAnimeRef | null>>({});
	// Per-history-entry Kitsu episode (by ep_no), keyed by allanime id.
	// Populated after the matching kitsuEpisodes() call resolves so cards
	// can show the actual episode thumbnail + canonical title rather than
	// generic anime-poster + anime-title.
	let historyEpisodes = $state<Record<string, KitsuEpisode | null>>({});
	let trendingError = $state<string | null>(null);
	let topRatedError = $state<string | null>(null);
	let scrollY = $state(0);

	const featured = $derived(trending && trending.length > 0 ? trending[0] : null);
	const trendingTail = $derived(trending ? trending.slice(1) : []);
	const featuredAccent = $derived(featured ? accentFor(featured.id) : 'var(--accent-ink)');

	onMount(() => {
		// Fire all three in parallel; render whichever lands first.
		kitsuTrending()
			.then((t) => (trending = t))
			.catch((e) => (trendingError = describeError(e)));
		kitsuTopRated()
			.then((t) => (topRated = t))
			.catch((e) => (topRatedError = describeError(e)));
		historyList()
			.then((h) => {
				history = h;
				// Two-stage lookup per entry:
				//   1. kitsuSearch by title → matches the anime, gives us
				//      the kitsu id we need for step 2 + a poster fallback.
				//   2. kitsuEpisodes(kitsuId) → find the entry's last-watched
				//      episode by number; gives us the real thumbnail + title.
				// Both stages are fire-and-forget per entry; if either
				// fails the card degrades gracefully (anime poster + anime
				// title is the floor).
				h.forEach((entry: HistoryEntry) => {
					kitsuSearch(titleOf(entry))
						.then((hits: KitsuAnimeRef[]) => {
							const match = hits[0] ?? null;
							historyMatches = { ...historyMatches, [entry.id]: match };
							if (!match) return;
							const wantNum = parseInt(entry.ep_no, 10);
							// Kitsu caps page[limit] at 20 — for entries past episode 20
							// (One Piece, Detective Conan, etc.) we need to compute the
							// right page or the lookup misses every time.
							const targetPage =
								Number.isFinite(wantNum) && wantNum > 0 ? Math.ceil(wantNum / 20) : 1;
							void kitsuEpisodes(match.id, targetPage)
								.then((eps: KitsuEpisode[]) => {
									const ep =
										eps.find((e) => e.number === wantNum) ??
										eps.find((e) => e.relative_number === wantNum) ??
										null;
									historyEpisodes = { ...historyEpisodes, [entry.id]: ep };
								})
								.catch(() => {
									historyEpisodes = { ...historyEpisodes, [entry.id]: null };
								});
						})
						.catch(() => {
							historyMatches = { ...historyMatches, [entry.id]: null };
						});
				});
			})
			.catch(() => {
				history = [];
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

	function describeError(e: unknown): string {
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

	function snippetOf(syn: string | null): string | null {
		if (!syn) return null;
		const trimmed = syn.replace(/\s+/g, ' ').trim();
		// Two-line clamp via CSS, but cap raw length too so rendering is fast.
		return trimmed.length > 280 ? trimmed.slice(0, 277) + '…' : trimmed;
	}

	function heroTransform(y: number, isCover: boolean): string {
		// Honor prefers-reduced-motion: when the user opts out of motion,
		// the hero stays still even on scroll.
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

	function epOf(entry: HistoryEntry): string {
		// Strip non-digits and pad. Some hsts entries have ranges like "1-12";
		// take the head.
		const head = entry.ep_no.split(/[^0-9]+/)[0] ?? entry.ep_no;
		return head || entry.ep_no;
	}

	function titleOf(entry: HistoryEntry): string {
		// Trim "( 26 episodes )" trailing parenthetical from ani-cli's hsts format.
		return entry.title.replace(/\s*\(\s*\d+\s+episodes?\s*\)\s*$/i, '').trim();
	}
</script>

<svelte:head>
	<title>ani-gui</title>
</svelte:head>

<!-- Hero -->
<section class="hero" style:--accent={featuredAccent}>
	{#if featured}
		{@const hero = heroFor(featured)}
		{@const synopsis = snippetOf(featured.synopsis)}
		{#if hero.url}
			<img
				class="hero-img"
				class:fallback={!hero.isCover}
				src={hero.url}
				alt=""
				style:transform={heroTransform(scrollY, hero.isCover)}
			/>
		{/if}
		<div class="hero-scrim" aria-hidden="true"></div>
		{#if !hero.isCover}
			<div class="hero-grain" aria-hidden="true"></div>
		{/if}

		<div class="hero-body">
			<p class="eyebrow">
				<span class="eyebrow-key">Tonight's marquee</span>
				<span class="eyebrow-rule" aria-hidden="true"></span>
				<span class="eyebrow-value">via Kitsu · trending</span>
			</p>
			<h1 class="hero-title">{featured.canonical_title}</h1>
			{#if synopsis}
				<p class="hero-snippet">{synopsis}</p>
			{/if}
			<div class="hero-actions">
				<a class="btn btn-primary" href={resolve('/anime/[id]', { id: featured.id })}>
					<span>View</span>
					<span aria-hidden="true">→</span>
				</a>
				<a class="btn btn-ghost" href={resolve('/search')}>
					<span aria-hidden="true">/</span>
					<span>Browse the catalogue</span>
				</a>
			</div>
		</div>
	{:else if trendingError}
		<div class="hero-empty">
			<p class="eyebrow">
				<span class="eyebrow-key">Off-air</span>
				<span class="eyebrow-rule" aria-hidden="true"></span>
				<span class="eyebrow-value">{trendingError}</span>
			</p>
			<h1 class="hero-title">The catalogue is unreachable.</h1>
			<p class="hero-snippet">Kitsu didn't answer. Search still works locally — type a title.</p>
			<div class="hero-actions">
				<a class="btn btn-primary" href={resolve('/search')}>
					<span>Open Search</span>
					<span aria-hidden="true">→</span>
				</a>
			</div>
		</div>
	{:else}
		<!-- Hero skeleton -->
		<div class="hero-skel" aria-busy="true">
			<div class="hero-skel-img"></div>
		</div>
		<div class="hero-body">
			<p class="eyebrow"><span class="eyebrow-key">Loading</span></p>
			<div class="line line-skel" style="inline-size: 50%; block-size: 3rem;"></div>
			<div class="line line-skel" style="inline-size: 70%;"></div>
			<div class="line line-skel" style="inline-size: 60%;"></div>
		</div>
	{/if}
</section>

<!-- Continue Watching: only when history is non-empty -->
{#if history && history.length > 0}
	<Strip eyebrow="Continue watching" caption="resume from history">
		{#each history as entry (entry.id)}
			{@const match = historyMatches[entry.id]}
			{@const ep = historyEpisodes[entry.id]}
			{@const accent = accentFor(match?.id ?? entry.id)}
			{@const epThumb = imageProxyUrl(ep?.thumbnail?.original ?? null)}
			{@const animePoster = imageProxyUrl(
				match?.poster_image?.medium ??
					match?.poster_image?.large ??
					match?.poster_image?.small ??
					null
			)}
			{@const image = epThumb ?? animePoster}
			{@const targetId = match?.id ?? null}
			<a
				class="resume-card"
				class:resume-card-loading={match === undefined}
				style="--accent: {accent};"
				href={targetId ? resolve('/anime/[id]', { id: targetId }) : resolve('/search')}
			>
				<span class="resume-poster">
					{#if image}
						<img src={image} alt="" loading="lazy" decoding="async" />
					{:else}
						<span class="resume-poster-placeholder" aria-hidden="true">
							{titleOf(entry).slice(0, 2).toUpperCase()}
						</span>
					{/if}
					<span class="resume-ep-tag" aria-hidden="true">
						<span class="resume-ep-key">EP</span>
						<span class="resume-ep-num">{epOf(entry)}</span>
					</span>
				</span>
				<span class="resume-body">
					<span class="resume-show">{titleOf(entry)}</span>
					{#if ep?.canonical_title}
						<span class="resume-title">{ep.canonical_title}</span>
					{:else}
						<span class="resume-title resume-title-faint">Episode {epOf(entry)}</span>
					{/if}
					<span class="resume-cta">
						<span aria-hidden="true">↺</span>
						<span>Resume</span>
					</span>
				</span>
			</a>
		{/each}
	</Strip>
{/if}

<!-- Trending strip (the tail; the head is the hero) -->
{#if trending === null && !trendingError}
	<Strip eyebrow="Trending now" caption="loading">
		{#each Array.from({ length: 8 }, (_, k) => k) as i (i)}
			<div class="skel-card" style="--i: {i};">
				<div class="skel-poster"></div>
				<div class="line line-skel" style="inline-size: 80%; block-size: 0.7rem;"></div>
				<div class="line line-skel" style="inline-size: 40%; block-size: 0.6rem;"></div>
			</div>
		{/each}
	</Strip>
{:else if trendingTail.length > 0}
	<Strip eyebrow="Trending now" caption="currently airing · top 20">
		{#each trendingTail as anime (anime.id)}
			<PosterCard {anime} />
		{/each}
	</Strip>
{/if}

<!-- Top Rated strip -->
{#if topRated === null && !topRatedError}
	<Strip eyebrow="Top rated" caption="loading">
		{#each Array.from({ length: 8 }, (_, k) => k) as i (i)}
			<div class="skel-card" style="--i: {i};">
				<div class="skel-poster"></div>
				<div class="line line-skel" style="inline-size: 80%; block-size: 0.7rem;"></div>
				<div class="line line-skel" style="inline-size: 40%; block-size: 0.6rem;"></div>
			</div>
		{/each}
	</Strip>
{:else if topRated && topRated.length > 0}
	<Strip eyebrow="Top rated" caption="all-time · via Kitsu">
		{#each topRated as anime (anime.id)}
			<PosterCard {anime} />
		{/each}
	</Strip>
{/if}

<style>
	/* — Hero. The marquee. */
	.hero {
		position: relative;
		min-block-size: clamp(28rem, 70dvh, 44rem);
		display: flex;
		align-items: flex-end;
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
	.hero-img.fallback {
		filter: blur(28px) brightness(0.55) saturate(0.85);
	}
	.hero-scrim {
		position: absolute;
		inset: 0;
		background:
			linear-gradient(
				180deg,
				color-mix(in oklab, var(--ink-000) 25%, transparent) 0%,
				color-mix(in oklab, var(--ink-000) 0%, transparent) 30%,
				color-mix(in oklab, var(--ink-000) 60%, transparent) 65%,
				var(--ink-000) 100%
			),
			linear-gradient(
				90deg,
				color-mix(in oklab, var(--ink-000) 78%, transparent) 0%,
				color-mix(in oklab, var(--ink-000) 35%, transparent) 45%,
				color-mix(in oklab, var(--ink-000) 0%, transparent) 75%
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

	.hero-body,
	.hero-empty {
		position: relative;
		z-index: 2;
		max-inline-size: 44rem;
		/* Inline padding matches the strip gutter (--space-8) so the hero
		   text and the trending row sit on the same vertical column. */
		padding: var(--space-9) var(--space-8) var(--space-7);
		animation: hero-text-in var(--dur-med) var(--ease-out-soft) both;
		animation-delay: 100ms;
	}
	@keyframes hero-text-in {
		from {
			opacity: 0;
			transform: translateY(8px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.eyebrow {
		margin: 0 0 var(--space-4);
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
		background: var(--accent);
	}

	.hero-title {
		margin: 0 0 var(--space-4);
		font-family: var(--font-display);
		font-size: clamp(2.25rem, 5vw, var(--type-display-xl));
		line-height: var(--leading-tight);
		letter-spacing: var(--tracking-display);
		font-style: italic;
		color: var(--bone-100);
	}

	.hero-snippet {
		margin: 0 0 var(--space-6);
		font-family: var(--font-display);
		font-size: var(--type-body-l);
		line-height: 1.55;
		color: var(--bone-200);
		max-inline-size: 38rem;
		/* Two-line clamp keeps it from running into the actions */
		overflow: hidden;
		display: -webkit-box;
		-webkit-line-clamp: 3;
		line-clamp: 3;
		-webkit-box-orient: vertical;
	}

	.hero-actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-3);
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
	.btn-ghost {
		color: var(--bone-200);
	}
	.btn-ghost:hover {
		color: var(--bone-100);
		border-color: var(--bone-300);
	}

	/* — Skeletons. */
	.hero-skel {
		position: absolute;
		inset: 0;
	}
	.hero-skel-img {
		inline-size: 100%;
		block-size: 100%;
		background: var(--ink-100);
		animation: pulse 1.6s var(--ease-in-out) infinite;
	}
	.skel-card {
		display: grid;
		gap: var(--space-2);
	}
	.skel-poster {
		aspect-ratio: var(--poster-aspect);
		background: var(--ink-100);
		border-radius: var(--radius-card);
		animation: pulse 1.6s var(--ease-in-out) infinite;
	}
	.line {
		block-size: 0.75rem;
		background: var(--ink-100);
		border-radius: 2px;
	}
	.line-skel {
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
		.hero-skel-img,
		.skel-poster,
		.line-skel {
			animation: none;
		}
	}

	/* — Continue Watching cards. The poster gives identity, the EP tag
	     is the editorial overlay, the title + Resume CTA close the card. */
	.resume-card {
		scroll-snap-align: start;
		display: grid;
		grid-template-rows: auto auto;
		gap: var(--space-3);
		color: inherit;
		background: var(--ink-050);
		border: 1px solid var(--ink-200);
		border-radius: var(--radius-card);
		overflow: hidden;
		transition:
			transform var(--dur-med) var(--ease-out-elastic),
			border-color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft);
	}
	.resume-card:hover {
		transform: translateY(-3px);
		border-color: color-mix(in oklab, var(--accent) 60%, var(--ink-300));
		background: color-mix(in oklab, var(--accent) 6%, var(--ink-050));
	}
	.resume-poster {
		position: relative;
		aspect-ratio: 16 / 9;
		overflow: hidden;
		background: var(--ink-100);
	}
	.resume-poster img {
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
		object-position: center 30%;
		filter: brightness(0.85);
		transition: filter var(--dur-med) var(--ease-out-soft);
	}
	.resume-card:hover .resume-poster img {
		filter: brightness(1);
	}
	.resume-poster-placeholder {
		display: grid;
		place-items: center;
		inline-size: 100%;
		block-size: 100%;
		font-family: var(--font-display);
		font-style: italic;
		font-size: var(--type-display-s);
		color: var(--bone-300);
		background: linear-gradient(
			135deg,
			var(--ink-100),
			color-mix(in oklab, var(--accent) 15%, var(--ink-100))
		);
	}
	.resume-ep-tag {
		position: absolute;
		inset-block-start: var(--space-2);
		inset-inline-start: var(--space-2);
		display: inline-flex;
		align-items: baseline;
		gap: var(--space-1);
		padding: var(--space-1) var(--space-2);
		background: color-mix(in oklab, var(--ink-000) 78%, transparent);
		backdrop-filter: blur(4px);
		border: 1px solid color-mix(in oklab, var(--accent) 40%, var(--bone-400));
	}
	.resume-ep-key {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.resume-ep-num {
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: var(--type-body);
		color: var(--bone-100);
	}
	.resume-body {
		display: grid;
		gap: var(--space-2);
		padding: var(--space-3) var(--space-4) var(--space-4);
	}
	/* Anime title above the episode title — small mono eyebrow voice. */
	.resume-show {
		display: -webkit-box;
		-webkit-line-clamp: 1;
		line-clamp: 1;
		-webkit-box-orient: vertical;
		overflow: hidden;
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.resume-title {
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
		font-family: var(--font-display);
		font-style: italic;
		font-size: var(--type-body-l);
		line-height: var(--leading-tight);
		color: var(--bone-100);
	}
	.resume-title-faint {
		color: var(--bone-300);
	}
	.resume-cta {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
		transition: color var(--dur-fast) var(--ease-out-soft);
	}
	.resume-card:hover .resume-cta {
		color: var(--bone-100);
	}
	.resume-card-loading .resume-poster-placeholder {
		animation: pulse 1.6s var(--ease-in-out) infinite;
	}
	@media (prefers-reduced-motion: reduce) {
		.resume-card-loading .resume-poster-placeholder {
			animation: none;
		}
	}
</style>
