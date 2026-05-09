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
	import { fade } from 'svelte/transition';
	import { resolve } from '$app/paths';
	import { goto } from '$app/navigation';
	import {
		altTitlesFromKitsu,
		historyList,
		imageProxyUrl,
		kitsuEpisodes,
		kitsuTopRated,
		kitsuTrending,
		markWatched,
		playStream,
		settingsGet,
		watchedAtAll,
		type Config,
		type HistoryEntry,
		type KitsuAnimeRef,
		type KitsuEpisode,
		type PlayProgress
	} from '$lib/api';
	import { accentFor } from '$lib/design/accent';
	import { EPISODES_KITSU_PAGE_SIZE, resolveHistoryEntry } from '$lib/history/resolve';
	import { resolveKitsuMatch } from '$lib/history/match';
	import { sortByWatchedAt } from '$lib/history/sort';
	import { nextHeroIndex, shouldRunHeroRotation } from '$lib/hero-rotation';
	import { getOrFire, makeKey } from '$lib/play/play-cache';
	import { buildPlayQuery } from '$lib/play/play-url';
	import Strip from '$lib/components/Strip.svelte';
	import PosterCard from '$lib/components/PosterCard.svelte';
	import LoadingOverlay from '$lib/components/LoadingOverlay.svelte';
	import ErrorOverlay from '$lib/components/ErrorOverlay.svelte';

	// Hero cycles through the top N trending titles. Rotation is slow
	// (~9s) and pauses while the cursor is over the hero, so it never
	// snatches a title out from under a user mid-read. Honors
	// prefers-reduced-motion (cycle disabled, first item shown
	// statically). Not a carousel — there's no scroll, no auto-advance
	// faster than read-time, and every item is one click away from the
	// strip below. AGENTS.md §7 anti-pattern is "auto-rotating carousels"
	// in the marketing-page sense; this is a programmed marquee.
	const HERO_ROTATION_COUNT = 3;
	const HERO_ROTATE_MS = 9000;

	let trending = $state<KitsuAnimeRef[] | null>(null);
	let topRated = $state<KitsuAnimeRef[] | null>(null);
	let history = $state<HistoryEntry[] | null>(null);
	let heroIndex = $state(0);
	let heroPaused = $state(false);
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
	// Continue Watching click handler state. The card resolves the
	// play directly (via getOrFire/playStream) and navigates to
	// /play/[id] — bypassing the detail page so the back button
	// returns to the home grid, not the detail view.
	let config = $state<Config | null>(null);
	let resumeBusy = $state<string | null>(null);
	let resumeProgress = $state<string | null>(null);
	let resumeFailure = $state<{ title: string; message: string } | null>(null);

	const heroRotation = $derived<KitsuAnimeRef[]>(
		trending && trending.length > 0 ? trending.slice(0, HERO_ROTATION_COUNT) : []
	);
	const featured = $derived(
		heroRotation.length > 0 ? heroRotation[heroIndex % heroRotation.length] : null
	);
	// Tail starts after the rotation set so the user doesn't see the
	// hero title also pinned as the first card of the row below.
	const trendingTail = $derived(trending ? trending.slice(HERO_ROTATION_COUNT) : []);
	const featuredAccent = $derived(featured ? accentFor(featured.id) : 'var(--accent-ink)');

	onMount(() => {
		// Fire all three in parallel; render whichever lands first.
		kitsuTrending()
			.then((t) => (trending = t))
			.catch((e) => (trendingError = describeError(e)));
		kitsuTopRated()
			.then((t) => (topRated = t))
			.catch((e) => (topRatedError = describeError(e)));
		// Settings drive mode/quality for the Continue Watching click
		// handler. Default {sub, best} when settings haven't loaded
		// yet — same fallback the click handler uses on /anime/[id].
		settingsGet()
			.then((c) => (config = c))
			.catch(() => {});
		Promise.all([historyList(), watchedAtAll().catch(() => ({}) as Record<string, number>)])
			.then(([h, watchedAt]) => {
				// Continue Watching ordering: GUI-stamped rows on top,
				// most recently watched first. Unstamped (CLI-only)
				// rows fall to the bottom in original file order. The
				// watched-at endpoint never throws — its catch above
				// degrades to "treat everything as unstamped," which
				// just preserves file order for everyone.
				history = sortByWatchedAt(h, watchedAt);
				// Two-stage lookup per entry, all routed through the
				// resolver in lib/history/resolve.ts so cour-split shows
				// (Stone Ocean Part 2 etc.) hit the right Kitsu episode
				// instead of collapsing onto Part 1's episode 1.
				//   1. resolveHistoryEntry(entry, null) — gives us the
				//      cour-stripped searchTitle + the episode number
				//      translated into Kitsu numbering.
				//   2. kitsuSearch(searchTitle) → first hit is the
				//      parent Kitsu anime (same for every cour).
				//   3. kitsuEpisodes(kitsuId, kitsuPage) → find the
				//      Kitsu-numbered episode we want; gives us the real
				//      thumbnail + title.
				// Fire-and-forget per entry; on failure the card
				// degrades gracefully (anime poster + entry's own title).
				history.forEach((entry: HistoryEntry) => {
					const preliminary = resolveHistoryEntry(entry, null);
					// resolveKitsuMatch checks the title-match cache first
					// (TITLE_MATCH_TTL = 30d), short-circuiting the
					// kitsuSearch + pickKitsuMatch round-trip on subsequent
					// loads. On miss it runs the live search + picker and
					// persists the resolved kitsu_id back into the cache.
					void resolveKitsuMatch(preliminary)
						.then((match) => {
							historyMatches = { ...historyMatches, [entry.id]: match };
							if (!match) return;
							const target = resolveHistoryEntry(entry, match);
							if (!target.kitsuEpisode) return;
							const kitsuPage = Math.max(
								1,
								Math.ceil(target.kitsuEpisode / EPISODES_KITSU_PAGE_SIZE)
							);
							void kitsuEpisodes(match.id, kitsuPage)
								.then((eps: KitsuEpisode[]) => {
									const ep =
										eps.find((e) => e.number === target.kitsuEpisode) ??
										eps.find((e) => e.relative_number === target.kitsuEpisode) ??
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

	// Hero auto-advance. Decision rules live in $lib/hero-rotation;
	// this effect is the runtime adapter (matchMedia probe + interval
	// management).
	$effect(() => {
		const reduced =
			typeof window !== 'undefined' &&
			window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
		if (
			!shouldRunHeroRotation({
				rotationLength: heroRotation.length,
				paused: heroPaused,
				prefersReducedMotion: !!reduced
			})
		) {
			return;
		}
		const id = window.setInterval(() => {
			heroIndex = nextHeroIndex(heroIndex, heroRotation.length);
		}, HERO_ROTATE_MS);
		return () => window.clearInterval(id);
	});

	function describeError(e: unknown): string {
		if (typeof e === 'object' && e !== null) {
			const obj = e as Record<string, unknown>;
			if (typeof obj.detail === 'string') return obj.detail;
			if (typeof obj.kind === 'string') return obj.kind;
		}
		return String(e);
	}

	/** Surface ani-cli failure kinds as user-readable copy. Mirrors the
	 *  same mapper on /anime/[id] and /play/[id]. */
	function describePlayFailure(e: unknown): string {
		const raw = describeError(e).toLowerCase();
		if (raw.includes('no_results')) {
			return "Couldn't find this title on the streaming source. The episode may not be available — try again later.";
		}
		if (raw.includes('scraper')) {
			return "Couldn't resolve a working stream right now. The streaming source looks unhappy — try again in a few minutes.";
		}
		if (raw.includes('timeout')) {
			return 'The streaming source took too long to respond. Try again in a few minutes.';
		}
		if (raw.includes('network') || raw.includes('upstream')) {
			return 'Network trouble reaching the streaming source. Check your connection and try again.';
		}
		return "Couldn't start this episode right now. Try again in a few minutes.";
	}

	function progressLabel(p: PlayProgress): string {
		if (p.kind === 'banner') return p.text;
		if (p.kind === 'links_fetched') return `${p.provider} ✓`;
		return p.text;
	}

	/** Click handler for a Continue Watching card. Resolves the play
	 *  through the same getOrFire/playStream pipeline /anime/[id]
	 *  uses, then navigates straight to /play/[id]?session=… —
	 *  bypassing the detail page. Once running, back from /play/[id]
	 *  returns home (where the user came from) instead of dropping
	 *  them on the detail view with a stale highlight ring. */
	async function startResume(match: KitsuAnimeRef, ep: number) {
		if (resumeBusy) return;
		const title = match.canonical_title;
		if (!title) return;
		const mode = (config?.mode === 'dub' ? 'dub' : 'sub') as 'sub' | 'dub';
		const quality = config?.quality ?? 'best';
		resumeBusy = match.id;
		resumeProgress = null;
		try {
			const session = await getOrFire(
				makeKey(match.id, ep, mode, quality),
				(emit, signal) =>
					playStream(
						{
							title,
							episode: String(ep),
							mode,
							quality,
							episode_count: match.episode_count ?? null,
							alt_titles: altTitlesFromKitsu(match),
							kitsu_id: match.id
						},
						emit,
						signal
					),
				(p) => {
					resumeProgress = progressLabel(p);
				}
			);
			void markWatched({
				title,
				episode: String(ep),
				mode,
				quality,
				episode_count: match.episode_count ?? null,
				alt_titles: altTitlesFromKitsu(match),
				kitsu_id: match.id
			}).catch(() => {});
			/* eslint-disable svelte/no-navigation-without-resolve */
			void goto(resolve('/play/[id]', { id: match.id }) + buildPlayQuery(session, ep));
			/* eslint-enable svelte/no-navigation-without-resolve */
		} catch (e) {
			resumeBusy = null;
			resumeProgress = null;
			resumeFailure = { title, message: describePlayFailure(e) };
		}
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

	// epOf / titleOf were absorbed into resolveHistoryEntry — see
	// $lib/history/resolve.ts (target.displayEpisode / displayTitle).
</script>

<svelte:head>
	<title>ani-gui</title>
</svelte:head>

<!-- Hero. -->
<section
	class="hero"
	style:--accent={featuredAccent}
	aria-label="Featured"
	onmouseenter={() => (heroPaused = true)}
	onmouseleave={() => (heroPaused = false)}
	onfocusin={() => (heroPaused = true)}
	onfocusout={() => (heroPaused = false)}
>
	{#if featured}
		{#key featured.id}
			{@const hero = heroFor(featured)}
			{@const synopsis = snippetOf(featured.synopsis)}
			<div class="hero-layer" in:fade={{ duration: 480 }} out:fade={{ duration: 480 }}>
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
						<a class="btn btn-glass" href={resolve('/anime/[id]', { id: featured.id })}>
							<span>View</span>
							<span aria-hidden="true">→</span>
						</a>
						<a class="btn btn-ghost" href={resolve('/search')}>
							<span aria-hidden="true">/</span>
							<span>Browse the catalogue</span>
						</a>
					</div>
				</div>
			</div>
		{/key}
		{#if heroRotation.length > 1}
			<div class="hero-pager" aria-label="Featured rotation">
				{#each heroRotation as item, i (item.id)}
					<button
						type="button"
						class="hero-pager-dot"
						class:active={i === heroIndex}
						onclick={() => (heroIndex = i)}
						aria-label={`Show ${item.canonical_title}`}
						aria-current={i === heroIndex ? 'true' : undefined}
					>
						<span class="hero-pager-track" aria-hidden="true"></span>
					</button>
				{/each}
			</div>
		{/if}
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
			{@const target = resolveHistoryEntry(entry, match ?? null)}
			{@const accent = accentFor(match?.id ?? entry.id)}
			{@const epThumb = imageProxyUrl(ep?.thumbnail?.original ?? null)}
			{@const animePoster = imageProxyUrl(
				match?.poster_image?.medium ??
					match?.poster_image?.large ??
					match?.poster_image?.small ??
					match?.poster_image?.original ??
					null
			)}
			{@const image = epThumb ?? animePoster}
			{@const resumable = match && target.kitsuEpisode !== null}
			{@const isResuming = resumeBusy === match?.id}
			<!-- Card is a button when we can resume (Kitsu match + an
			     episode to play); else falls through to /search as a
			     plain link. The href on the search-fallback path is
			     resolve()-produced; the lint rule's pattern matcher
			     doesn't recognise the ternary, so disabled around it. -->
			<!-- eslint-disable svelte/no-navigation-without-resolve -->
			{#if resumable && match}
				<button
					type="button"
					class="resume-card"
					class:resume-card-loading={match === undefined}
					class:resume-card-busy={isResuming}
					style="--accent: {accent};"
					disabled={!!resumeBusy && !isResuming}
					onclick={() => startResume(match, target.kitsuEpisode!)}
				>
					<span class="resume-poster">
						{#if image}
							<img src={image} alt="" loading="lazy" decoding="async" />
						{:else}
							<span class="resume-poster-placeholder" aria-hidden="true">
								{target.displayTitle.slice(0, 2).toUpperCase()}
							</span>
						{/if}
						<span class="resume-ep-tag" aria-hidden="true">
							<span class="resume-ep-key">EP</span>
							<span class="resume-ep-num">{target.displayEpisode}</span>
						</span>
					</span>
					<span class="resume-body">
						<span class="resume-show">{target.displayTitle}</span>
						{#if ep?.canonical_title}
							<span class="resume-title">{ep.canonical_title}</span>
						{:else}
							<span class="resume-title resume-title-faint">Episode {target.displayEpisode}</span>
						{/if}
						<span class="resume-cta">
							<span aria-hidden="true">↺</span>
							<span>Resume</span>
						</span>
					</span>
				</button>
			{:else}
				<a
					class="resume-card"
					class:resume-card-loading={match === undefined}
					style="--accent: {accent};"
					href={resolve('/search')}
				>
					<span class="resume-poster">
						{#if image}
							<img src={image} alt="" loading="lazy" decoding="async" />
						{:else}
							<span class="resume-poster-placeholder" aria-hidden="true">
								{target.displayTitle.slice(0, 2).toUpperCase()}
							</span>
						{/if}
						<span class="resume-ep-tag" aria-hidden="true">
							<span class="resume-ep-key">EP</span>
							<span class="resume-ep-num">{target.displayEpisode}</span>
						</span>
					</span>
					<span class="resume-body">
						<span class="resume-show">{target.displayTitle}</span>
						{#if ep?.canonical_title}
							<span class="resume-title">{ep.canonical_title}</span>
						{:else}
							<span class="resume-title resume-title-faint">Episode {target.displayEpisode}</span>
						{/if}
						<span class="resume-cta">
							<span aria-hidden="true">↺</span>
							<span>Resume</span>
						</span>
					</span>
				</a>
			{/if}
			<!-- eslint-enable svelte/no-navigation-without-resolve -->
		{/each}
	</Strip>
{/if}

<LoadingOverlay visible={resumeBusy !== null} progress={resumeProgress} />
{#if resumeFailure}
	<ErrorOverlay
		headline="Couldn't resume {resumeFailure.title}"
		body={resumeFailure.message}
		onDismiss={() => (resumeFailure = null)}
	/>
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
		/* Trimmed ~15%: was clamp(28rem, 70dvh, 44rem). The hero
		   was eating too much vertical space above the strips. */
		min-block-size: clamp(24rem, 60dvh, 37rem);
		overflow: hidden;
		background: var(--ink-050);
		margin-block-end: var(--space-7);
		/* Escape the layout shell's inline gutters — the banner
		   stays flush with the rail on the left and the window
		   edge on the right; only the strips below carry the
		   gutter inset. */
		margin-inline: calc(-1 * var(--space-7));
		inline-size: calc(100% + 2 * var(--space-7));
	}

	/* Each rotation tick mounts a new .hero-layer; the previous one
	   fades out simultaneously, giving a crossfade. Layer is absolute
	   so multiple can overlap during the transition without shifting
	   layout. */
	.hero-layer {
		position: absolute;
		inset: 0;
		display: flex;
		align-items: flex-end;
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
		/* Inline padding aligns the hero text with the strips below.
		   The hero box is full-bleed (escapes .main-area's left
		   gutter), so the body's inline-start padding has to make
		   up the gutter (space-7) PLUS the strip's own pad (space-8)
		   to land at the same vertical column as the strip eyebrows. */
		padding: var(--space-9) var(--space-8) var(--space-7) calc(var(--space-7) + var(--space-8));
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
		font-family: var(--font-body);
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
		font-weight: 600;
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-100);
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-control);
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft),
			box-shadow var(--dur-fast) var(--ease-out-soft),
			transform var(--dur-fast) var(--ease-out-soft);
	}
	.btn:hover {
		transform: translateY(-1px);
	}
	.btn-primary {
		/* Light text on accent bg — the previous ink-on-accent failed
		   contrast on dark accents (oxblood + black text was the user
		   complaint). Bone-100 on the brightened accents hits AA. */
		background: var(--accent);
		color: var(--bone-100);
		border-color: color-mix(in oklab, var(--accent) 70%, var(--bone-100));
	}
	.btn-primary:hover {
		background: color-mix(in oklab, var(--accent) 80%, var(--bone-100));
		border-color: var(--bone-100);
		box-shadow: 0 8px 20px -8px color-mix(in oklab, var(--accent) 60%, transparent);
	}
	.btn-ghost {
		color: var(--bone-200);
	}
	.btn-ghost:hover {
		color: var(--bone-100);
		border-color: var(--bone-300);
	}

	/* .btn-glass lives in tokens.css and is parametric on --btn-glow.
	   The home hero's CTA inherits the default (--brand). */

	/* Hero pager: thin underline-style dots in the bottom-right corner,
	   scoped to the hero. Active dot fills with the current accent;
	   inactive dots are hairlines. Click jumps the rotation. */
	.hero-pager {
		position: absolute;
		z-index: 3;
		inset-block-end: var(--space-5);
		/* Hero box now extends past .main-area's right gutter
		   (full-bleed), so push the pager inward by that gutter
		   so it lines up with the strip's right edge below. */
		inset-inline-end: calc(var(--space-8) + var(--space-7));
		display: inline-flex;
		gap: var(--space-2);
		align-items: center;
	}
	.hero-pager-dot {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		inline-size: 2.5rem;
		block-size: 1.25rem;
		padding: 0;
		background: transparent;
		border: 0;
		cursor: pointer;
	}
	.hero-pager-track {
		display: block;
		inline-size: 100%;
		block-size: 2px;
		background: color-mix(in oklab, var(--bone-100) 25%, transparent);
		border-radius: 999px;
		transition:
			background var(--dur-fast) var(--ease-out-soft),
			block-size var(--dur-fast) var(--ease-out-soft);
	}
	.hero-pager-dot:hover .hero-pager-track {
		background: color-mix(in oklab, var(--bone-100) 60%, transparent);
	}
	.hero-pager-dot.active .hero-pager-track {
		background: var(--accent);
		block-size: 3px;
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
		font-family: var(--font-body);
		font-weight: 600;
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
		font-family: var(--font-body);
		font-weight: 500;
		font-size: 1rem;
		line-height: 1.3;
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
