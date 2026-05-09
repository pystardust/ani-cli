<!--
  Kitsu search — dense scannable result grid. M3 design pass.
  Aesthetic note: late-night repertory cinema, programmed by a librarian.
  The motifs are restraint: serif voice, mono numerals, hairline rules.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import {
		imageProxyUrl,
		kitsuSearch,
		kitsuTopRated,
		kitsuTrending,
		type KitsuAnimeRef
	} from '$lib/api';
	import { accentFor } from '$lib/design/accent';
	import PosterCard from '$lib/components/PosterCard.svelte';
	import Strip from '$lib/components/Strip.svelte';

	let submitted = $state(''); // the query whose results are on screen.
	let results = $state<KitsuAnimeRef[] | null>(null);
	let error = $state<{ headline: string; detail: string | null } | null>(null);
	let busy = $state(false);

	// Discovery rows for the empty state — same data as the home page so
	// users have something to browse when they arrive without a query.
	let trending = $state<KitsuAnimeRef[] | null>(null);
	let topRated = $state<KitsuAnimeRef[] | null>(null);

	// Client-side sort + filter on the result set. Kitsu returns up
	// to ~20 hits per query; sorting/filtering is cheap to do here
	// rather than re-querying.
	type SortKey = 'relevance' | 'title' | 'year' | 'rating';
	const SORT_LABELS: Record<SortKey, string> = {
		relevance: 'Relevance',
		title: 'Title',
		year: 'Year',
		rating: 'Rating'
	};
	const SUBTYPES = ['TV', 'movie', 'special', 'OVA', 'ONA', 'music'];
	let sortKey = $state<SortKey>('relevance');
	const activeSubtypes = new SvelteSet<string>();

	function toggleSubtype(s: string) {
		if (activeSubtypes.has(s)) activeSubtypes.delete(s);
		else activeSubtypes.add(s);
	}

	const displayed = $derived.by(() => {
		if (!results) return null;
		let list = results.slice();
		if (activeSubtypes.size > 0) {
			list = list.filter((h) => activeSubtypes.has(h.subtype ?? 'TV'));
		}
		if (sortKey === 'title') {
			list.sort((a, b) =>
				a.canonical_title.localeCompare(b.canonical_title, undefined, { sensitivity: 'base' })
			);
		} else if (sortKey === 'year') {
			// Empty start_date sorts last regardless of direction.
			list.sort((a, b) => {
				const aY = a.start_date ?? '';
				const bY = b.start_date ?? '';
				if (!aY && bY) return 1;
				if (aY && !bY) return -1;
				return bY.localeCompare(aY);
			});
		} else if (sortKey === 'rating') {
			list.sort((a, b) => (b.average_rating ?? -1) - (a.average_rating ?? -1));
		}
		return list;
	});

	const count = $derived(displayed?.length ?? 0);

	onMount(() => {
		kitsuTrending()
			.then((t) => (trending = t))
			.catch(() => (trending = []));
		kitsuTopRated()
			.then((t) => (topRated = t))
			.catch(() => (topRated = []));
	});

	// Drive the search off the URL ?q=… param. The global topbar from
	// +layout.svelte writes that param on submit; this page reacts to
	// it. As a bonus, search history (browser back/forward) and deep
	// links work for free.
	$effect(() => {
		const q = page.url.searchParams.get('q')?.trim() ?? '';
		if (q && q !== submitted) {
			void runSearch(q);
		}
	});

	async function runSearch(q: string) {
		error = null;
		busy = true;
		submitted = q;
		try {
			results = await kitsuSearch(q);
		} catch (e) {
			error = describeError(e);
			results = null;
		} finally {
			busy = false;
		}
	}

	function describeError(e: unknown): { headline: string; detail: string | null } {
		if (typeof e === 'object' && e !== null) {
			const obj = e as Record<string, unknown>;
			const detail =
				typeof obj.detail === 'string'
					? obj.detail
					: typeof obj.kind === 'string'
						? obj.kind
						: null;
			return { headline: "Couldn't reach Kitsu.", detail };
		}
		return { headline: "Couldn't reach Kitsu.", detail: String(e) };
	}

	function posterFor(hit: KitsuAnimeRef): string | null {
		// `original` last — see PosterCard for the chain rationale.
		const url =
			hit.poster_image?.medium ??
			hit.poster_image?.large ??
			hit.poster_image?.small ??
			hit.poster_image?.original ??
			null;
		return imageProxyUrl(url);
	}
	function posterHoverFor(hit: KitsuAnimeRef): string | null {
		const url =
			hit.poster_image?.large ?? hit.poster_image?.medium ?? hit.poster_image?.original ?? null;
		return imageProxyUrl(url);
	}
	function yearOf(hit: KitsuAnimeRef): string | null {
		return hit.start_date ? hit.start_date.slice(0, 4) : null;
	}
	function ratingOf(hit: KitsuAnimeRef): string | null {
		if (hit.average_rating === null) return null;
		return (hit.average_rating / 10).toFixed(1);
	}
	function subtypeOf(hit: KitsuAnimeRef): string {
		return (hit.subtype ?? 'TV').toUpperCase();
	}
