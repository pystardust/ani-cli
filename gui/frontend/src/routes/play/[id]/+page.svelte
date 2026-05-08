<!--
  Player route — full-bleed page that hosts the active stream session
  for a given Kitsu show. Reuses the show context (poster, title) and
  the episode strip + similar titles from the detail page so the page
  doesn't feel empty around the player.

  URL shape:  /play/<kitsu_id>?session=<sid>&episode=<n>
    - kitsu_id:  drives the show context (poster, title, episodes,
                 similar titles) via the existing Kitsu endpoints.
    - session:   the StreamSession id. The master playlist URL is
                 reconstructed client-side from `apiBase` + the id;
                 hls.js loads that URL through the local proxy.
    - episode:   currently-playing episode number. Highlighted in the
                 strip below the player; consumed by the prev/next
                 affordances in the header.

  Layout:
    1. Header strip: poster thumb + show title (links back to detail) +
       Episode N · {episode title} + previous / next episode buttons.
    2. <video> hero, full body width, ~16:9 frame.
    3. Episode strip with the currently-playing tile accented.
    4. Similar-titles strip (same Kitsu-search-based seed the detail
       page uses).

  Browser refresh: the URL alone carries enough context to rebuild
  the page. The Rust session is in-memory, so a server restart will
  invalidate the session — surfaced via the player's error state when
  hls.js fails to load the master playlist.
