<!--
  Anime detail v2 — editorial spread, but no longer synopsis-dominant.
  Composition:
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
	import { onMount, tick } from 'svelte';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import {
		imageProxyUrl,
		kitsuAnimeDetail,
		kitsuEpisodes,
		kitsuSearch,
		play,
		playExternal,
		settingsGet,
		settingsPut,
		type Config,
		type KitsuAnimeRef,
		type KitsuEpisode
	} from '$lib/api';
	import { cubicOut } from 'svelte/easing';
	import { SvelteMap } from 'svelte/reactivity';
	import LoadingOverlay from '$lib/components/LoadingOverlay.svelte';
	import PosterCard from '$lib/components/PosterCard.svelte';
	import Strip from '$lib/components/Strip.svelte';
	import { accentFor } from '$lib/design/accent';
	import {
		decideEpisodeFetchAction,
		parseEpParam,
		parsePageParam,
		shouldHighlight
	} from '$lib/history/url-deeplink';

	let detail = $state<KitsuAnimeRef | null>(null);
	let error = $state<{ headline: string; detail: string | null } | null>(null);
	let scrollY = $state(0);

	// Episode list — fetched in parallel with the detail. Holds the
	// CURRENT page only (not concatenated) so a 500-episode show doesn't
	// produce a 500-tile vertical wall. Pagination replaces these.
	//
	// Two page sizes exist:
	//   UI_PAGE_SIZE  — visible tile count per UI page. ~3 rows × 4 cols
	//                   on widescreen, 4 × 3 on the narrower body width.
	//                   Smaller than the Kitsu cap so the row count feels
	//                   contained instead of running past the fold.
	//   KITSU_PAGE_SIZE — Kitsu's hard `page[limit]` cap. Backend fetches
	//                   come in multiples of this; the UI window slices
	//                   into them.
	// One UI page therefore maps to 1 or 2 Kitsu pages, kept in an
	// in-memory cache (kitsuPageCache) so prev/next is instant after
	// the first hop. Adjacent UI pages are prefetched after every load.
	let episodes = $state<KitsuEpisode[] | null>(null);
	let episodesError = $state<string | null>(null);
	let episodesPage = $state(1);
	let episodesLoading = $state(false);
	let jumpInput = $state('');
	// Episode number to highlight after a deep link from Continue
	// Watching (?ep=…). Cleared on a timeout so the accent ring isn't
	// sticky.
	let highlightEp = $state<number | null>(null);
	const UI_PAGE_SIZE = 12;
	const KITSU_PAGE_SIZE = 20;
	// SvelteMap (vs plain Map) keeps the eslint reactivity rule happy.
	// The cache itself doesn't drive any reactive UI — the windowed slice
	// gets stored back into `episodes`.
	const kitsuPageCache = new SvelteMap<number, KitsuEpisode[]>();
	const totalEpisodePages = $derived.by(() => {
		const total = detail?.episode_count;
		if (!total) return null;
		return Math.max(1, Math.ceil(total / UI_PAGE_SIZE));
	});
	const epStart = $derived((episodesPage - 1) * UI_PAGE_SIZE + 1);
	const epEnd = $derived((episodesPage - 1) * UI_PAGE_SIZE + (episodes?.length ?? 0));

	function kitsuPagesForUiPage(uiPage: number): number[] {
		const start = (uiPage - 1) * UI_PAGE_SIZE + 1;
		const end = uiPage * UI_PAGE_SIZE;
		const first = Math.ceil(start / KITSU_PAGE_SIZE);
		const last = Math.ceil(end / KITSU_PAGE_SIZE);
		const out: number[] = [];
		for (let k = first; k <= last; k++) out.push(k);
		return out;
	}

	async function getKitsuPage(p: number): Promise<KitsuEpisode[]> {
		if (!id) return [];
		const cached = kitsuPageCache.get(p);
		if (cached) return cached;
		const eps = await kitsuEpisodes(id, p);
		kitsuPageCache.set(p, eps);
		return eps;
	}

	async function fetchEpisodesPage(p: number, opts: { initial?: boolean } = {}) {
		if (!id) return;
		const wantPage = Math.max(1, p);
		episodesLoading = true;
		try {
			const start = (wantPage - 1) * UI_PAGE_SIZE + 1;
			const end = wantPage * UI_PAGE_SIZE;
			const merged = (await Promise.all(kitsuPagesForUiPage(wantPage).map(getKitsuPage))).flat();
			const windowed = merged.filter((ep) => {
				const n = ep.number ?? ep.relative_number ?? -1;
				return n >= start && n <= end;
			});
			episodes = windowed;
			episodesPage = wantPage;
			episodesError = null;
			void prefetchAdjacent(wantPage);
		} catch (e) {
			if (opts.initial) episodes = [];
			episodesError = describeErrorString(e);
		} finally {
			episodesLoading = false;
		}
	}

	// Warm the cache for ±1 UI pages so prev/next feels instant. Quietly
	// swallows failures — the user-driven path surfaces errors itself.
	function prefetchAdjacent(uiPage: number) {
		const cap = totalEpisodePages;
		const targets: number[] = [];
		if (cap === null || uiPage + 1 <= cap) targets.push(uiPage + 1);
		if (uiPage - 1 >= 1) targets.push(uiPage - 1);
		for (const t of targets) {
			for (const k of kitsuPagesForUiPage(t)) {
				if (!kitsuPageCache.has(k)) {
					void getKitsuPage(k).catch(() => {});
				}
			}
		}
	}

	function gotoPage(p: number) {
		const cap = totalEpisodePages ?? p;
		const next = Math.min(Math.max(1, p), cap);
		if (next === episodesPage) return;
		// Drive the URL — the `?page=` change wakes the $effect above,
		// which calls fetchEpisodesPage. Calling fetchEpisodesPage
		// directly here would race the effect: the effect re-runs on
		// the resulting `episodesPage` write, sees the URL still on
		// page 1, and immediately fetches page 1 again — which lands
		// just after the page-2 tiles finish their entrance animation,
		// so the UI snaps back to page 1.
		// Build the next query string by hand instead of mutating a
		// URLSearchParams (svelte/prefer-svelte-reactivity flags any
		// mutable instance of the built-in class). It's a one-shot
		// serialization here, so we just iterate the existing params,
		// drop any prior `page=`, and append our new value.
		const parts: string[] = [];
		for (const [k, v] of page.url.searchParams.entries()) {
			if (k === 'page') continue;
			parts.push(`${encodeURIComponent(k)}=${encodeURIComponent(v)}`);
		}
		parts.push(`page=${next}`);
		// goto target IS resolve()-produced; the rule pattern-matches a
		// literal `goto(resolve(...))` call and trips on the template
		// literal that interpolates resolve() with the query string.
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		void goto(`${resolve('/anime/[id]', { id })}?${parts.join('&')}`, {
			replaceState: true,
			keepFocus: true,
			noScroll: true
		});
	}

	function jumpToEpisode(event: SubmitEvent) {
		event.preventDefault();
		const n = parseInt(jumpInput, 10);
		if (Number.isNaN(n) || n < 1) return;
		const target = Math.ceil(n / UI_PAGE_SIZE);
		gotoPage(target);
		jumpInput = '';
	}

	// Custom transition: tiles fade up + scale + de-blur into place.
	// Combined with cubicOut, the early motion is fast and the tile
	// decelerates as it lands. With a per-index delay this gives a
	// staggered "settle" feel between page transitions. Reduced motion
	// drops to a flat fade.
	//
	// css(t, u): for `in:`, t goes 0→1 and u = 1−t. So at t=0 the tile
	// starts at opacity 0, scaled to 0.9, translated +28px below its
	// final position, and blurred by 8px; it eases out to its rest
	// state by t=1. The same function is reused for `out:` via
	// settleOut, which mirrors the easing curve on the way out (drop
	// down + fade + soft blur). Stagger comes from `delay` chosen by
	// the caller, not the function itself, so cards on different
	// indices run with different offsets.
	function settle(
		_node: Element,
		{ delay = 0, duration = 620 }: { delay?: number; duration?: number } = {}
	) {
		const reduced =
			typeof window !== 'undefined' &&
			window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
		return {
			delay,
			duration: reduced ? 0 : duration,
			easing: cubicOut,
			css: (t: number, u: number) =>
				reduced
					? `opacity: ${t};`
					: `opacity: ${t}; transform: translateY(${u * 28}px) scale(${0.9 + t * 0.1}); filter: blur(${u * 8}px);`
		};
	}

	function settleOut(
		_node: Element,
		{ delay = 0, duration = 320 }: { delay?: number; duration?: number } = {}
	) {
		const reduced =
			typeof window !== 'undefined' &&
			window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
		return {
			delay,
			duration: reduced ? 0 : duration,
			easing: cubicOut,
			// For `out:` transitions Svelte runs t from 1→0; u = 1−t. So
			// at the start t=1 (rest), at the end t=0 (gone). Mirror the
			// in: shape but drop downward instead of up so it doesn't
			// feel like the same gesture playing in reverse.
			css: (t: number, u: number) =>
				reduced
					? `opacity: ${t};`
					: `opacity: ${t}; transform: translateY(${u * -16}px) scale(${0.94 + t * 0.06}); filter: blur(${u * 4}px);`
		};
	}

	let config = $state<Config | null>(null);
	let configError = $state<string | null>(null);

	// Synopsis collapse/expand. Default collapsed (long synopses are
	// dominant otherwise); expands on user request.
	let synopsisExpanded = $state(false);

	// Similar titles strip (below the body). Searches Kitsu by the first
	// 1-2 words of the canonical_title and filters out the current anime.
	// Cheap and effective for franchise neighbours; richer
	// recommendations come once AniList wires up (M3+).
	let similar = $state<KitsuAnimeRef[] | null>(null);

	// Inline status banner when an action isn't wired yet (Play/Download/External
	// hit allanime, which is M2). Kept tight; not a modal.
	let actionNotice = $state<string | null>(null);
	// True while a play/playExternal request is in flight. Buttons
	// disable themselves to keep the user from double-clicking ani-cli
	// into a stack of concurrent spawns.
	let actionBusy = $state(false);

	const id = $derived(page.params.id ?? '');
	const accent = $derived(id ? accentFor(id) : 'var(--accent-ink)');

	// Episodes-fallback derivations live in script so they can be used in
	// the markup without {@const} (which only accepts Svelte-block parents).
	const epPlaceholderCount = $derived(
		detail?.episode_count ? Math.min(UI_PAGE_SIZE, detail.episode_count) : UI_PAGE_SIZE
	);
	const showEpPlaceholders = $derived(episodes !== null && episodes.length === 0);

	const QUALITIES: Array<{ key: string; label: string }> = [
		{ key: 'best', label: 'Best' },
		{ key: '1080', label: '1080' },
		{ key: '720', label: '720' },
		{ key: '480', label: '480' },
		{ key: 'worst', label: 'Worst' }
	];

	onMount(() => {
		if (!id) {
			error = { headline: 'No anime selected.', detail: 'URL is missing the id segment.' };
			return;
		}
		// Fire detail + settings in parallel; episodes are driven by
		// the URL-param effect below, not from here.
		void kitsuAnimeDetail(id)
			.then((d) => {
				detail = d;
				const seed = (d.canonical_title ?? '').split(/\s+/).slice(0, 2).join(' ').trim();
				if (seed.length >= 2) {
					void kitsuSearch(seed)
						.then((hits) => {
							similar = hits.filter((h) => h.id !== d.id).slice(0, 12);
						})
						.catch(() => {
							similar = [];
						});
				} else {
					similar = [];
				}
			})
			.catch((e) => (error = describeError(e)));
		void settingsGet()
			.then((c) => (config = c))
			.catch((e) => (configError = describeErrorString(e)));
	});

	// Drive the episode page off the URL ?page= param. Re-runs on
	// every URL change so navigation between two /anime/[id] entries
	// with different query strings works (SvelteKit reuses the
	// component when the route id is the same; a plain onMount fires
	// only once). The decision rule lives in $lib/history/url-deeplink.
	$effect(() => {
		if (!id) return;
		const targetPage = parsePageParam(page.url.searchParams);
		const action = decideEpisodeFetchAction({
			episodes,
			episodesPage,
			episodesLoading,
			targetPage
		});
		if (action === 'fetch-initial') {
			void fetchEpisodesPage(targetPage, { initial: true });
		} else if (action === 'fetch') {
			void fetchEpisodesPage(targetPage);
		}
	});

	// Deep-link episode highlight. Fires once per ?ep= value (tracked
	// via consumedEp) once the matching tile is in `episodes`. Scrolls
	// the tile into view and auto-clears the accent ring after ~3.2s.
	let consumedEp: number | null = null;
	$effect(() => {
		const target = parseEpParam(page.url.searchParams);
		if (!shouldHighlight({ target, consumed: consumedEp, episodes })) return;
		// shouldHighlight has already validated target is non-null + present.
		const epParam = target as number;
		consumedEp = epParam;
		highlightEp = epParam;
		void tick().then(() => {
			document
				.querySelector(`[data-ep-num="${epParam}"]`)
				?.scrollIntoView({ behavior: 'smooth', block: 'center' });
		});
		window.setTimeout(() => {
			if (highlightEp === epParam) highlightEp = null;
		}, 3200);
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

	// Title we feed to ani-cli's search. The backend's run_debug picks
	// the first allanime match, so a stable canonical title is the
	// best signal we have. KitsuAnimeRef.canonical_title is non-null
	// per the type, but the detail isn't populated until kitsuAnimeDetail
	// resolves — guard for the null pre-load state.
	function playTitle(): string {
		return detail?.canonical_title ?? '';
	}

	/** Default to the show's first episode for the hero "Play" button. */
	function defaultEpisode(): number {
		return 1;
	}

	async function startPlay(ep: number) {
		const title = playTitle();
		if (!title) {
			notify('No title available for playback yet.');
			return;
		}
		const mode = (config?.mode === 'dub' ? 'dub' : 'sub') as 'sub' | 'dub';
		// LoadingOverlay binds to actionBusy; it stays up until goto
		// fires (which unmounts this page) or the catch branch resets
		// busy and surfaces an error toast.
		actionBusy = true;
		actionNotice = null;
		try {
			const session = await play({
				title,
				episode: String(ep),
				mode,
				quality: config?.quality
			});
			actionNotice = null;
			// The target is built from `resolve()` plus a query string;
			// the no-resolve lint rule's pattern matcher only recognises
			// a literal `goto(resolve(...))`, so we suppress around it.
			// `kind` rides along so the player page knows whether to
			// mount hls.js or a plain `<video src>`.
			/* eslint-disable svelte/no-navigation-without-resolve */
			void goto(
				resolve('/play/[id]', { id }) +
					`?session=${encodeURIComponent(session.session_id)}` +
					`&episode=${ep}&kind=${session.media_kind}`
			);
			/* eslint-enable svelte/no-navigation-without-resolve */
		} catch (e) {
			actionBusy = false;
			notify(describeErrorString(e));
		}
	}

	async function startExternal(ep: number) {
		const title = playTitle();
		if (!title) {
			notify('No title available for playback yet.');
			return;
		}
		const mode = (config?.mode === 'dub' ? 'dub' : 'sub') as 'sub' | 'dub';
		// Same overlay pattern as the embedded path — the resolution
		// wait is the same chain, only the terminal action differs.
		actionBusy = true;
		actionNotice = null;
		try {
			await playExternal({
				title,
				episode: String(ep),
				mode,
				quality: config?.quality
			});
			actionBusy = false;
			notify(`Episode ${ep} sent to external player.`);
		} catch (e) {
			actionBusy = false;
			notify(describeErrorString(e));
		}
	}

	function onPlay() {
		void startPlay(defaultEpisode());
	}
	function onDownload() {
		// Download is a separate flow (-d) that's not part of this M2
		// slice — the upstream URL resolution path is the same, but
		// the terminal action would invoke aria2c via ani-cli. Tracked
		// as a follow-up; the toast keeps the button discoverable.
		notify('Downloads land in a follow-up — the backend chain is the same.');
	}
	function onExternal() {
		void startExternal(defaultEpisode());
	}
	function onPickEpisode(n: number) {
		void startPlay(n);
	}
</script>

<svelte:head>
	<title>{detail?.canonical_title ?? 'Loading'} · ani-gui</title>
</svelte:head>

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
					<button
						type="button"
						class="btn btn-glass"
						style:--btn-glow="var(--accent)"
						onclick={onPlay}
						disabled={actionBusy}
					>
						<span aria-hidden="true">▸</span>
						<span>Play episode 1</span>
					</button>
					<button type="button" class="btn btn-outline" onclick={onDownload}>
						<span aria-hidden="true">↓</span>
						<span>Download</span>
					</button>
					<button type="button" class="btn btn-ghost" onclick={onExternal} disabled={actionBusy}>
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

		<!-- Body: synopsis + episodes stacked vertically. The previous
		     side-by-side layout looked unbalanced when one was much taller
		     than the other (long synopsis + 12-ep show, or short synopsis
		     + 1100-ep show). Stacked, both panels use the full editorial
		     column width and visually breathe. -->
		<section class="body">
			<div class="body-col body-col-prose">
				<h2 class="section-eyebrow">Synopsis</h2>
				{#if detail.synopsis}
					<div class="prose-wrap" class:expanded={synopsisExpanded}>
						<p class="prose">{detail.synopsis}</p>
						<div class="prose-fade" aria-hidden="true"></div>
					</div>
					{#if detail.synopsis.length > 360}
						<button
							type="button"
							class="prose-toggle"
							onclick={() => (synopsisExpanded = !synopsisExpanded)}
							aria-expanded={synopsisExpanded}
						>
							{synopsisExpanded ? 'Read less' : 'Read more'}
						</button>
					{/if}
				{:else}
					<p class="prose-empty">No synopsis on file at Kitsu.</p>
				{/if}
			</div>

			<div class="body-col body-col-episodes">
				<h2 class="section-eyebrow">
					<span>Episodes</span>
					<span class="section-eyebrow-faint">
						{#if episodes && episodes.length > 0 && detail.episode_count}
							{epStart}–{epEnd} of {detail.episode_count}
						{:else if episodes && episodes.length > 0}
							page {episodesPage}
						{:else if episodesError}
							unavailable
						{:else}
							loading
						{/if}
					</span>
				</h2>

				{#if (totalEpisodePages !== null && totalEpisodePages > 1) || (episodes && episodes.length === UI_PAGE_SIZE)}
					<div class="ep-controls">
						<form class="ep-jump" onsubmit={jumpToEpisode}>
							<label class="ep-jump-label">
								<span class="ep-jump-key">Jump</span>
								<input
									type="number"
									min="1"
									max={detail.episode_count ?? 9999}
									step="1"
									placeholder="ep #"
									aria-label="Jump to episode number"
									bind:value={jumpInput}
								/>
							</label>
							<button
								type="submit"
								class="ep-jump-go"
								disabled={!jumpInput || episodesLoading}
								aria-label="Go to episode"
							>
								↵
							</button>
						</form>
						<div class="ep-pager" role="group" aria-label="Episode pagination">
							<button
								type="button"
								class="ep-pager-btn"
								onclick={() => gotoPage(episodesPage - 1)}
								disabled={episodesPage <= 1 || episodesLoading}
								aria-label="Previous page"
							>
								←
							</button>
							<span class="ep-pager-state">
								{episodesPage}{#if totalEpisodePages}<span class="ep-pager-of">
										/ {totalEpisodePages}</span
									>{/if}
							</span>
							<button
								type="button"
								class="ep-pager-btn"
								onclick={() => gotoPage(episodesPage + 1)}
								disabled={(totalEpisodePages !== null && episodesPage >= totalEpisodePages) ||
									episodesLoading ||
									(episodes !== null && episodes.length < UI_PAGE_SIZE)}
								aria-label="Next page"
							>
								→
							</button>
						</div>
					</div>
				{/if}
				<!--
				  No {#key episodesPage} wrapping the <ul>: that destroyed
				  the parent on every page change, taking the children with
				  it before their out: transitions could run, which is why
				  the previous build looked like an instant swap. Now the
				  <ul> stays mounted; the keyed each block (key=ep.id) does
				  the diff. Old LIs run out:settleOut, new LIs run
				  in:settle, with a stagger via per-index delay so episodes
				  land left-to-right, top-to-bottom.
				-->
				<ul class="ep-grid">
					{#if episodes === null}
						<!-- Skeleton while fetch is in flight -->
						{#each Array.from({ length: 6 }, (_, k) => k) as i (i)}
							<li>
								<div class="ep-tile ep-tile-skel" aria-hidden="true">
									<div class="ep-thumb ep-thumb-skel"></div>
									<div class="ep-foot-skel"></div>
								</div>
							</li>
						{/each}
					{:else if episodes.length > 0}
						<!-- Real Kitsu data path; per-tile staggered enter for a
						     premium feel — tiles flow in left-to-right, top-to-bottom. -->
						{#each episodes as ep, i (ep.id)}
							{@const thumb = imageProxyUrl(ep.thumbnail?.original ?? null)}
							{@const num = ep.number ?? ep.relative_number ?? null}
							<li
								class:ep-highlight={num !== null && num === highlightEp}
								data-ep-num={num ?? ''}
								in:settle={{ duration: 620, delay: i * 45 }}
								out:settleOut={{ duration: 320, delay: i * 18 }}
							>
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
					{:else if showEpPlaceholders}
						<!-- Fallback: Kitsu didn't have episode data, but episode_count
						     gives us a usable count. Render numbered placeholder tiles
						     so the user isn't blocked from poking the panel. -->
						{#each Array.from({ length: epPlaceholderCount }, (_, k) => k + 1) as n, i (n)}
							<li
								class:ep-highlight={n === highlightEp}
								data-ep-num={n}
								in:settle={{ duration: 580, delay: i * 40 }}
								out:settleOut={{ duration: 300, delay: i * 16 }}
							>
								<button type="button" class="ep-tile" onclick={() => onPickEpisode(n)}>
									<span class="ep-thumb">
										<span class="ep-thumb-placeholder" aria-hidden="true">
											{n.toString().padStart(2, '0')}
										</span>
										<span class="ep-tag" aria-hidden="true">
											<span class="ep-tag-key">Ep</span>
											<span class="ep-tag-num">{n}</span>
										</span>
									</span>
									<span class="ep-foot">
										<span class="ep-title">Episode {n}</span>
										<span class="ep-meta">—</span>
									</span>
								</button>
							</li>
						{/each}
					{/if}
				</ul>
				{#if episodesError}
					<p class="ep-grid-foot ep-grid-foot-warn">
						Episode metadata unavailable from Kitsu — playable list above is a fallback.
					</p>
				{:else if episodes && episodes.length > 0}
					<p class="ep-grid-foot">
						Thumbnails + titles via Kitsu. Tap to play once the allanime bridge lands.
					</p>
				{:else}
					<p class="ep-grid-foot">Tap to play once the allanime bridge lands.</p>
				{/if}
			</div>
		</section>

		<!-- Similar titles strip — placeholder for AniList recommendations.
		     Today: re-uses kitsuSearch with the canonical title's first
		     1-2 words to surface franchise neighbours / look-alikes. -->
		{#if similar && similar.length > 0}
			<section class="similar">
				<Strip eyebrow="Similar titles" caption="via Kitsu search">
					{#each similar as hit (hit.id)}
						<PosterCard anime={hit} />
					{/each}
				</Strip>
			</section>
		{/if}
	{/if}
</main>

<LoadingOverlay visible={actionBusy} />

<style>
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
	/* btn-primary is no longer used here — Play episode 1 uses
	   .btn-glass with --btn-glow=var(--accent). Removed to satisfy the
	   unused-selector check; restore if a future button needs the flat
	   accent background. */
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

	/* — Body: vertical stack of synopsis → episodes. The earlier 2-col
	     layout produced an imbalance whenever one block was much taller
	     than the other. Stacked, both panels span the full editorial
	     column width and never compete for vertical alignment.
	     Container is the wide cap (matches the hero above), so widescreens
	     don't leave huge empty side margins. The synopsis used to cap
	     at 70ch for prose readability, but that left a ragged disjoint
	     against the much-wider episode grid below. Now both blocks share
	     the full body width — synopsis is short enough that the wider
	     line length doesn't hurt comprehension, and the collapsed
	     preview shows more text in the same vertical space. */
	.body {
		display: flex;
		flex-direction: column;
		gap: var(--space-8);
		max-inline-size: var(--content-max-wide);
		margin: var(--space-7) auto 0;
		padding-inline: var(--space-8);
	}

	.body-col-prose {
		inline-size: 100%;
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
	/* Synopsis — collapsed by default to a 5-line preview with a soft
	   gradient fade at the bottom; expands on user click. The font is
	   bumped a notch (display-m, was body-l) so the prose feels like a
	   proper editorial spread instead of body chrome. */
	.prose-wrap {
		position: relative;
		max-block-size: 9.5rem;
		overflow: hidden;
		transition: max-block-size var(--dur-slow) var(--ease-out-soft);
	}
	.prose-wrap.expanded {
		max-block-size: 200rem;
	}
	.prose {
		margin: 0;
		font-family: var(--font-display);
		font-size: var(--type-display-m);
		line-height: 1.5;
		color: var(--bone-100);
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
	.prose-fade {
		position: absolute;
		inset-block-end: 0;
		inset-inline: 0;
		block-size: 4rem;
		background: linear-gradient(180deg, transparent 0%, var(--ink-000) 90%);
		pointer-events: none;
		transition: opacity var(--dur-fast) var(--ease-out-soft);
	}
	.prose-wrap.expanded .prose-fade {
		opacity: 0;
	}
	.prose-toggle {
		margin-block-start: var(--space-3);
		padding: 4px 0;
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-200);
		border-block-end: 1px solid var(--bone-300);
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft);
	}
	.prose-toggle:hover {
		color: var(--bone-100);
		border-block-end-color: var(--bone-100);
	}
	.prose-empty {
		margin: 0;
		font-family: var(--font-display);
		font-style: italic;
		color: var(--bone-300);
	}

	/* — Episodes panel. Auto-fill grid that adapts to the viewport:
	     ~3-4 columns at standard widths, 4-5 at widescreen. Tiles always
	     ≥ 18rem wide so thumbnails stay readable and titles fit. */
	.body-col-episodes {
		inline-size: 100%;
	}
	.ep-grid {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(18rem, 1fr));
		gap: var(--space-4);
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
		/* Origin sits low so scale-up on hover lifts upward toward the
		   poster + thumbnail rather than pushing into the next row. */
		transform-origin: 50% 80%;
		transition:
			transform var(--dur-med) var(--ease-out-elastic),
			border-color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft),
			box-shadow var(--dur-med) var(--ease-out-soft);
	}
	.ep-tile:hover {
		/* More expressed pop: lift, scale, accent-tinted shadow halo. */
		transform: translateY(-4px) scale(1.04);
		z-index: 1;
		border-color: color-mix(in oklab, var(--accent) 80%, var(--ink-300));
		box-shadow:
			0 12px 28px -6px color-mix(in oklab, var(--accent) 28%, transparent),
			0 4px 10px -4px rgb(0 0 0 / 0.45);
	}

	/* Deep-link highlight: when arriving from Continue Watching with
	   ?ep=N, the matching tile pulses an accent ring twice and stays
	   ringed for a beat so the user sees "this is the one you were
	   on." Class is auto-removed after ~3.2s by the script. */
	.ep-grid li.ep-highlight .ep-tile {
		border-color: color-mix(in oklab, var(--accent) 90%, var(--bone-100));
		box-shadow:
			0 0 0 2px color-mix(in oklab, var(--accent) 80%, transparent),
			0 16px 32px -8px color-mix(in oklab, var(--accent) 38%, transparent);
		animation: ep-highlight-pulse 1.6s ease-out 2;
	}
	@keyframes ep-highlight-pulse {
		0% {
			box-shadow:
				0 0 0 0 color-mix(in oklab, var(--accent) 70%, transparent),
				0 0 0 0 color-mix(in oklab, var(--accent) 30%, transparent);
		}
		35% {
			box-shadow:
				0 0 0 4px color-mix(in oklab, var(--accent) 70%, transparent),
				0 0 24px 4px color-mix(in oklab, var(--accent) 35%, transparent);
		}
		100% {
			box-shadow:
				0 0 0 2px color-mix(in oklab, var(--accent) 70%, transparent),
				0 16px 32px -8px color-mix(in oklab, var(--accent) 30%, transparent);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.ep-grid li.ep-highlight .ep-tile {
			animation: none;
		}
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
		padding: var(--space-3) var(--space-4);
		min-block-size: 5rem;
	}
	.ep-title {
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
		font-family: var(--font-display);
		font-size: var(--type-body);
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
	@media (prefers-reduced-motion: reduce) {
		.ep-thumb-skel {
			animation: none;
		}
	}

	/* Caption beneath the grid — informational on the Kitsu-real path,
	   warning when we're rendering numbered fallbacks. */
	.ep-grid-foot {
		margin-block-start: var(--space-4);
		margin-block-end: 0;
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-meta);
		text-transform: uppercase;
		color: var(--bone-400);
	}
	.ep-grid-foot-warn {
		color: color-mix(in oklab, var(--accent-oxblood) 60%, var(--bone-400));
	}

	/* Episode panel controls: jump-to-N input + prev/next pager. Sits
	   between the section header and the grid. Replaces the additive
	   load-more pattern (which made 500-episode shows render a vertical
	   wall on the page). */
	.ep-controls {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: var(--space-4);
		margin-block-end: var(--space-3);
		padding-block-end: var(--space-3);
		border-block-end: 1px solid var(--ink-200);
	}
	@media (max-inline-size: 600px) {
		.ep-controls {
			flex-direction: column;
			align-items: stretch;
		}
	}
	.ep-jump {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}
	.ep-jump-label {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
	}
	.ep-jump-key {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.ep-jump input {
		inline-size: 4.5rem;
		padding: 4px var(--space-2);
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: var(--type-meta);
		color: var(--bone-100);
		background: transparent;
		border: 0;
		border-block-end: 1px solid var(--bone-300);
		outline: 0;
		transition: border-color var(--dur-fast) var(--ease-out-soft);
	}
	.ep-jump input:focus {
		border-block-end-color: var(--bone-100);
	}
	.ep-jump input::-webkit-inner-spin-button,
	.ep-jump input::-webkit-outer-spin-button {
		appearance: none;
	}
	.ep-jump-go {
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		color: var(--bone-300);
		padding: 2px var(--space-3);
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-control);
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft);
	}
	.ep-jump-go:hover:not(:disabled) {
		color: var(--bone-100);
		border-color: var(--bone-300);
	}
	.ep-jump-go:disabled {
		color: var(--bone-400);
		cursor: not-allowed;
	}
	.ep-pager {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
	}
	.ep-pager-btn {
		display: grid;
		place-items: center;
		inline-size: 2rem;
		block-size: 2rem;
		font-family: var(--font-display);
		font-size: var(--type-body-l);
		color: var(--bone-200);
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-pill);
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft);
	}
	.ep-pager-btn:hover:not(:disabled) {
		color: var(--bone-100);
		border-color: var(--bone-300);
	}
	.ep-pager-btn:disabled {
		color: var(--bone-400);
		border-color: var(--ink-200);
		cursor: not-allowed;
	}
	.ep-pager-state {
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: var(--type-meta);
		color: var(--bone-100);
		min-inline-size: 4rem;
		text-align: center;
	}
	.ep-pager-of {
		color: var(--bone-400);
	}

	/* — Similar titles strip below the body. Inherits Strip's gutter
	     (--strip-pad = --space-8) so the row aligns with the page rhythm. */
	.similar {
		margin-block-start: var(--space-8);
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