</script>

<svelte:head>
	<title>Search · ani-gui</title>
</svelte:head>

<div class="page">
	<header class="page-header">
		<h1 class="page-title">Search</h1>
		{#if !submitted}
			<p class="page-sub">Refine your search above. Results are grouped by source.</p>
		{/if}
	</header>

	<section class="results-meta" aria-live="polite">
		{#if submitted}
			<p class="eyebrow">
				<span class="eyebrow-key">Query</span>
				<span class="eyebrow-rule" aria-hidden="true"></span>
				<span class="eyebrow-value">{submitted}</span>
				<span class="eyebrow-sep" aria-hidden="true">·</span>
				{#if busy}
					<span class="eyebrow-count">searching…</span>
				{:else}
					<span class="eyebrow-count">
						<span class="num">{count}</span>
						<span class="word">{count === 1 ? 'result' : 'results'}</span>
					</span>
				{/if}
			</p>
		{:else}
			<p class="eyebrow">
				<span class="eyebrow-key">Catalogue</span>
				<span class="eyebrow-rule" aria-hidden="true"></span>
				<span class="eyebrow-value">via Kitsu</span>
			</p>
			<p class="eyebrow-caption">Trending now · Currently airing · Top&nbsp;20</p>
		{/if}
	</section>

	{#if submitted && results !== null && results.length > 0}
		<section class="controls" aria-label="Sort and filter results">
			<div class="control-group">
				<span class="control-label">Sort</span>
				<div class="seg" role="group" aria-label="Sort">
					{#each Object.entries(SORT_LABELS) as [key, label] (key)}
						<button
							type="button"
							class="seg-btn"
							class:active={sortKey === key}
							aria-pressed={sortKey === key}
							onclick={() => (sortKey = key as SortKey)}
						>
							{label}
						</button>
					{/each}
				</div>
			</div>
			<div class="control-group">
				<span class="control-label">Type</span>
				<div class="chips" role="group" aria-label="Filter by subtype">
					{#each SUBTYPES as s (s)}
						<button
							type="button"
							class="chip"
							class:active={activeSubtypes.has(s)}
							aria-pressed={activeSubtypes.has(s)}
							onclick={() => toggleSubtype(s)}
						>
							{s.toUpperCase()}
						</button>
					{/each}
					{#if activeSubtypes.size > 0}
						<button
							type="button"
							class="chip chip-clear"
							onclick={() => activeSubtypes.clear()}
							aria-label="Clear type filters"
						>
							Clear
						</button>
					{/if}
				</div>
			</div>
		</section>
	{/if}

	{#if error}
		<div class="state state-error" role="alert">
			<p class="state-headline">{error.headline}</p>
			{#if error.detail}<p class="state-detail">{error.detail}</p>{/if}
		</div>
	{:else if busy && !results}
		<ul class="grid" aria-busy="true" aria-label="Loading results">
			{#each Array.from({ length: 12 }, (_, k) => k) as i (i)}
				<li class="card card-skeleton" style="--i: {i}">
					<div class="poster poster-skeleton"></div>
					<div class="line line-skeleton" style="inline-size: 70%"></div>
					<div class="line line-skeleton line-thin" style="inline-size: 40%"></div>
				</li>
			{/each}
		</ul>
	{:else if results !== null && results.length === 0}
		<div class="state state-empty">
			<p class="state-headline">
				Nothing matched <em>“{submitted}”</em>.
			</p>
			<p class="state-detail">
				Kitsu's catalogue can be sparse for season-only or aliased titles. Try the romaji form, or a
				shorter prefix.
			</p>
		</div>
	{:else if !submitted}
		<!-- No query yet: surface the same discovery rows the home page uses. -->
		{#if trending && trending.length > 0}
			<Strip eyebrow="Trending now" caption="currently airing · top 20">
				{#each trending as hit (hit.id)}
					<PosterCard anime={hit} />
				{/each}
			</Strip>
		{/if}
		{#if topRated && topRated.length > 0}
			<Strip eyebrow="Top rated" caption="all-time · via Kitsu">
				{#each topRated as hit (hit.id)}
					<PosterCard anime={hit} />
				{/each}
			</Strip>
		{/if}
	{:else if displayed !== null && displayed.length === 0 && results !== null && results.length > 0}
		<div class="state state-empty">
			<p class="state-headline">No matches in the current filter.</p>
			<p class="state-detail">Clear or change the type filter above to see more results.</p>
		</div>
	{:else if displayed !== null}
		<ul class="grid">
			{#each displayed as hit, i (hit.id)}
				{@const poster = posterFor(hit)}
				{@const posterHover = posterHoverFor(hit)}
				{@const accent = accentFor(hit.id)}
				{@const year = yearOf(hit)}
				{@const rating = ratingOf(hit)}
				<li class="card" style="--i: {i}; --accent: {accent};">
					<a href={resolve('/anime/[id]', { id: hit.id })} class="card-link">
						<span class="poster">
							{#if poster}
								<img src={poster} alt="" loading="lazy" decoding="async" />
								{#if posterHover && posterHover !== poster}
									<img
										class="poster-hover"
										src={posterHover}
										alt=""
										loading="lazy"
										decoding="async"
									/>
								{/if}
							{:else}
								<span class="poster-placeholder" aria-hidden="true">
									<span class="poster-placeholder-title">{hit.canonical_title}</span>
								</span>
							{/if}
						</span>
						<span class="card-body">
							<span class="card-title">{hit.canonical_title}</span>
							<span class="card-meta">
								<span>{subtypeOf(hit)}</span>
								{#if year}
									<span class="card-meta-sep" aria-hidden="true">·</span>
									<span>{year}</span>
								{/if}
								{#if hit.episode_count}
									<span class="card-meta-sep" aria-hidden="true">·</span>
									<span>{hit.episode_count} eps</span>
								{/if}
								{#if rating}
									<span class="card-meta-sep" aria-hidden="true">·</span>
									<span class="card-meta-rating">{rating}</span>
								{/if}
							</span>
						</span>
					</a>
				</li>
			{/each}
		</ul>
	{/if}
</div>

<style>
	.page {
		max-inline-size: var(--content-max-wide);
		margin-inline: auto;
		padding: var(--space-7) var(--space-8) var(--space-8);
	}

	.page-header {
		margin-block-end: var(--space-6);
	}
	.page-title {
		margin: 0;
		font-family: var(--font-body);
		font-weight: 600;
		font-size: var(--type-display-l);
		line-height: var(--leading-tight);
		letter-spacing: var(--tracking-display);
		color: var(--bone-100);
	}
	.page-sub {
		margin: var(--space-3) 0 0;
		font-family: var(--font-body);
		font-size: var(--type-body);
		line-height: var(--leading-prose);
		color: var(--bone-300);
		max-inline-size: 60ch;
	}

	.results-meta {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--space-2);
		margin-block-end: var(--space-6);
		padding-block-end: var(--space-4);
		/* manga-page divider: 1px solid + 1px hairline 4px below */
		border-block-end: 1px solid var(--ink-200);
		box-shadow: 0 5px 0 -4px var(--ink-200);
	}
	.eyebrow-sep {
		color: color-mix(in oklab, var(--bone-100) 30%, transparent);
	}
	.eyebrow-count {
		display: inline-flex;
		align-items: baseline;
		gap: var(--space-2);
		font-variant-numeric: tabular-nums lining-nums;
	}
	.eyebrow-count .num {
		color: var(--bone-100);
	}
	.eyebrow-caption {
		margin: 0;
		font-family: var(--font-body);
		font-size: var(--type-meta);
		color: var(--bone-300);
	}

	/* — Grid: dense, never centered, never gappy. */
	.grid {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(11rem, 1fr));
		gap: var(--space-6) var(--space-5);
	}

	.card {
		--i: 0;
		animation: card-in var(--dur-med) var(--ease-out-soft) both;
		animation-delay: calc(var(--i) * var(--dur-stagger));
	}
	@keyframes card-in {
		from {
			opacity: 0;
			transform: translateY(8px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.card-link {
		display: block;
		color: inherit;
		transition: transform var(--dur-med) var(--ease-out-elastic);
		will-change: transform;
	}
	.card-link:hover {
		transform: translateY(-4px);
	}
	.card-link:focus-visible {
		outline: none;
	}
	.card-link:focus-visible .poster {
		box-shadow:
			var(--shadow-card-rest),
			0 0 0 2px var(--bone-100);
	}

	.poster {
		position: relative;
		display: block;
		aspect-ratio: var(--poster-aspect);
		background: var(--ink-100);
		border-radius: var(--radius-card);
		overflow: hidden;
		box-shadow: var(--shadow-card-rest);
		transition: box-shadow var(--dur-med) var(--ease-out-soft);
	}
	.card-link:hover .poster {
		box-shadow: var(--shadow-card-hover);
	}
	.poster img {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
		transition:
			opacity var(--dur-med) var(--ease-out-soft),
			transform var(--dur-slow) var(--ease-out-soft);
	}
	.poster img.poster-hover {
		opacity: 0;
	}
	.card-link:hover .poster img:not(.poster-hover) {
		transform: scale(1.04);
	}
	.card-link:hover .poster .poster-hover {
		opacity: 1;
		transform: scale(1.04);
	}

	.poster-placeholder {
		position: absolute;
		inset: 0;
		display: grid;
		place-items: center;
		padding: var(--space-4);
		background: linear-gradient(180deg, var(--ink-100) 0%, var(--ink-050) 100%);
		border-block-end: 1px solid var(--ink-200);
	}
	.poster-placeholder-title {
		font-family: var(--font-body);
		font-weight: 600;
		font-size: var(--type-display-m);
		line-height: var(--leading-tight);
		color: var(--bone-200);
		text-align: center;
		/* Keep long titles from blowing the card. */
		overflow: hidden;
		display: -webkit-box;
		-webkit-line-clamp: 5;
		line-clamp: 5;
		-webkit-box-orient: vertical;
	}

	.card-body {
		display: block;
		padding-block-start: var(--space-3);
	}
	.card-title {
		display: block;
		font-family: var(--font-body);
		font-weight: 500;
		font-size: 1rem;
		line-height: 1.3;
		color: var(--bone-100);
		/* clamp to 2 lines, keep grid rhythm */
		overflow: hidden;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
	}
	.card-meta {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		/* row-gap then column-gap — values keep tight horizontal rhythm
		   while wrapped lines get a smaller vertical gap. */
		gap: var(--space-1) var(--space-2);
		margin-block-start: var(--space-2);
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: var(--type-micro);
		font-weight: 500;
		color: var(--bone-200);
		letter-spacing: var(--tracking-meta);
	}
	/* Each value (and separator) stays as one unit when the row wraps.
	   Without nowrap, "366 eps" splits between "366" and "eps", so the
	   rating slides up next to "366" while "eps" drops to a new line —
	   which is the misalignment the user flagged. */
	.card-meta > span {
		white-space: nowrap;
	}
	.card-meta-sep {
		color: var(--bone-400);
	}
	.card-meta-rating::before {
		content: '★ ';
		color: var(--accent);
	}

	/* — Skeletons: opacity pulse, no shimmer-stripe gradient. */
	.card-skeleton .poster-skeleton {
		inline-size: 100%;
		aspect-ratio: var(--poster-aspect);
		background: var(--ink-100);
		border-radius: var(--radius-card);
		animation: pulse 1.6s var(--ease-in-out) infinite;
	}
	.line {
		block-size: 0.75rem;
		background: var(--ink-100);
		border-radius: 2px;
		margin-block-start: var(--space-3);
		animation: pulse 1.6s var(--ease-in-out) infinite;
	}
	.line-thin {
		block-size: 0.6rem;
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

	/* — Calm states. */
	.state {
		margin-block-start: var(--space-7);
		padding: var(--space-6);
		border-inline-start: 2px solid var(--ink-300);
	}
	.state-error {
		border-inline-start-color: var(--accent-oxblood);
	}
	.state-empty {
		border-inline-start-color: var(--bone-400);
	}
	.state-headline {
		margin: 0 0 var(--space-2);
		font-family: var(--font-body);
		font-weight: 600;
		font-size: var(--type-display-m);
		color: var(--bone-100);
		letter-spacing: -0.01em;
	}
	.state-headline em {
		font-style: normal;
		font-weight: 600;
		color: var(--bone-200);
	}
	.state-detail {
		margin: 0;
		font-family: var(--font-body);
		font-size: var(--type-body);
		color: var(--bone-300);
		max-inline-size: 60ch;
	}

	/* Sort + filter controls — sit between the eyebrow and the grid.
	   Stacked: Sort on its own row, Type on the next. */
	.controls {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		margin-block-end: var(--space-5);
	}
	.control-group {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		flex-wrap: wrap;
	}
	.control-label {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.seg {
		display: inline-flex;
		gap: 1px;
		padding: 2px;
		/* Bumped from ink-050 to ink-100 for visible contrast against
		   the page background — earlier the inactive segment track
		   blended with the surrounding ink-000 page surface. */
		background: var(--ink-100);
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-card);
	}
	.seg-btn {
		padding: 4px var(--space-3);
		background: transparent;
		border: 0;
		border-radius: calc(var(--radius-card) - 2px);
		font-family: var(--font-body);
		font-size: var(--type-body-s);
		color: var(--bone-200);
		cursor: pointer;
		transition: background var(--dur-fast) var(--ease-out-soft);
	}
	.seg-btn:hover:not(.active) {
		background: color-mix(in oklab, var(--bone-100) 6%, transparent);
		color: var(--bone-100);
	}
	.seg-btn.active {
		background: var(--accent-persimmon);
		color: var(--ink-000);
	}
	.chips {
		display: inline-flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
	.chip {
		padding: 4px var(--space-3);
		background: transparent;
		border: 1px solid var(--ink-200);
		border-radius: var(--radius-pill);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		color: var(--bone-300);
		cursor: pointer;
		transition:
			background var(--dur-fast) var(--ease-out-soft),
			color var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft);
	}
	.chip:hover:not(.active) {
		color: var(--bone-100);
		border-color: var(--ink-300);
	}
	.chip.active {
		background: color-mix(in oklab, var(--accent-persimmon) 22%, var(--ink-050));
		color: var(--bone-100);
		border-color: var(--accent-persimmon);
	}
	.chip-clear {
		border-style: dashed;
		text-transform: none;
		letter-spacing: normal;
	}
</style>