-->
<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import Hls from 'hls.js';
	import { settle, settleOut } from '$lib/transitions/settle';
	import {
		altTitlesFromKitsu,
		evictPlayCache,
		imageProxyUrl,
		kitsuAnimeDetail,
		kitsuEpisodes,
		kitsuSearch,
		markWatched,
		playStream,
		playExternal,
		settingsGet,
		type Config,
		type KitsuAnimeRef,
		type KitsuEpisode,
		type MediaKind
	} from '$lib/api';
	import { accentFor } from '$lib/design/accent';
	import { buildMediaUrl } from '$lib/play/media-url';
	import { clearForShow, getOrFire, makeKey } from '$lib/play/play-cache';
	import { breadcrumb } from '$lib/breadcrumb';
	import ErrorOverlay from '$lib/components/ErrorOverlay.svelte';
	import LoadingOverlay from '$lib/components/LoadingOverlay.svelte';
	import PosterCard from '$lib/components/PosterCard.svelte';
	import Strip from '$lib/components/Strip.svelte';

	const id = $derived(page.params.id ?? '');
	const sessionId = $derived(page.url.searchParams.get('session') ?? '');
	const episodeNum = $derived(parseInt(page.url.searchParams.get('episode') ?? '1', 10));
	// kind defaults to hls — the legacy URL shape didn't carry one. The
	// detail page now always appends &kind=<hls|mp4>, so this default
	// only kicks in when a refresh on a stale URL drops the param.
	const mediaKind = $derived<MediaKind>(
		page.url.searchParams.get('kind') === 'mp4' ? 'mp4' : 'hls'
	);
	// True when the play resolution that produced the current session
	// came from the long-term cache. Used to decide whether a player
	// error is silently retryable (cache hit → evict + re-resolve) or
	// terminal (fresh fetch already exhausted the resolve path).
	// Re-set in switchToEpisode whenever a new session lands.
	let cacheHit = $state(page.url.searchParams.get('cache_hit') === '1');
	const accent = $derived(id ? accentFor(id) : 'var(--accent-ink)');

	let detail = $state<KitsuAnimeRef | null>(null);
	let episodes = $state<KitsuEpisode[] | null>(null);
	let episodesPage = $state(1);
	let episodesLoading = $state(false);
	let episodesError = $state<string | null>(null);
	let jumpInput = $state('');
	const UI_PAGE_SIZE = 12;
	const KITSU_PAGE_SIZE = 20;
	const kitsuPageCache = new SvelteMap<number, KitsuEpisode[]>();
	let similar = $state<KitsuAnimeRef[] | null>(null);
	let config = $state<Config | null>(null);
	let detailError = $state<string | null>(null);
	let playerError = $state<string | null>(null);
	let switchBusy = $state(false);
	let switchProgress = $state<string | null>(null);

	function progressLabel(p: import('$lib/api').PlayProgress): string {
		switch (p.kind) {
			case 'banner':
				return p.text;
			case 'links_fetched':
				return `${p.provider} ✓`;
			case 'other':
				return p.text;
		}
	}

	let videoEl: HTMLVideoElement | undefined = $state();
	let hls: Hls | null = null;

	/** Reconstruct the proxy URL from the session id + kind. The proxy
	 *  mounts each session at /s/<id>/master.m3u8 (HLS) or /s/<id>/file.mp4
	 *  (MP4) — the pattern is stable, so we don't round-trip the backend
	 *  just to learn the URL. window.aniGui.apiBase is set by Electron's
	 *  preload script before the renderer mounts. */
	const mediaUrl = $derived.by(() => {
		if (!sessionId) return null;
		const base = (typeof window !== 'undefined' && window.aniGui?.apiBase) || '';
		return base ? buildMediaUrl(base, sessionId, mediaKind) : null;
	});

	const totalEpisodes = $derived(detail?.episode_count ?? null);
	const hasPrev = $derived(episodeNum > 1);
	const hasNext = $derived(totalEpisodes === null || episodeNum < totalEpisodes);

	// Find the metadata for the episode the user is actually
	// watching. May not be in the visible page (the user can browse
	// other pages while ep N plays), so scan all cached pages.
	const currentEpisodeMeta = $derived.by(() => {
		for (const eps of kitsuPageCache.values()) {
			const found = eps.find((e) => (e.number ?? e.relative_number ?? -1) === episodeNum);
			if (found) return found;
		}
		return null;
	});

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
			episodesError = describeError(e);
		} finally {
			episodesLoading = false;
		}
	}

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
		void fetchEpisodesPage(next);
	}

	function jumpToEpisode(event: SubmitEvent) {
		event.preventDefault();
		const n = parseInt(jumpInput, 10);
		if (Number.isNaN(n) || n < 1) return;
		const target = Math.ceil(n / UI_PAGE_SIZE);
		gotoPage(target);
		jumpInput = '';
	}

	const showThumb = $derived(
		imageProxyUrl(
			detail?.poster_image?.small ??
				detail?.poster_image?.medium ??
				detail?.poster_image?.large ??
				detail?.poster_image?.original ??
				null
		)
	);

	// Theater mode hides the episode sidebar and lets the player
	// stretch edge-to-edge of the viewport (still windowed — not
	// browser fullscreen). YouTube has the equivalent toggle next
	// to its fullscreen button. Defaults off; per-session only.
	let theaterMode = $state(false);

	// Synopsis collapsed by default. Same pattern as /anime/[id]:
	// 5-ish-line preview with a soft fade, expands on click.
	let synopsisExpanded = $state(false);

	function teardown() {
		if (hls) {
			hls.destroy();
			hls = null;
		}
		if (videoEl) {
			videoEl.removeAttribute('src');
			videoEl.load();
		}
	}

	$effect(() => {
		if (!videoEl || !mediaUrl) return;
		teardown();
		playerError = null;

		// Native <video> error events fire for HTTP 4xx/5xx and codec
		// failures alike. Wire one listener that covers both the MP4
		// path and the native-HLS fallback so the user sees something
		// when upstream returns 403 / the byte-stream is unreadable.
		const onVideoError = () => {
			const err = videoEl?.error;
			const code = err?.code ?? 0;
			const codeName =
				{
					1: 'aborted',
					2: 'network',
					3: 'decode',
					4: 'not-supported'
				}[code] ?? 'unknown';
			const reason = `${codeName}${err?.message ? ` (${err.message})` : ''}`;
			// network errors on a cache-hit play almost always mean the
			// upstream URL rotated since our HEAD validated. Silent
			// evict + retry rather than dumping a cryptic error on the
			// user. Decode/not-supported errors aren't URL-rotation
			// symptoms, so let them surface.
			if (cacheHit && code === 2) {
				cacheHit = false; // don't infinite-loop if retry also fails
				void silentRetryAfterCacheHitFailure(`video ${reason}`);
				return;
			}
			playerError = `Playback error: ${reason}`;
		};
		videoEl.addEventListener('error', onVideoError);

		// MP4 sessions stream from the local proxy with byte-range
		// support; the <video> element handles seek natively, no need
		// for hls.js. HLS sessions still go through hls.js so that
		// chromium without native HLS works (it doesn't, on Linux).
		if (mediaKind === 'mp4') {
			videoEl.src = mediaUrl;
		} else if (Hls.isSupported()) {
			hls = new Hls({ lowLatencyMode: false });
			hls.loadSource(mediaUrl);
			hls.attachMedia(videoEl);
			hls.on(Hls.Events.ERROR, (_, data) => {
				if (!data.fatal) return;
				if (cacheHit && data.type === 'networkError') {
					cacheHit = false;
					void silentRetryAfterCacheHitFailure(`hls ${data.details}`);
					return;
				}
				playerError = `Playback error: ${data.type} / ${data.details}`;
			});
		} else if (videoEl.canPlayType('application/vnd.apple.mpegurl')) {
			videoEl.src = mediaUrl;
		} else {
			playerError = 'HLS playback is not supported in this webview.';
		}

		return () => {
			videoEl?.removeEventListener('error', onVideoError);
		};
	});

	onDestroy(() => {
		teardown();
		// Cancel any in-flight prefetches for this show. Without this,
		// abandoned ani-cli spawns keep streaming SSE events to a
		// closed page and holding allmanga rate-limit slots. Note this
		// runs on real component unmount only — episode switching keeps
		// the component alive and prefetches remain valid.
		if (id) clearForShow(id);
	});

	function describeError(e: unknown): string {
		if (typeof e === 'object' && e !== null) {
			const obj = e as Record<string, unknown>;
			const kind = typeof obj.kind === 'string' ? obj.kind : null;
			const detail = typeof obj.detail === 'string' ? obj.detail : null;
			if (kind && detail) return `${kind}: ${detail}`;
			if (kind) return kind;
		}
		return String(e);
	}

	/** Human-readable copy for a play-call failure. Mirrors the detail
	 *  page helper of the same name — kept duplicated so the two routes
	 *  can diverge messaging without coupling. */
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

	/** Hard-failure overlay state — distinct from `playerError` (which
	 *  shows in the player area when the video element / hls.js errors
	 *  *during* playback). This one fires when the play *call* itself
	 *  fails (switchToEpisode catch); the overlay must follow the user
	 *  even if they've scrolled to the episode strip. */
	let playFailure = $state<{ episode: number; message: string } | null>(null);

	onMount(() => {
		if (!id) {
			detailError = 'Missing show id in URL.';
			return;
		}
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
			.catch((e) => {
				detailError = describeError(e);
			});

		// Open the ep list at the page that contains the episode the
		// user is watching so they don't land on page 1 of a long
		// show. Pagination state is local — URL drives session/ep,
		// not page.
		const startPage = Math.max(1, Math.ceil(episodeNum / UI_PAGE_SIZE));
		void fetchEpisodesPage(startPage, { initial: true });

		void settingsGet()
			.then((c) => (config = c))
			.catch(() => {
				config = null;
			});
	});

	// Breadcrumb: Home › <title> › EP <n>. Re-runs when the episode
	// number flips (next/prev buttons replaceState the URL) or when
	// the title finally lands. The leaf is plain text — current page
	// gets no href in the trail.
	$effect(() => {
		const title = detail?.canonical_title ?? null;
		breadcrumb.set([
			{ label: 'Home', href: '/' },
			{ label: title ?? 'Anime', href: resolve('/anime/[id]', { id }) },
			{ label: `EP ${episodeNum}` }
		]);
	});

	// Background prefetch: warm every episode visible in the strip
	// concurrently so any click in the page is instant. Re-runs when
	// the strip page changes (different `episodes` array) or settings
	// flip mode/quality. The play-cache dedupes across calls — duplicate
	// keys share a single in-flight promise, so reloading the same page
	// after a swap doesn't refire requests already resolved.
	//
	// Backend will see up to 12 concurrent ani-cli spawns; if allanime
	// or local CPU complains, wire SCRAPER_CONCURRENCY (already on
	// AppState) into run_debug. Today the semaphore is allocated but
	// not acquired — bumping the radius is what surfaces the need.
	$effect(() => {
		if (!detail || !config || !episodes) return;
		const title = detail.canonical_title;
		if (!title) return;
		const mode = (config.mode === 'dub' ? 'dub' : 'sub') as 'sub' | 'dub';
		const quality = config.quality ?? 'best';
		const altTitles = altTitlesFromKitsu(detail);
		for (const ep of episodes) {
			const targetEp = ep.number ?? ep.relative_number ?? null;
			if (targetEp === null) continue;
			void getOrFire(makeKey(id, targetEp, mode, quality), (emit, signal) =>
				playStream(
					{
						title,
						episode: String(targetEp),
						mode,
						quality,
						episode_count: detail?.episode_count ?? null,
						alt_titles: altTitles,
						// Prefetches must NOT update Continue Watching —
						// switchToEpisode (the click path) does that
						// directly via prefetch:false (default).
						prefetch: true,
						kitsu_id: id
					},
					emit,
					signal
				)
			).catch(() => {
				/* click surfaces errors when it fires; abort on unmount
				 *  rejects with "aborted" which we swallow */
			});
		}
	});

	async function switchToEpisode(targetEp: number) {
		if (!detail || switchBusy) return;
		const title = detail.canonical_title;
		if (!title) return;
		const mode = (config?.mode === 'dub' ? 'dub' : 'sub') as 'sub' | 'dub';
		const quality = config?.quality ?? 'best';
		switchBusy = true;
		switchProgress = null;
		playerError = null;
		try {
			// Hits the play-cache: ep+1 was prefetched on mount, so the
			// next-episode click is usually instant. Streaming variant
			// surfaces `<provider> ✓` ticks under the Lottie when the
			// click races a prefetch that hasn't finished yet.
			const session = await getOrFire(
				makeKey(id, targetEp, mode, quality),
				(emit, signal) =>
					playStream(
						{
							title,
							episode: String(targetEp),
							mode,
							quality,
							episode_count: detail?.episode_count ?? null,
							alt_titles: altTitlesFromKitsu(detail),
							kitsu_id: id
						},
						emit,
						signal
					),
				(p) => {
					switchProgress = progressLabel(p);
				}
			);
			// goto navigates within the same route, so the page doesn't
			// fully unmount — `$effect` above re-fires with the new
			// session, and hls.js swaps source. The target is built
			// from `resolve()` plus a query string; the no-resolve
			// lint rule's pattern matcher only recognises a literal
			// `goto(resolve(...))` call, so we suppress around the call.
			cacheHit = session.cache_hit === true;
			// Stamp Continue Watching — see the equivalent block in
			// /anime/[id] for the rationale (getOrFire reuse).
			void markWatched({
				title,
				episode: String(targetEp),
				mode,
				quality,
				episode_count: detail?.episode_count ?? null,
				alt_titles: altTitlesFromKitsu(detail),
				kitsu_id: id
			}).catch(() => {});
			/* eslint-disable svelte/no-navigation-without-resolve */
			// replaceState: true so prev/next don't accumulate history
			// entries — back from /play/[id] always returns to
			// /anime/[id], not to the previously-watched episode.
			// Episode navigation already lives in the player's prev/
			// next controls; the back button is for leaving the show.
			void goto(
				resolve('/play/[id]', { id }) +
					`?session=${encodeURIComponent(session.session_id)}` +
					`&episode=${targetEp}&kind=${session.media_kind}` +
					(cacheHit ? '&cache_hit=1' : ''),
				{ replaceState: true }
			);
			/* eslint-enable svelte/no-navigation-without-resolve */
		} catch (e) {
			// switchToEpisode is the play *call* failing — the user
			// might be scrolled into the episode strip when this fires,
			// so a fixed-position overlay is the only visible surface.
			// playerError stays for *playback*-time errors (in-place
			// `<p class="player-empty">` substitute for the video).
			playFailure = { episode: targetEp, message: describePlayFailure(e) };
		} finally {
			switchBusy = false;
		}
	}

	/** Hand off to a fresh ani-cli resolve when a cached play fails at
	 *  the player layer (4xx mid-stream, hls.js fatal error). Drops
	 *  the cache row server-side AND in memory, then re-runs
	 *  switchToEpisode for the current ep — which cache-misses,
	 *  runs ani-cli, swaps the session URL. LoadingOverlay shows
	 *  naturally during the retry because switchBusy goes high inside
	 *  switchToEpisode. */
	async function silentRetryAfterCacheHitFailure(reason: string) {
		if (!detail || !config) return;
		const title = detail.canonical_title;
		const mode = (config.mode === 'dub' ? 'dub' : 'sub') as 'sub' | 'dub';
		const quality = config.quality ?? 'best';
		console.info('[play] silent retry after cache-hit failure:', reason);
		try {
			await evictPlayCache({
				title,
				episode: String(episodeNum),
				mode,
				quality,
				episode_count: detail.episode_count ?? null,
				alt_titles: altTitlesFromKitsu(detail)
			});
		} catch {
			/* eviction-endpoint failure shouldn't block retry — the
			 *  server may have already evicted on HEAD-fail */
		}
		// Drop in-memory entries so getOrFire fires fresh. clearForShow
		// is broader than needed (drops sibling episodes too) but the
		// sibling prefetches are warming work; losing them costs only
		// the next slow play, which is acceptable for the retry.
		clearForShow(id);
		await switchToEpisode(episodeNum);
	}

	function onPrev() {
		if (!hasPrev) return;
		void switchToEpisode(episodeNum - 1);
	}
	function onNext() {
		if (!hasNext) return;
		void switchToEpisode(episodeNum + 1);
	}
	function onPickEpisode(ep: KitsuEpisode) {
		const n = ep.number ?? ep.relative_number ?? null;
		if (n === null) return;
		if (n === episodeNum) return; // already playing
		void switchToEpisode(n);
	}

	// Hand the currently-playing episode off to the user's mpv (or
	// whichever player they configured). The backend resolves the same
	// upstream URL it would for the embedded path; only the terminal
	// action differs. Errors surface as a short-lived inline notice
	// rather than the LoadingOverlay so the playing video keeps going.
	let externalNotice = $state<string | null>(null);
	let externalBusy = $state(false);
	async function onOpenExternal() {
		const title = detail?.canonical_title;
		if (!title || !config) return;
		const mode = (config.mode === 'dub' ? 'dub' : 'sub') as 'sub' | 'dub';
		const quality = config.quality ?? 'best';
		externalBusy = true;
		externalNotice = `Launching external player for episode ${episodeNum}…`;
		try {
			await playExternal({
				title,
				episode: String(episodeNum),
				mode,
				quality,
				episode_count: detail?.episode_count ?? null,
				alt_titles: altTitlesFromKitsu(detail)
			});
			externalNotice = `Episode ${episodeNum} sent to external player.`;
		} catch (e) {
			externalNotice = `External player failed: ${describeError(e)}`;
		} finally {
			externalBusy = false;
			setTimeout(() => {
				externalNotice = null;
			}, 4000);
		}
	}

	// Keyboard shortcuts: `n` / `p` step episodes. Arrow keys are left
	// to the <video> element for seek control.
	$effect(() => {
		if (typeof window === 'undefined') return;
		const onKey = (e: KeyboardEvent) => {
			const t = e.target as HTMLElement | null;
			const inField =
				t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable);
			if (inField) return;
			if (e.key === 'n' || e.key === 'N') {
				e.preventDefault();
				onNext();
			} else if (e.key === 'p' || e.key === 'P') {
				e.preventDefault();
				onPrev();
			}
		};
		window.addEventListener('keydown', onKey);
		return () => window.removeEventListener('keydown', onKey);
	});
