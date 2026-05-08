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
	let theaterMode = $state(false);
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

	// Scan every cached page so the meta resolves even when the user
	// has paginated away from the page that contains their current ep.
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
			// `original` last as defense — backend warms signed URLs
			// at Kitsu cache-write time and stores bytes under a
			// canonical hash, so the proxy serves cached bytes for
			// stale signed URLs too. Placeholder still kicks in for
			// shows with no posterImage at all.
			detail?.poster_image?.small ??
				detail?.poster_image?.medium ??
				detail?.poster_image?.large ??
				detail?.poster_image?.original ??
				null
		)
	);

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

		// Open the ep grid at the page containing the current episode
		// so the user lands on their session, not page 1 of a long
		// show. Pagination is local — URL drives session/ep, not page.
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
	<!-- Header strip: show context + prev/next + episode label. -->
	<header class="player-header">
		<a
			class="show-link"
			href={resolve('/anime/[id]', { id })}
			onclick={(e) => {
				// Treat poster → details as an "up" navigation:
				// replace the player history entry rather than push,
				// so back from /anime/[id] returns to whatever opened
				// the player (typically home), not to the player
				// itself. Keep the href so right-click / middle-click
				// still work for power users.
				e.preventDefault();
				void goto(resolve('/anime/[id]', { id }), { replaceState: true });
			}}
		>
			<span class="show-thumb" aria-hidden="true">
				{#if showThumb}
					<img src={showThumb} alt="" loading="lazy" decoding="async" />
				{:else if detail?.canonical_title}
					<!-- Kitsu has no posterImage at any size — render the
					     show's first two letters as a placeholder so the
					     thumb slot reads as branded, not dead-grey. Same
					     pattern PosterCard uses on the home strips. -->
					<span class="show-thumb-placeholder">
						{detail.canonical_title.slice(0, 2).toUpperCase()}
					</span>
				{/if}
			</span>
			<span class="show-meta">
				<span class="eyebrow">
					<span class="eyebrow-key">Now playing</span>
				</span>
				<span class="show-title">{detail?.canonical_title ?? 'Loading…'}</span>
				{#if currentEpisodeMeta?.canonical_title}
					<span class="show-episode">
						<span class="show-episode-num">Episode {episodeNum}</span>
						<span class="show-episode-rule" aria-hidden="true"></span>
						<span class="show-episode-title">{currentEpisodeMeta.canonical_title}</span>
					</span>
				{/if}
			</span>
		</a>

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
	</header>

	{#if externalNotice}
		<p class="external-notice" role="status">{externalNotice}</p>
	{/if}

	<!-- The player. video controls are intentionally native — full
	     keyboard accessibility, no custom shell to maintain. Quality
	     selection lives in hls.js's auto behavior for now; an explicit
	     selector lands when the player chrome polish (M1.8 / follow-ups)
	     does. -->
	<section class="player-frame" class:player-busy={switchBusy}>
		{#if !sessionId}
			<p class="player-empty">No session in URL — return to the show page and pick an episode.</p>
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

	<!-- Episode section: display-face heading, pagination controls,
	     and a wrapping grid of modern thumbnail cards. The thumb
	     carries the ep number + title overlaid under a top fade
	     gradient — same shape regardless of theater state (theater
	     just enlarges the video above). -->
	<section class="ep-section" aria-label="Episodes">
		<header class="ep-section-header">
			<h2 class="ep-section-heading">Episodes</h2>
			<span class="ep-section-counter">
				{#if episodes && episodes.length > 0 && detail?.episode_count}
					{#if totalEpisodePages !== null && totalEpisodePages > 1}
						<!-- Multi-page show: surface the visible range so the
						     user knows where they are in the season. -->
						<span class="ep-section-counter-range">
							<span class="num">{epStart}</span><span aria-hidden="true">–</span><span class="num"
								>{epEnd}</span
							>
						</span>
						<span class="ep-section-counter-of"
							>of <span class="num">{detail.episode_count}</span></span
						>
					{:else}
						<!-- Single-page show: range info is redundant; just say
						     how many episodes there are. -->
						<span class="ep-section-counter-range">
							<span class="num">{detail.episode_count}</span> episodes
						</span>
					{/if}
				{:else if episodes && episodes.length > 0}
					<span class="ep-section-counter-range">page <span class="num">{episodesPage}</span></span>
				{:else if episodesError}
					<span class="ep-section-counter-of">unavailable</span>
				{:else}
					<span class="ep-section-counter-of">loading…</span>
				{/if}
			</span>
		</header>

		{#if totalEpisodePages !== null ? totalEpisodePages > 1 : (episodes?.length ?? 0) >= UI_PAGE_SIZE}
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
								<span class="ep-card-overlay" aria-hidden="true">
									<span class="ep-card-overlay-num">EP {n}</span>
									<span class="ep-card-overlay-title">
										{ep.canonical_title ?? `Episode ${n}`}
									</span>
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
						</button>
					</li>
				{/each}
			</ol>
		{:else if episodesError}
			<p class="ep-list-empty">Couldn't load episodes ({episodesError}).</p>
		{:else}
			<p class="ep-list-empty">Loading episodes…</p>
		{/if}
	</section>

	<!-- "More like this" strip — recommendations seeded from the
	     show's first 1-2 title words via Kitsu search. Wrapped to
	     align with the player + ep section above. -->
	{#if similar && similar.length > 0}
		<div class="similar-wrap">
			<Strip eyebrow="More like this">
				{#each similar as hit (hit.id)}
					<PosterCard anime={hit} />
				{/each}
			</Strip>
		</div>
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
		gap: var(--space-7);
		padding-block: var(--space-6) var(--space-9);
		padding-inline: var(--space-8);
		/* Cap pushed wide (130rem ≈ 2080px) so /play uses most of
		   the screen at a 1920px window instead of leaving black
		   margins. Only at 4K / ultrawide does the cap kick in.
		   Other routes (editorial pages) keep --content-max-wide;
		   /play is a watch surface and feels right when it spreads. */
		max-inline-size: 130rem;
		margin-inline: auto;
		/* Hero glow extends further down (60% block radius vs 35%)
		   with a softer mid-stop, so the warmth bleeds into the
		   page instead of looking like a clipped strip behind the
		   header. The fade ends well before the ep section so the
		   bottom of the page is still neutral. */
		background: radial-gradient(
			ellipse 95% 60% at 50% 0%,
			color-mix(in oklab, var(--accent) 20%, transparent) 0%,
			color-mix(in oklab, var(--accent) 10%, transparent) 25%,
			color-mix(in oklab, var(--accent) 4%, transparent) 50%,
			transparent 75%
		);
	}

	.player-header {
		display: flex;
		align-items: center;
		gap: var(--space-5);
		flex-wrap: wrap;
	}
	.show-link {
		margin-inline-end: auto;
	}

	.show-link {
		display: flex;
		align-items: center;
		gap: var(--space-4);
		color: inherit;
		text-decoration: none;
		min-inline-size: 0;
	}
	.show-link:hover {
		color: var(--bone-100);
	}
	.show-thumb {
		flex: 0 0 auto;
		inline-size: 4.5rem;
		block-size: 6.3rem; /* 5:7 poster aspect */
		border-radius: var(--radius-control);
		overflow: hidden;
		background: color-mix(in oklab, var(--accent) 18%, var(--ink-100));
	}
	.show-thumb img {
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
	}
	.show-thumb-placeholder {
		display: flex;
		align-items: center;
		justify-content: center;
		inline-size: 100%;
		block-size: 100%;
		font-family: var(--font-display);
		font-style: italic;
		font-weight: 600;
		font-size: 1.6rem;
		letter-spacing: 0.01em;
		color: var(--bone-200);
		background: linear-gradient(
			145deg,
			color-mix(in oklab, var(--accent) 28%, var(--ink-100)) 0%,
			color-mix(in oklab, var(--accent) 12%, var(--ink-100)) 100%
		);
	}
	.show-meta {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		min-inline-size: 0;
	}
	.eyebrow {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		color: var(--bone-300);
		text-transform: uppercase;
		letter-spacing: 0.06em;
	}
	.eyebrow-key {
		color: var(--accent);
	}
	.show-title {
		font-family: var(--font-display);
		font-style: italic;
		font-size: var(--type-display-l);
		line-height: 1.05;
		color: var(--bone-100);
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.show-episode {
		display: inline-flex;
		align-items: baseline;
		gap: var(--space-3);
		margin-block-start: var(--space-2);
		font-family: var(--font-mono);
		font-size: var(--type-body);
		color: var(--bone-100);
		min-inline-size: 0;
	}
	.show-episode-num {
		color: var(--accent);
		font-weight: 500;
		text-transform: uppercase;
		letter-spacing: var(--tracking-meta);
	}
	.show-episode-rule {
		flex: 0 0 1.5rem;
		block-size: 1px;
		background: color-mix(in oklab, var(--accent) 50%, var(--ink-300));
		align-self: center;
	}
	.show-episode-title {
		color: var(--bone-100);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
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
		max-inline-size: var(--player-max);
		margin-inline: auto;
		aspect-ratio: 16 / 9;
		background: #000;
		/* Larger rounding + a deeper layered shadow with an
		   accent-tinted halo lifts the player off the page and
		   reads as "premium streaming surface" rather than a flat
		   embedded frame. */
		border-radius: 16px;
		overflow: hidden;
		box-shadow:
			0 30px 80px -20px rgb(0 0 0 / 0.65),
			0 0 60px -12px color-mix(in oklab, var(--accent) 35%, transparent),
			inset 0 0 0 1px color-mix(in oklab, var(--bone-100) 10%, transparent);
	}
	.page.theater .player-frame {
		/* Sizes the 16:9 frame to fit the largest rectangle that
		   simultaneously: (a) doesn't exceed the available width
		   from the rail edge to the viewport's right edge, and
		   (b) doesn't exceed the viewport height minus the chrome.
		   inline-size is the explicit driver — min() picks the
		   smaller of "what (100dvh − chrome) × 16/9 would need"
		   and "what's actually available horizontally". Inherited
		   aspect-ratio: 16/9 then computes block-size from that.
		   Centered over the (viewport − rail) midline via
		   translateX(-50%). */
		inline-size: min(calc((100dvh - 8rem) * 16 / 9), calc(100vw - var(--rail-width)));
		max-inline-size: none;
		block-size: auto;
		max-block-size: none;
		margin-inline: 0;
		position: relative;
		left: 50%;
		transform: translateX(-50%);
		border-radius: 0;
		box-shadow: none;
	}

	.player-frame video {
		inline-size: 100%;
		block-size: 100%;
		display: block;
		background: #000;
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

	/* Similar Titles strip wrapper — caps at --player-max and zeroes
	   Strip's internal --strip-pad (via a :global pierce, since the
	   .strip element redeclares the variable on itself). The strip's
	   content now extends flush with the video edges above instead
	   of sitting inside its own 4.5rem inset. */
	.similar-wrap {
		inline-size: 100%;
		max-inline-size: var(--player-max);
		margin-inline: auto;
	}
	.similar-wrap :global(.strip) {
		--strip-pad: 0;
	}

	/* — Episode section: same width as the player frame above, no
	     extra inner padding so the ep grid spans the full video
	     width (left edge of first card = left edge of video, right
	     edge of last column = right edge of video). — */
	.ep-section {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		min-inline-size: 0;
		inline-size: 100%;
		max-inline-size: var(--player-max);
		margin-inline: auto;
	}
	.ep-section-header {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-3);
		padding-block-end: var(--space-3);
		border-block-end: 1px solid color-mix(in oklab, var(--accent) 25%, var(--ink-300));
	}
	.ep-section-heading {
		margin: 0;
		font-family: var(--font-display);
		font-style: italic;
		font-size: var(--type-display-m);
		font-weight: 500;
		line-height: 1;
		color: var(--bone-100);
		letter-spacing: var(--tracking-display);
	}
	.ep-section-counter {
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
	.ep-section-counter .num {
		color: var(--bone-100);
	}
	.ep-section-counter-range {
		color: var(--bone-200);
	}
	.ep-section-counter-of {
		color: var(--bone-300);
	}

	/* Pagination controls — same widget pair /anime/[id] uses. */
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
	.ep-jump-go,
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
	.ep-jump-go:hover:not(:disabled),
	.ep-pager-btn:hover:not(:disabled) {
		border-color: var(--accent);
		color: var(--bone-100);
	}
	.ep-jump-go:disabled,
	.ep-pager-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}
	.ep-pager {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
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

	/* Wrapping grid — cards flow left-to-right and add rows once
	   they fill the available width. */
	.ep-list {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(18rem, 1fr));
		gap: var(--space-4);
		list-style: none;
		margin: 0;
		padding: 0;
		min-inline-size: 0;
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

	/* Modern episode card — chunky 16:9 thumb with the ep number +
	   title floated on top under a top→bottom fade so they stay
	   readable on bright frames. Hover lifts the card and reveals
	   an accent-tinted play glyph; active card gets an accent ring
	   and a "Now playing" pill. */
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
			0 14px 36px -6px color-mix(in oklab, var(--accent) 55%, transparent);
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

	/* Top-fade overlay carrying ep number + title — stronger gradient
	   and a slight extra ink-stop in the middle so the title stays
	   readable on bright frames (sky / sand / sea backgrounds). */
	.ep-card-overlay {
		position: absolute;
		inset-block-start: 0;
		inset-inline: 0;
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding: var(--space-3) var(--space-4) var(--space-8);
		background: linear-gradient(
			180deg,
			rgb(0 0 0 / 0.92) 0%,
			rgb(0 0 0 / 0.7) 40%,
			rgb(0 0 0 / 0.35) 70%,
			transparent 100%
		);
		pointer-events: none;
	}
	.ep-card-overlay-num {
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		font-weight: 600;
		letter-spacing: var(--tracking-meta);
		text-transform: uppercase;
		color: color-mix(in oklab, var(--accent) 80%, var(--bone-100));
		text-shadow: 0 1px 4px rgb(0 0 0 / 0.8);
	}
	.ep-card-overlay-title {
		font-family: var(--font-display);
		font-style: italic;
		font-size: var(--type-body-l);
		line-height: 1.2;
		color: var(--bone-100);
		text-shadow: 0 1px 4px rgb(0 0 0 / 0.85);
		overflow: hidden;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
	}
	.ep-card-current .ep-card-overlay-num {
		color: var(--accent);
	}

	/* Play glyph — fades in on hover, bottom-right of thumb */
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

	/* "Now playing" pill on the active episode (top-right corner) */
	.ep-card-thumb-flag {
		position: absolute;
		inset-block-start: var(--space-2);
		inset-inline-end: var(--space-2);
		padding: 4px 10px;
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		font-weight: 600;
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--ink-000);
		background: linear-gradient(
			135deg,
			var(--accent),
			color-mix(in oklab, var(--accent) 70%, var(--ink-000))
		);
		border-radius: var(--radius-pill);
		box-shadow:
			0 4px 14px color-mix(in oklab, var(--accent) 50%, transparent),
			0 1px 2px rgb(0 0 0 / 0.4);
	}
</style>