</script>

<svelte:head>
	<title>
		{detail
			? `Ep ${episodeNum}${currentEpisodeMeta?.canonical_title ? ` · ${currentEpisodeMeta.canonical_title}` : ''} · ${detail.canonical_title} · ani-gui`
			: 'Player · ani-gui'}
	</title>
</svelte:head>

<main class="page" class:theater={theaterMode} style:--accent={accent}>
	<!-- Slim controls bar. Show context lives in the breadcrumb
	     and the show-info section below the player; this row is
	     just for episode nav + the external-player escape hatch. -->
	<header class="player-controls">
		<div class="ep-nav" role="group" aria-label="Episode navigation">
			<button
				type="button"
				class="ep-btn"
				onclick={onPrev}
				disabled={!hasPrev || switchBusy}
				aria-label="Previous episode"
			>
				<span aria-hidden="true">‹</span>
				<span>Ep {episodeNum - 1}</span>
			</button>
			<span class="ep-current">
				<span class="ep-num">Ep {episodeNum}</span>
				{#if currentEpisodeMeta?.canonical_title}
					<span class="ep-title">· {currentEpisodeMeta.canonical_title}</span>
				{/if}
			</span>
			<button
				type="button"
				class="ep-btn"
				onclick={onNext}
				disabled={!hasNext || switchBusy}
				aria-label="Next episode"
			>
				<span>Ep {episodeNum + 1}</span>
				<span aria-hidden="true">›</span>
			</button>
		</div>

		<div class="player-actions">
			<button
				type="button"
				class="ep-btn theater-toggle"
				class:theater-on={theaterMode}
				onclick={() => (theaterMode = !theaterMode)}
				aria-pressed={theaterMode}
				aria-label={theaterMode ? 'Exit theater mode' : 'Enter theater mode'}
				title={theaterMode ? 'Exit theater mode' : 'Theater mode'}
			>
				<svg
					viewBox="0 0 24 24"
					width="14"
					height="14"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					aria-hidden="true"
				>
					<rect x="3" y="6" width="18" height="12" rx="1" />
				</svg>
				<span>Theater</span>
			</button>

			<button
				type="button"
				class="ep-btn external"
				onclick={onOpenExternal}
				disabled={switchBusy || externalBusy}
				aria-label="Open this episode in your external player"
				title="Open in external player"
			>
				<span>{externalBusy ? 'Launching…' : 'Open in external'}</span>
				<span aria-hidden="true">↗</span>
			</button>
		</div>
	</header>

	{#if externalNotice}
		<p class="external-notice" role="status">{externalNotice}</p>
	{/if}

	<!-- The player. video controls are intentionally native — full
	     keyboard accessibility, no custom shell to maintain. Quality
	     selection lives in hls.js's auto behavior for now; an explicit
	     selector lands when the player chrome polish (M1.8 / follow-ups)
	     does. -->
	<!-- Player stage: video + show info on the left, episode list
	     pinned to the right. 2-col is the default layout; only
	     viewports under 800px (genuinely narrow / split-screen) fall
	     back to a stack with the ep list spilled into a grid. -->
	<section class="player-stage">
		<div class="player-column">
			<section class="player-frame" class:player-busy={switchBusy}>
				{#if !sessionId}
					<p class="player-empty">
						No session in URL — return to the show page and pick an episode.
					</p>
				{:else if playerError}
					<p class="player-empty">{playerError}</p>
				{:else}
					<video bind:this={videoEl} controls autoplay></video>
				{/if}
				{#if switchBusy}
					<span class="player-spinner" aria-hidden="true">…</span>
				{/if}
			</section>

			{#if detailError}
				<p class="player-empty">{detailError}</p>
			{/if}

			<!-- Show / episode metadata under the video. The banner
			     card on top restores the cover + title from the
			     previous layout (the user explicitly missed it).
			     Synopsis flows full-width below — no narrow column
			     cap, the player column itself is the editorial
			     measure. -->
			{#if detail}
				<section class="show-info">
					<a
						class="show-banner"
						href={resolve('/anime/[id]', { id })}
						onclick={(e) => {
							e.preventDefault();
							void goto(resolve('/anime/[id]', { id }), { replaceState: true });
						}}
					>
						{#if showThumb}
							<img
								class="show-banner-cover"
								src={showThumb}
								alt={`Cover art for ${detail.canonical_title}`}
								loading="lazy"
							/>
						{:else}
							<span class="show-banner-cover show-banner-cover-placeholder" aria-hidden="true"
							></span>
						{/if}
						<span class="show-banner-text">
							<span class="show-banner-eyebrow">Now watching</span>
							<h1 class="show-banner-title">{detail.canonical_title}</h1>
							<span class="show-banner-meta">
								{#if detail.subtype}<span>{detail.subtype.toUpperCase()}</span>{/if}
								{#if detail.start_date}
									<span class="show-banner-meta-sep" aria-hidden="true">·</span>
									<span>{detail.start_date.slice(0, 4)}</span>
								{/if}
								{#if detail.episode_count}
									<span class="show-banner-meta-sep" aria-hidden="true">·</span>
									<span><span class="num">{detail.episode_count}</span> episodes</span>
								{/if}
								{#if detail.average_rating}
									<span class="show-banner-meta-sep" aria-hidden="true">·</span>
									<span class="show-banner-rating">★ {(detail.average_rating / 10).toFixed(1)}</span
									>
								{/if}
							</span>
						</span>
					</a>

					{#if currentEpisodeMeta?.canonical_title}
						<p class="show-info-ep">
							<span class="show-info-ep-key">Episode {episodeNum}</span>
							<span class="show-info-ep-rule" aria-hidden="true"></span>
							<span class="show-info-ep-title">{currentEpisodeMeta.canonical_title}</span>
						</p>
					{/if}

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
					{/if}
				</section>
			{/if}
		</div>

		<aside class="ep-sidebar" aria-label="Episodes">
			<header class="ep-sidebar-header">
				<h2 class="ep-sidebar-heading">Episodes</h2>
				<span class="ep-sidebar-counter">
					{#if episodes && episodes.length > 0 && detail?.episode_count}
						<span class="ep-sidebar-counter-range">
							<span class="num">{epStart}</span><span aria-hidden="true">–</span><span class="num"
								>{epEnd}</span
							>
						</span>
						<span class="ep-sidebar-counter-of"
							>of <span class="num">{detail.episode_count}</span></span
						>
					{:else if episodes && episodes.length > 0}
						<span class="ep-sidebar-counter-range"
							>page <span class="num">{episodesPage}</span></span
						>
					{:else if episodesError}
						<span class="ep-sidebar-counter-of">unavailable</span>
					{:else}
						<span class="ep-sidebar-counter-of">loading…</span>
					{/if}
				</span>
			</header>

			{#if (totalEpisodePages !== null && totalEpisodePages > 1) || (episodes && episodes.length === UI_PAGE_SIZE)}
				<div class="ep-controls">
					<form class="ep-jump" onsubmit={jumpToEpisode}>
						<label class="ep-jump-label">
							<span class="ep-jump-key">Jump</span>
							<input
								type="number"
								min="1"
								max={detail?.episode_count ?? 9999}
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

			{#if episodes && episodes.length > 0}
				<ol class="ep-list">
					{#each episodes as ep, i (ep.id)}
						{@const n = ep.number ?? ep.relative_number ?? 0}
						{@const isCurrent = n === episodeNum}
						{@const epThumb = imageProxyUrl(ep.thumbnail?.original ?? null)}
						<li
							in:settle={{ duration: 620, delay: i * 45 }}
							out:settleOut={{ duration: 320, delay: i * 18 }}
						>
							<button
								type="button"
								class="ep-card"
								class:ep-card-current={isCurrent}
								disabled={switchBusy && !isCurrent}
								onclick={() => onPickEpisode(ep)}
							>
								<span class="ep-card-thumb">
									{#if epThumb}
										<img src={epThumb} alt="" loading="lazy" />
									{:else}
										<span class="ep-card-thumb-placeholder" aria-hidden="true">
											{n.toString().padStart(2, '0')}
										</span>
									{/if}

									<!-- Default sidebar mode: text overlaid on the thumb
									     with a top fade gradient. Hidden in theater mode
									     where the foot below takes over. -->
									<span class="ep-card-overlay" aria-hidden="true">
										<span class="ep-card-overlay-num">EP {n}</span>
										<span class="ep-card-overlay-title">
											{ep.canonical_title ?? `Episode ${n}`}
										</span>
									</span>

									<!-- Theater-mode tag — corner badge same as the
									     detail-page tile. Hidden by default. -->
									<span class="ep-card-tag" aria-hidden="true">
										<span class="ep-card-tag-key">Ep</span>
										<span class="ep-card-tag-num">{n}</span>
									</span>

									<span class="ep-card-thumb-play" aria-hidden="true">
										<svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true">
											<path d="M8 5v14l11-7z" fill="currentColor" />
										</svg>
									</span>
									{#if isCurrent}
										<span class="ep-card-thumb-flag" aria-hidden="true">Now playing</span>
									{/if}
								</span>

								<!-- Theater-mode foot. Hidden by default. -->
								<span class="ep-card-foot">
									<span class="ep-card-foot-title">
										{ep.canonical_title ?? `Episode ${n}`}
									</span>
									{#if ep.length}
										<span class="ep-card-foot-meta">{ep.length}m</span>
									{/if}
								</span>
							</button>
						</li>
					{/each}
				</ol>
			{:else if episodesError}
				<p class="ep-list-empty">Couldn't load episodes ({episodesError}).</p>
			{:else}
				<p class="ep-list-empty">Loading episodes…</p>
			{/if}
		</aside>
	</section>

	<!-- Similar titles — same component the detail page uses; gives
	     the page somewhere to land the eye when an episode wraps. -->
	{#if similar && similar.length > 0}
		<Strip eyebrow="Similar titles" caption="via Kitsu search">
			{#each similar as hit (hit.id)}
				<PosterCard anime={hit} />
			{/each}
		</Strip>
	{/if}
</main>

<LoadingOverlay visible={switchBusy} progress={switchProgress} />

{#if playFailure}
	<ErrorOverlay
		headline={`Couldn't play episode ${playFailure.episode}`}
		body={playFailure.message}
		onDismiss={() => (playFailure = null)}
	/>
{/if}

<style>
	.page {
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
		padding-block: var(--space-5) var(--space-9);
		/* Inline padding bumped from space-5 to space-7 so the
		   layout breathes against the rail on the left and against
		   the window edge on the right. The video sits in a
		   correspondingly narrower column — feels less "spread to
		   fill" and more anchored. */
		padding-inline: var(--space-7);
		/* No max-inline-size cap and no margin-inline: auto — /play
		   uses the full viewport width so the ep list sits flush to
		   the right edge of the page area instead of being centered
		   inside a 110rem container. */
	}

	/* Theater mode (YouTube-style): the video grows to take the full
	   stage width that the sidebar used to occupy, AND the ep list
	   moves below the video instead of disappearing. The user
	   explicitly wanted the list to stay visible — losing it on
	   toggle felt like a punishment for a UX choice that was
	   supposed to enlarge, not strip. */
	.page.theater .player-stage {
		grid-template-columns: 1fr;
	}
	.page.theater .ep-sidebar {
		/* Override the sticky right-column rules so the list flows
		   naturally below the player. */
		position: static;
		max-block-size: none;
	}
	/* Theater ep-list overrides live further down in the file, in
	   the section that styles .ep-card to match the /anime/[id]
	   tile shape. Keeping the layout-side rules grouped with their
	   visual side-effects so changes to one don't drift from the
	   other. */
	.page.theater .player-frame {
		/* Cap by viewport height so the video doesn't push the ep
		   grid below the fold on shallow windows. aspect-ratio
		   continues to drive the 16:9 shape; max-block-size kicks
		   in only when the natural inline-size would compute a
		   block-size taller than (100dvh − chrome). */
		max-block-size: calc(100dvh - 10rem);
		margin-inline: auto;
	}

	.player-controls {
		display: flex;
		align-items: center;
		gap: var(--space-5);
		flex-wrap: wrap;
	}
	.player-actions {
		display: inline-flex;
		align-items: center;
		gap: var(--space-3);
		margin-inline-start: auto;
	}
	.theater-toggle {
		gap: var(--space-2);
		color: var(--bone-100);
	}
	.theater-toggle svg {
		display: block;
	}
	.theater-toggle.theater-on {
		border-color: var(--accent);
		background: color-mix(in oklab, var(--accent) 18%, var(--ink-050));
		color: var(--bone-100);
	}
	.theater-toggle.theater-on svg {
		stroke: var(--accent);
	}

	/* Two-col stage IS the /play layout — video + show info on the
	   left, episode list flush to the right. Earlier builds gated
	   this behind a 1100/1280 viewport breakpoint and the user's
	   feedback was that the page kept feeling stacked/centered at
	   their working window size. The layout is now the default;
	   only viewports below 800px (genuinely narrow — handheld /
	   half-screen-split) fall back to a stack. Sidebar uses a
	   clamp range with a hard ceiling so it doesn't grow on
	   ultrawide. */
	.player-stage {
		display: grid;
		/* Sidebar bumped wider (was clamp 16-22rem) so the ep list
		   can run as a 2-col grid of chunky thumbnails without
		   cards getting squeezed below readable width. */
		grid-template-columns: minmax(0, 1fr) clamp(24rem, 28vw, 30rem);
		gap: var(--space-5);
		align-items: start;
	}
	.player-column {
		display: flex;
		flex-direction: column;
		gap: var(--space-5);
		min-inline-size: 0;
	}
	@media (max-inline-size: 800px) {
		.player-stage {
			display: flex;
			flex-direction: column;
			gap: var(--space-5);
		}
	}
	/* Theater mode collapses the grid back to a single column even
	   when the viewport is wide enough for two — the sidebar is
	   hidden, so a 2-col grid would leave a phantom empty track. */
	.page.theater .player-stage {
		display: flex;
		flex-direction: column;
		gap: var(--space-5);
	}

	/* Episode sidebar — vertical list of episode cards. Sticks to
	   the top of the viewport while scrolling and caps its block
	   size to keep the surface from running off the page on long
	   episode counts. */
	.ep-sidebar {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		min-inline-size: 0;
	}
	@media (min-inline-size: 801px) {
		.ep-sidebar {
			position: sticky;
			inset-block-start: var(--space-5);
			max-block-size: calc(100dvh - 6rem);
		}
	}
	/* Sidebar header — display-face heading + refined mono
	   counter on a single baseline. Replaces the previous mono
	   uppercase eyebrow + thin rule, which read flat. The accent
	   underline gives "Episodes" a subtle anchor without becoming
	   a heavy divider. */
	.ep-sidebar-header {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-3);
		padding-block-end: var(--space-3);
		border-block-end: 1px solid color-mix(in oklab, var(--accent) 25%, var(--ink-300));
	}
	.ep-sidebar-heading {
		margin: 0;
		font-family: var(--font-display);
		font-style: italic;
		font-size: var(--type-display-m);
		font-weight: 500;
		line-height: 1;
		color: var(--bone-100);
		letter-spacing: var(--tracking-display);
	}
	.ep-sidebar-counter {
		display: inline-flex;
		align-items: baseline;
		gap: var(--space-2);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-meta);
		text-transform: uppercase;
		color: var(--bone-300);
		font-variant-numeric: tabular-nums lining-nums;
	}
	.ep-sidebar-counter .num {
		color: var(--bone-100);
	}
	.ep-sidebar-counter-range {
		color: var(--bone-200);
	}
	.ep-sidebar-counter-of {
		color: var(--bone-300);
	}
	/* Sidebar ep list — 2-col grid of chunky thumbnail cards with
	   real margin between items. Below 800px the list spreads as
	   an auto-fill responsive grid since the page is stacked. */
	.ep-list {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: var(--space-4) var(--space-3);
		list-style: none;
		margin: 0;
		padding: 0;
		min-inline-size: 0;
	}
	@media (min-inline-size: 801px) {
		.ep-list {
			overflow-y: auto;
			padding-block-end: var(--space-2);
			padding-inline-end: var(--space-2);
		}
	}
	@media (max-inline-size: 800px) {
		.ep-list {
			grid-template-columns: repeat(auto-fill, minmax(11rem, 1fr));
		}
	}
	.ep-list li {
		display: block;
		min-inline-size: 0;
	}
	.ep-list-empty {
		margin: 0;
		padding: var(--space-4);
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		color: var(--bone-300);
		text-align: center;
		border: 1px dashed var(--ink-300);
		border-radius: var(--radius-card);
	}

	/* Show + episode metadata under the video. No max-inline-size
	   cap — the synopsis flows to the player column's natural
	   width, which IS the editorial measure (the wider grid
	   already constrains the column to a comfortable reading
	   width via the 2-col split). The previous 60ch cap created
	   a narrow column glued to the left edge that read as awkward. */
	.show-info {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}
	.show-banner {
		display: grid;
		grid-template-columns: 5rem 1fr;
		gap: var(--space-5);
		align-items: center;
		padding: var(--space-3);
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-card);
		background: color-mix(in oklab, var(--ink-050) 70%, transparent);
		text-decoration: none;
		color: inherit;
		transition:
			border-color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft);
	}
	.show-banner:hover {
		border-color: var(--accent);
		background: color-mix(in oklab, var(--accent) 8%, var(--ink-050));
	}
	.show-banner-cover {
		display: block;
		inline-size: 100%;
		aspect-ratio: 5 / 7;
		object-fit: cover;
		border-radius: var(--radius-control);
		background: var(--ink-100);
	}
	.show-banner-cover-placeholder {
		background: linear-gradient(135deg, var(--ink-100), var(--ink-200));
	}
	.show-banner-text {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		min-inline-size: 0;
	}
	.show-banner-eyebrow {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--accent);
	}
	.show-banner-title {
		margin: 0;
		font-family: var(--font-display);
		font-style: italic;
		font-size: var(--type-display-l);
		line-height: 1.05;
		color: var(--bone-100);
	}
	.show-banner:hover .show-banner-title {
		color: var(--accent);
	}
	.show-banner-meta {
		display: inline-flex;
		align-items: baseline;
		gap: var(--space-2);
		flex-wrap: wrap;
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		color: var(--bone-300);
		text-transform: uppercase;
		letter-spacing: var(--tracking-meta);
	}
	.show-banner-meta .num {
		color: var(--bone-100);
		font-variant-numeric: tabular-nums lining-nums;
	}
	.show-banner-meta-sep {
		color: var(--bone-400);
	}
	.show-banner-rating {
		color: var(--accent);
	}
	.show-info-ep {
		display: inline-flex;
		align-items: baseline;
		gap: var(--space-3);
		margin: 0;
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		color: var(--bone-200);
		min-inline-size: 0;
	}
	.show-info-ep-key {
		color: var(--accent);
		text-transform: uppercase;
		letter-spacing: var(--tracking-meta);
	}
	.show-info-ep-rule {
		flex: 0 0 1.5rem;
		block-size: 1px;
		background: var(--ink-300);
		align-self: center;
	}
	.show-info-ep-title {
		color: var(--bone-100);
		min-inline-size: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	/* Synopsis collapse/expand. Same pattern + visual rhythm as
	   /anime/[id]: 5-ish-line preview, soft fade at the bottom,
	   editorial display face with a drop cap when expanded. The
	   font ladder lifts to display-m so the prose below the video
	   feels like a proper editorial column instead of a caption. */
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
		align-self: flex-start;
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

	.ep-nav {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		flex-wrap: wrap;
	}
	.ep-btn {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-4);
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-pill);
		background: color-mix(in oklab, var(--ink-050) 70%, transparent);
		color: var(--bone-200);
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		cursor: pointer;
		transition:
			border-color var(--dur-fast) var(--ease-out-soft),
			color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft);
	}
	.ep-btn:hover:not(:disabled) {
		border-color: var(--accent);
		color: var(--bone-100);
	}
	.ep-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}
	.external-notice {
		margin: var(--space-3) 0 0;
		padding: var(--space-2) var(--space-3);
		font-size: var(--type-meta);
		color: var(--bone-100);
		background: rgba(0, 0, 0, 0.4);
		border-radius: var(--radius-control);
	}
	.ep-current {
		display: inline-flex;
		align-items: baseline;
		gap: var(--space-2);
		padding-inline: var(--space-3);
		min-inline-size: 0;
	}
	.ep-num {
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		color: var(--accent);
	}
	.ep-title {
		font-size: var(--type-meta);
		color: var(--bone-200);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		max-inline-size: 32ch;
	}

	.player-frame {
		position: relative;
		inline-size: 100%;
		aspect-ratio: 16 / 9;
		/* Height cap so the video never grows so tall it pushes the
		   show-info / synopsis below the fold, even when the player
		   column is very wide. The aspect-ratio still drives shape;
		   when max-block-size kicks in, inline-size is reduced to
		   match (and the frame centers via margin-inline auto). */
		max-block-size: calc(100dvh - 14rem);
		margin-inline: auto;
		background: #000;
		border-radius: var(--radius-card);
		overflow: hidden;
		box-shadow: 0 4px 24px color-mix(in oklab, var(--accent) 18%, transparent);
	}
	.player-frame video {
		inline-size: 100%;
		block-size: 100%;
		display: block;
		background: #000;
		/* object-fit: contain mirrors YouTube/native HTML5 video
		   behavior — videos that aren't 16:9 (the frame's
		   aspect-ratio) get letterboxed inside the frame instead
		   of being cropped or stretched. */
		object-fit: contain;
	}
	.player-empty {
		position: absolute;
		inset: 0;
		display: grid;
		place-items: center;
		text-align: center;
		padding: var(--space-6);
		color: var(--bone-100);
		font-family: var(--font-body);
		font-size: var(--type-body-l);
		font-weight: 500;
		line-height: 1.5;
	}
	.player-busy video {
		opacity: 0.5;
		transition: opacity var(--dur-med) var(--ease-out-soft);
	}
	.player-spinner {
		position: absolute;
		inset: 0;
		display: grid;
		place-items: center;
		color: var(--accent);
		font-family: var(--font-mono);
		font-size: var(--type-display-l);
		pointer-events: none;
	}

	/* Episode card — two visual modes:
	     • Default sidebar: chunky 16:9 thumb fills the card; ep
	       number + title float on top of the thumb under a top→
	       bottom dark fade so they stay readable on bright frames.
	     • Theater mode: thumb shrinks to detail-page-tile shape
	       (16:9 image, corner badge), title + duration drop below
	       in a dedicated foot. Same component as /anime/[id].
	   The mode switch is CSS-only — markup carries both layers,
	   visibility flips on .page.theater. */
	.ep-card {
		position: relative;
		display: block;
		padding: 0;
		inline-size: 100%;
		border: 0;
		border-radius: var(--radius-card);
		background: transparent;
		color: inherit;
		text-align: start;
		cursor: pointer;
		isolation: isolate;
		transition: transform var(--dur-fast) var(--ease-out-soft);
	}
	.ep-card:hover:not(:disabled) {
		transform: translateY(-2px);
	}
	.ep-card:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	/* Thumb — chunkier rounding (12px instead of card 8px) to
	   give the card more visual weight. */
	.ep-card-thumb {
		position: relative;
		display: block;
		aspect-ratio: 16 / 9;
		background: linear-gradient(
			135deg,
			var(--ink-100),
			color-mix(in oklab, var(--accent) 14%, var(--ink-100))
		);
		border-radius: 12px;
		overflow: hidden;
		box-shadow:
			0 1px 2px rgb(0 0 0 / 0.45),
			inset 0 0 0 1px color-mix(in oklab, var(--ink-300) 80%, transparent);
		transition:
			box-shadow var(--dur-med) var(--ease-out-soft),
			transform var(--dur-fast) var(--ease-out-soft);
	}
	.ep-card:hover:not(:disabled) .ep-card-thumb {
		box-shadow:
			0 12px 28px -6px color-mix(in oklab, var(--accent) 28%, transparent),
			0 4px 10px -4px rgb(0 0 0 / 0.45),
			inset 0 0 0 1px color-mix(in oklab, var(--accent) 80%, var(--bone-300));
	}
	.ep-card-current .ep-card-thumb {
		box-shadow:
			0 0 0 2px var(--accent),
			0 8px 24px -4px color-mix(in oklab, var(--accent) 45%, transparent);
	}
	.ep-card-thumb img {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
		filter: brightness(0.92);
		transition:
			transform var(--dur-med) var(--ease-out-soft),
			filter var(--dur-med) var(--ease-out-soft);
	}
	.ep-card:hover:not(:disabled) .ep-card-thumb img {
		transform: scale(1.04);
		filter: brightness(1);
	}
	.ep-card-thumb-placeholder {
		position: absolute;
		inset: 0;
		display: grid;
		place-items: center;
		font-family: var(--font-display);
		font-style: italic;
		font-size: var(--type-display-l);
		font-variant-numeric: tabular-nums lining-nums;
		color: var(--bone-300);
	}

	/* — DEFAULT MODE: text overlay on thumb with top fade — */
	.ep-card-overlay {
		position: absolute;
		inset-block-start: 0;
		inset-inline: 0;
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: var(--space-3) var(--space-3) var(--space-7);
		background: linear-gradient(
			180deg,
			rgb(0 0 0 / 0.78) 0%,
			rgb(0 0 0 / 0.45) 60%,
			transparent 100%
		);
		pointer-events: none;
	}
	.ep-card-overlay-num {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: color-mix(in oklab, var(--accent) 70%, var(--bone-100));
		text-shadow: 0 1px 3px rgb(0 0 0 / 0.7);
	}
	.ep-card-overlay-title {
		font-family: var(--font-display);
		font-style: italic;
		font-size: var(--type-body);
		line-height: 1.2;
		color: var(--bone-100);
		text-shadow: 0 1px 3px rgb(0 0 0 / 0.7);
		overflow: hidden;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
	}
	.ep-card-current .ep-card-overlay-num {
		color: var(--accent);
	}

	/* — THEATER MODE: corner tag (default-hidden) and foot (also
	     hidden by default) — */
	.ep-card-tag {
		display: none; /* shown only in theater mode */
		position: absolute;
		inset-block-start: var(--space-2);
		inset-inline-start: var(--space-2);
		align-items: baseline;
		gap: var(--space-1);
		padding: 2px var(--space-2);
		background: color-mix(in oklab, var(--ink-000) 78%, transparent);
		backdrop-filter: blur(4px);
		border: 1px solid color-mix(in oklab, var(--accent) 40%, var(--bone-400));
		border-radius: var(--radius-control);
	}
	.ep-card-tag-key {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.ep-card-tag-num {
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: var(--type-meta);
		color: var(--bone-100);
	}
	.ep-card-foot {
		display: none; /* shown only in theater mode */
		flex-direction: column;
		gap: var(--space-1);
		padding: var(--space-3) var(--space-4);
		min-block-size: 5rem;
		background: var(--ink-050);
		border: 1px solid var(--ink-200);
		border-block-start: 0;
		border-end-end-radius: var(--radius-card);
		border-end-start-radius: var(--radius-card);
		margin-block-start: -12px; /* tuck under the chunky thumb radius */
		padding-block-start: calc(var(--space-3) + 12px);
	}
	.ep-card-foot-title {
		font-family: var(--font-display);
		font-size: var(--type-body);
		line-height: var(--leading-tight);
		color: var(--bone-100);
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}
	.ep-card-foot-meta {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-meta);
		color: var(--bone-400);
	}

	/* Theater mode = literal /anime/[id] ep-tile shape: a single
	   bordered card wrapping a 16:9 thumb on top and a foot below,
	   with brightness-filter on the image, lift + scale on hover,
	   accent-tinted shadow halo, and a transform-origin that pulls
	   the lift upward instead of pushing the row below. Mirrors the
	   detail page byte-for-byte so the two routes feel like one
	   surface. */
	.page.theater .ep-list {
		/* Wrapping grid — cards flow left-to-right and wrap into
		   additional rows once they fill the row width. Same
		   shape /anime/[id]'s ep-grid uses, no horizontal scroll
		   bar (the user wanted rows, not a strip). */
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(18rem, 1fr));
		gap: var(--space-4);
		overflow: visible;
		max-block-size: none;
		padding-inline-end: 0;
		padding-block-end: 0;
	}
	.page.theater .ep-card {
		display: grid;
		grid-template-rows: auto 1fr;
		gap: 0;
		background: var(--ink-050);
		border: 1px solid var(--ink-200);
		border-radius: var(--radius-card);
		overflow: hidden;
		transform-origin: 50% 80%;
		transition:
			transform var(--dur-med) var(--ease-out-elastic),
			border-color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft),
			box-shadow var(--dur-med) var(--ease-out-soft);
	}
	.page.theater .ep-card:hover:not(:disabled) {
		transform: translateY(-4px) scale(1.04);
		z-index: 1;
		border-color: color-mix(in oklab, var(--accent) 80%, var(--ink-300));
		box-shadow:
			0 12px 28px -6px color-mix(in oklab, var(--accent) 28%, transparent),
			0 4px 10px -4px rgb(0 0 0 / 0.45);
	}
	.page.theater .ep-card-thumb {
		border-radius: 0;
		box-shadow: none;
	}
	.page.theater .ep-card-thumb img {
		filter: brightness(0.85);
	}
	.page.theater .ep-card:hover:not(:disabled) .ep-card-thumb img {
		transform: none;
		filter: brightness(1);
	}
	.page.theater .ep-card-current {
		border-color: var(--accent);
	}
	.page.theater .ep-card-current .ep-card-thumb {
		box-shadow: none;
	}

	/* Mode switch — show theater-only elements, hide sidebar-only. */
	.page.theater .ep-card-overlay {
		display: none;
	}
	.page.theater .ep-card-tag {
		display: inline-flex;
	}
	.page.theater .ep-card-foot {
		display: flex;
		background: transparent;
		border: 0;
		margin-block-start: 0;
		padding-block-start: var(--space-3);
	}
	.page.theater .ep-card-thumb-play {
		display: none;
	}

	/* Play glyph — fades in on hover. Sits in the bottom-right so
	   it doesn't fight the title overlay (top) or the corner tag
	   (top-left, theater mode). */
	.ep-card-thumb-play {
		position: absolute;
		inset-block-end: var(--space-2);
		inset-inline-end: var(--space-2);
		display: inline-flex;
		align-items: center;
		justify-content: center;
		inline-size: 2.4rem;
		block-size: 2.4rem;
		border-radius: var(--radius-pill);
		background: color-mix(in oklab, var(--accent) 80%, var(--ink-000));
		color: var(--bone-100);
		opacity: 0;
		transform: scale(0.78);
		transition:
			opacity var(--dur-fast) var(--ease-out-soft),
			transform var(--dur-fast) var(--ease-out-elastic);
		box-shadow: 0 4px 12px -2px color-mix(in oklab, var(--accent) 50%, transparent);
	}
	.ep-card:hover:not(:disabled) .ep-card-thumb-play {
		opacity: 1;
		transform: scale(1);
	}

	/* "Now playing" pill on the active episode (top-right) */
	.ep-card-thumb-flag {
		position: absolute;
		inset-block-start: var(--space-2);
		inset-inline-end: var(--space-2);
		padding: 3px 10px;
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--ink-000);
		background: var(--accent);
		border-radius: var(--radius-pill);
		box-shadow: 0 2px 6px rgb(0 0 0 / 0.4);
	}
	.page.theater .ep-card-thumb-flag {
		/* Theater mode has the corner tag in top-left; move flag
		   to bottom so they don't collide. */
		inset-block-start: auto;
		inset-block-end: var(--space-2);
		inset-inline-end: var(--space-2);
	}

	/* Pagination controls — same widget pair as /anime/[id]:
	   jump-to-episode form on one side, prev/state/next on the
	   other. Compact enough to sit at the top of the sidebar
	   without crowding the ep cards underneath. */
	.ep-controls {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: var(--space-3);
		flex-wrap: wrap;
	}
	.ep-jump {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
	}
	.ep-jump-label {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		padding: 4px var(--space-3);
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-pill);
		background: color-mix(in oklab, var(--ink-050) 70%, transparent);
	}
	.ep-jump-key {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.ep-jump-label input {
		inline-size: 4rem;
		padding: 0;
		border: 0;
		background: transparent;
		color: var(--bone-100);
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		font-variant-numeric: tabular-nums lining-nums;
	}
	.ep-jump-label input::placeholder {
		color: var(--bone-400);
	}
	.ep-jump-label input:focus-visible {
		outline: none;
	}
	.ep-jump-go {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		inline-size: 2rem;
		block-size: 2rem;
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-pill);
		background: color-mix(in oklab, var(--ink-050) 70%, transparent);
		color: var(--bone-200);
		font-family: var(--font-mono);
		cursor: pointer;
		transition:
			border-color var(--dur-fast) var(--ease-out-soft),
			color var(--dur-fast) var(--ease-out-soft);
	}
	.ep-jump-go:hover:not(:disabled) {
		border-color: var(--accent);
		color: var(--bone-100);
	}
	.ep-jump-go:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}
	.ep-pager {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
	}
	.ep-pager-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		inline-size: 2rem;
		block-size: 2rem;
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-pill);
		background: color-mix(in oklab, var(--ink-050) 70%, transparent);
		color: var(--bone-200);
		font-family: var(--font-mono);
		cursor: pointer;
		transition:
			border-color var(--dur-fast) var(--ease-out-soft),
			color var(--dur-fast) var(--ease-out-soft);
	}
	.ep-pager-btn:hover:not(:disabled) {
		border-color: var(--accent);
		color: var(--bone-100);
	}
	.ep-pager-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}
	.ep-pager-state {
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		font-variant-numeric: tabular-nums lining-nums;
		color: var(--bone-100);
		min-inline-size: 3.5rem;
		text-align: center;
	}
	.ep-pager-of {
		color: var(--bone-300);
	}
</style>
