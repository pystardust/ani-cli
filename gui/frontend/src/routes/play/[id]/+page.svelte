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
	import { settle } from '$lib/transitions/settle';
	import {
		altTitlesFromKitsu,
		downloadDefaultDir as downloadDefaultDirApi,
		evictPlayCache,
		imageProxyUrl,
		kitsuAnimeDetail,
		kitsuEpisodes,
		kitsuSearch,
		markWatched,
		playStream,
		playExternal,
		settingsGet,
		settingsPut,
		type Config,
		type DownloadArgs,
		type KitsuAnimeRef,
		type KitsuEpisode,
		type MediaKind
	} from '$lib/api';
	import { accentFor } from '$lib/design/accent';
	import { buildMediaUrl } from '$lib/play/media-url';
	import { buildPlayQuery } from '$lib/play/play-url';
	import { decideAutoPlayNext } from '$lib/play/auto-play-next';
	import { clearForShow, getOrFire, makeKey } from '$lib/play/play-cache';
	import { breadcrumb } from '$lib/breadcrumb';
	import DownloadConfirm from '$lib/components/DownloadConfirm.svelte';
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
	// 5 cards per UI page = a single horizontal row under the player.
	// On viewports too narrow to fit all 5, the headless Strip does
	// the same drag-scroll / edge-fade / arrow-on-hover dance the home
	// rows use. Static constant so resizing doesn't reshuffle items
	// between pages.
	const UI_PAGE_SIZE = 5;
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

	// Feature flag — toggles between Chromium's native <video>
	// controls and our custom controls bar. The custom bar lets
	// the timeline take the per-show accent color (Chromium's
	// shadow-DOM slider can't be themed); the native controls
	// are more battle-tested for edge cases (PIP, captions menu,
	// keyboard shortcuts). Off by default until the custom bar
	// proves out under real usage. Flip to true to preview.
	const USE_CUSTOM_PLAYER_CONTROLS = false;

	// Custom controls — Chromium's native media controls don't
	// honor accent-color on the timeline shadow DOM, so we render
	// our own progress bar / play / volume / fullscreen UI on top
	// of the video. Bindings keep the bar in sync with the video
	// element's actual state without polling.
	let isPaused = $state(true);
	let currentTime = $state(0);
	let duration = $state(0);
	let videoVolume = $state(1);
	let isMuted = $state(false);
	let scrubberHover = $state(false);

	function formatTime(seconds: number): string {
		if (!Number.isFinite(seconds) || seconds < 0) return '0:00';
		const total = Math.floor(seconds);
		const h = Math.floor(total / 3600);
		const m = Math.floor((total % 3600) / 60);
		const s = total % 60;
		const mm = h > 0 ? String(m).padStart(2, '0') : String(m);
		const ss = String(s).padStart(2, '0');
		return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
	}

	function togglePlay() {
		if (!videoEl) return;
		if (videoEl.paused) void videoEl.play();
		else videoEl.pause();
	}

	function toggleMute() {
		if (!videoEl) return;
		videoEl.muted = !videoEl.muted;
	}

	function seekToFraction(fraction: number) {
		if (!videoEl || !duration) return;
		videoEl.currentTime = Math.max(0, Math.min(duration, fraction * duration));
	}

	function onScrubberClick(event: MouseEvent) {
		const target = event.currentTarget as HTMLElement;
		const rect = target.getBoundingClientRect();
		const fraction = (event.clientX - rect.left) / rect.width;
		seekToFraction(fraction);
	}

	function toggleFullscreen() {
		if (!videoEl) return;
		const frame = videoEl.closest('.player-frame');
		if (!document.fullscreenElement) {
			void (frame ?? videoEl).requestFullscreen?.();
		} else {
			void document.exitFullscreen?.();
		}
	}

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

	/** Subtitle plumbing (F1.11). The play navigation appends `?sub=1`
	 *  when the backend resolution returned a subtitle URL — see
	 *  buildPlayQuery. The proxy mounts the .vtt at /s/<session>/sub.vtt
	 *  with the upstream Referer injected, so the renderer never
	 *  fetches the upstream URL directly. Most allmanga sources embed
	 *  subs in the HLS manifest as a TextTrack and don't ship a
	 *  separate file; ?sub=1 is absent for those, and the <track>
	 *  isn't rendered. */
	const hasSubtitles = $derived(page.url.searchParams.get('sub') === '1');
	const subtitleUrl = $derived.by(() => {
		if (!sessionId || !hasSubtitles) return null;
		const base = (typeof window !== 'undefined' && window.aniGui?.apiBase) || '';
		return base ? `${base.replace(/\/+$/, '')}/s/${sessionId}/sub.vtt` : null;
	});

	const totalEpisodes = $derived(detail?.episode_count ?? null);
	const hasPrev = $derived(episodeNum > 1);
	const hasNext = $derived(totalEpisodes === null || episodeNum < totalEpisodes);

	// Auto-play next: persisted in Config.auto_play_next; mirrored as a
	// local boolean so the inline toggle on the player can flip the
	// state instantly while the persist call goes out async.
	let autoPlayNext = $state(false);
	$effect(() => {
		if (config) autoPlayNext = config.auto_play_next;
	});

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

	// Episode tile to scroll-to + briefly highlight after a Jump-to-ep
	// submit. Mirrors the detail page's spotlight effect: targeted card
	// pulses an accent ring; sibling cards dim to 0.35 then fade back as
	// the highlight ends. Cleared on a 3.2s timeout once on screen.
	let highlightEp = $state<number | null>(null);

	function jumpToEpisode(event: SubmitEvent) {
		event.preventDefault();
		const n = parseInt(jumpInput, 10);
		if (Number.isNaN(n) || n < 1) return;
		const target = Math.ceil(n / UI_PAGE_SIZE);
		// Set highlight first so the effect that watches it sees the
		// target. If gotoPage triggers a fetch (different page), the
		// effect re-runs once `episodes` updates; if same page, the
		// effect runs on the next microtask.
		highlightEp = n;
		gotoPage(target);
		jumpInput = '';
	}

	// Scroll-to + accent-ring pulse for the target episode tile. Watches
	// `highlightEp` AND `episodes` so the effect re-runs after a page
	// fetch completes — that's when the matching `<li data-ep-num=…>`
	// tile is actually in the DOM. The 3.2s clear timer only starts on
	// the run that successfully finds the tile (off-page targets wait
	// silently for the next run).
	$effect(() => {
		const target = highlightEp;
		if (target === null) return;
		// Track `episodes` reactively so we re-run after the right page
		// loads. Without this, jumping to an episode on a different
		// page would scroll once before the data lands.
		// eslint-disable-next-line @typescript-eslint/no-unused-expressions
		episodes;

		let timerId: number | undefined;
		const rafId = requestAnimationFrame(() => {
			const el = document.querySelector(`[data-ep-num="${target}"]`);
			if (!el) return;
			el.scrollIntoView({ behavior: 'smooth', block: 'center' });
			timerId = window.setTimeout(() => {
				if (highlightEp === target) highlightEp = null;
			}, 3200);
		});

		return () => {
			cancelAnimationFrame(rafId);
			if (timerId !== undefined) clearTimeout(timerId);
		};
	});

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
			void goto(resolve('/play/[id]', { id }) + buildPlayQuery(session, targetEp), {
				replaceState: true
			});
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

	/** Inline toggle handler for the "Auto-play on/off" button next to
	 *  the episode-nav cluster. Updates local state immediately, then
	 *  persists to settings so the choice carries across sessions and
	 *  is reflected on the Settings page. */
	function toggleAutoPlayNext() {
		if (!config) return;
		const next = !autoPlayNext;
		autoPlayNext = next;
		const merged: Config = { ...config, auto_play_next: next };
		config = merged;
		void settingsPut(merged).catch(() => {
			// Persist failure doesn't undo the local toggle — the user
			// asked for it now, and the next play action still runs the
			// updated decision. The settings file will be re-attempted
			// on the next change.
		});
	}

	/** Wired to <video onended>. Consults the pure decision function
	 *  with the current toggle + episode position; if the answer is
	 *  "advance," reuses the prev/next switchToEpisode pipeline. */
	function onVideoEnded() {
		const decision = decideAutoPlayNext({
			enabled: autoPlayNext,
			episodeNum,
			totalEpisodes
		});
		if (decision.advance) {
			void switchToEpisode(decision.target);
		}
	}

	// Hand the currently-playing episode off to the user's mpv (or
	// whichever player they configured). The backend resolves the same
	// upstream URL it would for the embedded path; only the terminal
	// action differs. Errors surface as a short-lived inline notice
	// rather than the LoadingOverlay so the playing video keeps going.
	let externalNotice = $state<string | null>(null);
	let externalBusy = $state(false);

	// Overflow menu (the "..." button) housing the secondary actions
	// Open in external + Download. They were demoted from the np-actions
	// row to declutter — the primary controls are now ep-nav +
	// auto-play, with everything else one click deeper.
	let moreOpen = $state(false);
	let moreAnchor = $state<HTMLButtonElement | null>(null);
	function toggleMore() {
		moreOpen = !moreOpen;
	}
	function closeMore() {
		moreOpen = false;
	}
	$effect(() => {
		if (!moreOpen) return;
		// Click outside the menu OR its trigger closes it; Escape closes
		// it and returns focus to the trigger so keyboard users don't
		// lose their place.
		const onPointerDown = (e: PointerEvent) => {
			const target = e.target as Node | null;
			if (!target) return;
			if (moreAnchor?.contains(target)) return;
			const menu = document.getElementById('np-more-menu');
			if (menu?.contains(target)) return;
			closeMore();
		};
		const onKey = (e: KeyboardEvent) => {
			if (e.key === 'Escape') {
				closeMore();
				moreAnchor?.focus();
			}
		};
		document.addEventListener('pointerdown', onPointerDown);
		document.addEventListener('keydown', onKey);
		return () => {
			document.removeEventListener('pointerdown', onPointerDown);
			document.removeEventListener('keydown', onKey);
		};
	});
	async function onOpenExternal() {
		const title = detail?.canonical_title;
		if (!title || !config) return;
		// Pause the embedded video so the user isn't watching two
		// instances simultaneously. The notice/launch flow continues
		// regardless; if the external player fails to start the user
		// can press space (or click the embedded player) to resume.
		videoEl?.pause();
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

	// Download flow — opens the DownloadConfirm modal. The modal lets
	// the user pick a destination folder (defaulting to the backend's
	// download_dir resolver) before kicking off ani-cli -d. Once
	// confirmed, the download lives in the global download store and
	// surfaces in the topbar dock + bottom progress strip.
	let downloadModalOpen = $state(false);
	let downloadArgs = $state<DownloadArgs | null>(null);
	let downloadDefaultDir = $state('');

	void downloadDefaultDirApi()
		.then((d) => {
			if (d) downloadDefaultDir = d;
		})
		.catch(() => {});

	function onDownload() {
		if (!detail) return;
		const mode = (config?.mode === 'dub' ? 'dub' : 'sub') as 'sub' | 'dub';
		const quality = config?.quality ?? 'best';
		downloadArgs = {
			title: detail.canonical_title,
			episode: String(episodeNum),
			mode,
			quality,
			episode_count: detail.episode_count ?? undefined,
			alt_titles: altTitlesFromKitsu(detail),
			kitsu_id: id
		};
		downloadModalOpen = true;
	}

	// Keyboard shortcuts: `n` / `p` step episodes, `f` toggles
	// fullscreen on the player frame. Arrow keys are left to the
	// <video> element for seek control. Modifier presses (Ctrl/Cmd/
	// Alt) are ignored so we don't shadow browser shortcuts like
	// Ctrl+F (find).
	$effect(() => {
		if (typeof window === 'undefined') return;
		const onKey = (e: KeyboardEvent) => {
			const t = e.target as HTMLElement | null;
			const inField =
				t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable);
			if (inField) return;
			if (e.ctrlKey || e.metaKey || e.altKey) return;
			if (e.key === 'n' || e.key === 'N') {
				e.preventDefault();
				onNext();
			} else if (e.key === 'p' || e.key === 'P') {
				e.preventDefault();
				onPrev();
			} else if (e.key === 'f' || e.key === 'F') {
				e.preventDefault();
				toggleFullscreen();
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
	<!-- Player extends to the page's full content width (Patreon-style):
	     a black frame whose height is viewport-capped, with the <video>
	     letterboxed inside via object-fit: contain. The watch-column
	     below caps everything else (now-playing, episodes, similar) at
	     a readable width. -->
	<section class="player-frame" class:player-busy={switchBusy}>
		{#if !sessionId}
			<p class="player-empty">No session in URL — return to the show page and pick an episode.</p>
		{:else if playerError}
			<p class="player-empty">{playerError}</p>
		{:else}
			<video
				bind:this={videoEl}
				bind:currentTime
				bind:duration
				bind:paused={isPaused}
				bind:volume={videoVolume}
				bind:muted={isMuted}
				autoplay
				controls={!USE_CUSTOM_PLAYER_CONTROLS}
				onclick={USE_CUSTOM_PLAYER_CONTROLS ? togglePlay : undefined}
				onended={onVideoEnded}
			>
				{#if subtitleUrl}
					<track kind="subtitles" label="Subtitles" srclang="en" src={subtitleUrl} default />
				{/if}
			</video>

			<!-- Custom controls overlay — toggled by the
				     USE_CUSTOM_PLAYER_CONTROLS feature flag (off by
				     default). When on, replaces Chromium's native
				     timeline so the progress bar can take the
				     per-show accent color. -->
			{#if USE_CUSTOM_PLAYER_CONTROLS}
				<div class="player-controls" class:scrubber-hover={scrubberHover}>
					<button
						type="button"
						class="pc-btn"
						onclick={togglePlay}
						aria-label={isPaused ? 'Play' : 'Pause'}
					>
						{#if isPaused}
							<svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
								<path d="M8 5v14l11-7z" fill="currentColor" />
							</svg>
						{:else}
							<svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
								<rect x="6" y="5" width="4" height="14" fill="currentColor" />
								<rect x="14" y="5" width="4" height="14" fill="currentColor" />
							</svg>
						{/if}
					</button>

					<span class="pc-time" aria-label="Current time">
						{formatTime(currentTime)} <span class="pc-time-sep">/</span>
						{formatTime(duration)}
					</span>

					<div
						class="pc-scrubber"
						role="slider"
						tabindex="0"
						aria-label="Seek"
						aria-valuemin="0"
						aria-valuemax={Math.max(1, Math.floor(duration))}
						aria-valuenow={Math.floor(currentTime)}
						onclick={onScrubberClick}
						onmouseenter={() => (scrubberHover = true)}
						onmouseleave={() => (scrubberHover = false)}
						onkeydown={(e) => {
							if (e.key === 'ArrowRight') seekToFraction((currentTime + 5) / duration);
							else if (e.key === 'ArrowLeft') seekToFraction((currentTime - 5) / duration);
						}}
					>
						<div class="pc-scrubber-track">
							<div
								class="pc-scrubber-fill"
								style:inline-size="{duration ? (currentTime / duration) * 100 : 0}%"
							></div>
							<div
								class="pc-scrubber-thumb"
								style:inset-inline-start="{duration ? (currentTime / duration) * 100 : 0}%"
							></div>
						</div>
					</div>

					<button
						type="button"
						class="pc-btn"
						onclick={toggleMute}
						aria-label={isMuted ? 'Unmute' : 'Mute'}
					>
						{#if isMuted || videoVolume === 0}
							<svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
								<path
									d="M16.5 12L19 9.5l1.5 1.5L18 13.5l2.5 2.5-1.5 1.5L16.5 15l-2.5 2.5L12.5 16l2.5-2.5-2.5-2.5L14 9.5zM3 9v6h4l5 5V4L7 9z"
									fill="currentColor"
								/>
							</svg>
						{:else}
							<svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
								<path
									d="M3 9v6h4l5 5V4L7 9zm13.5 3a4.5 4.5 0 00-2.5-4v8a4.5 4.5 0 002.5-4z"
									fill="currentColor"
								/>
							</svg>
						{/if}
					</button>

					<button
						type="button"
						class="pc-btn"
						onclick={toggleFullscreen}
						aria-label="Toggle fullscreen"
					>
						<svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
							<path
								d="M7 14H5v5h5v-2H7zm-2-4h2V7h3V5H5zm12 7h-3v2h5v-5h-2zm-3-12v2h3v3h2V5z"
								fill="currentColor"
							/>
						</svg>
					</button>
				</div>
			{/if}
		{/if}
		{#if switchBusy}
			<span class="player-spinner" aria-hidden="true">…</span>
		{/if}
	</section>

	<div class="watch-column">
		<!-- Now-playing row: compact breadcrumb under the player so the
		     page hierarchy reads "video → context → more episodes →
		     similar". Show title links back to the detail page; episode
		     prev/next + external player live on the right. -->
		<header class="now-playing">
			<a
				class="np-link"
				href={resolve('/anime/[id]', { id })}
				onclick={(e) => {
					// Replace, not push — treats poster → details as an "up"
					// navigation rather than burying the player in history.
					e.preventDefault();
					void goto(resolve('/anime/[id]', { id }), { replaceState: true });
				}}
			>
				<!-- title attrs surface the full string when the text is
				     truncated by the row's no-wrap shrink. Native title
				     fires after the OS hover delay (~600ms) — that's the
				     conventional "wait then reveal" affordance the user
				     asked for, not the immediate custom tooltip used on
				     the disabled "All" button. -->
				<span class="np-show" title={detail?.canonical_title ?? ''}
					>{detail?.canonical_title ?? 'Loading…'}</span
				>
				<span class="np-sep" aria-hidden="true">·</span>
				<span class="np-ep">Episode {episodeNum}</span>
				{#if currentEpisodeMeta?.canonical_title}
					<span class="np-em-dash" aria-hidden="true">—</span>
					<span class="np-ep-title" title={currentEpisodeMeta.canonical_title}
						>{currentEpisodeMeta.canonical_title}</span
					>
				{/if}
			</a>

			<div class="np-actions">
				<div class="ep-nav" role="group" aria-label="Episode navigation">
					<button
						type="button"
						class="ep-btn"
						onclick={onPrev}
						disabled={!hasPrev || switchBusy}
						aria-label="Previous episode"
					>
						<svg class="ep-btn-icon" viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
							<path
								d="M15 5l-7 7 7 7"
								fill="none"
								stroke="currentColor"
								stroke-width="2.25"
								stroke-linecap="round"
								stroke-linejoin="round"
							/>
						</svg>
						<span>Ep {episodeNum - 1}</span>
					</button>
					<button type="button" class="ep-btn ep-current-btn" disabled aria-current="true">
						<span>Ep {episodeNum}</span>
						<span class="bars" aria-hidden="true">
							<span></span><span></span><span></span>
						</span>
					</button>
					<button
						type="button"
						class="ep-btn"
						onclick={onNext}
						disabled={!hasNext || switchBusy}
						aria-label="Next episode"
					>
						<span>Ep {episodeNum + 1}</span>
						<svg class="ep-btn-icon" viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
							<path
								d="M9 5l7 7-7 7"
								fill="none"
								stroke="currentColor"
								stroke-width="2.25"
								stroke-linecap="round"
								stroke-linejoin="round"
							/>
						</svg>
					</button>
				</div>

				<button
					type="button"
					class="ep-btn ep-toggle"
					onclick={toggleAutoPlayNext}
					aria-pressed={autoPlayNext}
					aria-label="Auto-play next episode"
					title={autoPlayNext
						? 'Auto-play is on — episodes advance automatically'
						: 'Auto-play is off — click to enable'}
				>
					<!-- The "Auto-play" label disambiguates from a skip/next
					     control. Filled play+forward-bar when on (accent
					     background); outlined and faded when off. -->
					<svg class="ep-btn-icon" viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
						{#if autoPlayNext}
							<path d="M5 4l9 8-9 8V4z" fill="currentColor" />
							<rect x="16" y="4" width="2.5" height="16" rx="1" fill="currentColor" />
						{:else}
							<path
								d="M5 4l9 8-9 8V4zm12 0v16"
								fill="none"
								stroke="currentColor"
								stroke-width="2.25"
								stroke-linecap="round"
								stroke-linejoin="round"
							/>
						{/if}
					</svg>
					<span>Auto-play</span>
				</button>

				<div class="more-wrap">
					<button
						type="button"
						class="ep-btn ep-icon-btn"
						bind:this={moreAnchor}
						onclick={toggleMore}
						aria-haspopup="menu"
						aria-expanded={moreOpen}
						aria-controls="np-more-menu"
						aria-label="More actions"
						title="More actions"
					>
						<svg class="ep-btn-icon" viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
							<circle cx="5" cy="12" r="1.7" fill="currentColor" />
							<circle cx="12" cy="12" r="1.7" fill="currentColor" />
							<circle cx="19" cy="12" r="1.7" fill="currentColor" />
						</svg>
					</button>

					{#if moreOpen}
						<div
							id="np-more-menu"
							class="more-menu"
							role="menu"
							aria-label="More actions"
							transition:settle={{ duration: 120 }}
						>
							<button
								type="button"
								class="more-item"
								role="menuitem"
								onclick={() => {
									closeMore();
									void onOpenExternal();
								}}
								disabled={switchBusy || externalBusy}
							>
								<svg
									class="more-icon"
									viewBox="0 0 24 24"
									width="18"
									height="18"
									aria-hidden="true"
								>
									<path
										d="M14 5h5v5M19 5L10 14M5 9v10h10"
										fill="none"
										stroke="currentColor"
										stroke-width="2.25"
										stroke-linecap="round"
										stroke-linejoin="round"
									/>
								</svg>
								<span>{externalBusy ? 'Launching…' : 'Open in external'}</span>
							</button>

							<button
								type="button"
								class="more-item"
								role="menuitem"
								onclick={() => {
									closeMore();
									onDownload();
								}}
								disabled={switchBusy}
							>
								<svg
									class="more-icon"
									viewBox="0 0 24 24"
									width="18"
									height="18"
									aria-hidden="true"
								>
									<path
										d="M12 4v12m0 0l-4-4m4 4l4-4M5 20h14"
										fill="none"
										stroke="currentColor"
										stroke-width="2.25"
										stroke-linecap="round"
										stroke-linejoin="round"
									/>
								</svg>
								<span>Download</span>
							</button>
						</div>
					{/if}
				</div>
			</div>
		</header>

		{#if externalNotice}
			<p class="external-notice" role="status">{externalNotice}</p>
		{/if}

		{#if detailError}
			<p class="player-empty">{detailError}</p>
		{/if}

		<!-- Episodes carousel — already a horizontal-scroll strip. Drives
		     the same paginated data model as the detail page (toolbar
		     above for jump/seek; cards below). -->
		<section class="ep-section" aria-label="Episodes">
			<!-- Single compact toolbar: heading + range on the left, jump
			     pill + prev/next chevrons on the right. No scrubber here
			     (the /play page should stay compact under the player —
			     scrubber lives on the detail page where the user is
			     browsing). -->
			<div class="ep-toolbar">
				<div class="ep-toolbar-left">
					<h2 class="ep-section-heading">Episodes</h2>
					<span class="ep-range">
						{#if episodes && episodes.length > 0 && detail?.episode_count}
							{#if totalEpisodePages !== null && totalEpisodePages > 1}
								{epStart}–{epEnd} of {detail.episode_count}
							{:else}
								{detail.episode_count} episodes
							{/if}
						{:else if episodes && episodes.length > 0}
							page {episodesPage}
						{:else if episodesError}
							unavailable
						{:else}
							loading…
						{/if}
					</span>
				</div>

				{#if totalEpisodePages !== null ? totalEpisodePages > 1 : (episodes?.length ?? 0) >= UI_PAGE_SIZE}
					<div class="ep-toolbar-right">
						<form class="ep-jump" onsubmit={jumpToEpisode}>
							<span class="ep-jump-key" aria-hidden="true">jump</span>
							<span class="ep-jump-pill">
								<input
									class="jump-input"
									type="number"
									min="1"
									max={detail?.episode_count ?? 9999}
									step="1"
									placeholder="ep #"
									aria-label="Jump to episode number"
									bind:value={jumpInput}
								/>
								<button
									type="submit"
									class="ep-jump-go"
									disabled={!jumpInput || episodesLoading}
									aria-label="Go to episode"
								>
									<svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true">
										<path
											d="M9 5l7 7-7 7"
											fill="none"
											stroke="currentColor"
											stroke-width="2.5"
											stroke-linecap="round"
											stroke-linejoin="round"
										/>
									</svg>
								</button>
							</span>
						</form>
						<div class="ep-pager-mini" role="group" aria-label="Episode pagination">
							<button
								type="button"
								class="ep-pager-mini-btn"
								onclick={() => gotoPage(episodesPage - 1)}
								disabled={episodesPage <= 1 || episodesLoading}
								aria-label="Previous page"
							>
								<svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true">
									<path
										d="M15 5l-7 7 7 7"
										fill="none"
										stroke="currentColor"
										stroke-width="2.5"
										stroke-linecap="round"
										stroke-linejoin="round"
									/>
								</svg>
							</button>
							<button
								type="button"
								class="ep-pager-mini-btn"
								onclick={() => gotoPage(episodesPage + 1)}
								disabled={(totalEpisodePages !== null && episodesPage >= totalEpisodePages) ||
									episodesLoading ||
									(episodes !== null && episodes.length < UI_PAGE_SIZE)}
								aria-label="Next page"
							>
								<svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true">
									<path
										d="M9 5l7 7-7 7"
										fill="none"
										stroke="currentColor"
										stroke-width="2.5"
										stroke-linecap="round"
										stroke-linejoin="round"
									/>
								</svg>
							</button>
						</div>
					</div>
				{/if}
			</div>

			{#if episodes && episodes.length > 0}
				<!-- Single fluid row of 5 cards. No scroll: cards shrink/
				     grow with the container so all 5 are always visible. -->
				<ol class="ep-row" aria-label="Episodes">
					{#each episodes as ep, i (ep.id)}
						{@const n = ep.number ?? ep.relative_number ?? 0}
						{@const isCurrent = n === episodeNum}
						{@const epThumb = imageProxyUrl(ep.thumbnail?.original ?? null)}
						<!-- in: only, no out:. With a 5-col grid, simultaneously
						     mounting outgoing + incoming cards wraps to two
						     rows for ~320ms during a page change, pushing the
						     strip below down. Letting old cards unmount
						     instantly keeps the row at one row's height; the
						     staggered in: still gives the new cards a settle
						     feel as they land. -->
						<li
							class:ep-highlight={n === highlightEp}
							data-ep-num={n}
							in:settle={{ duration: 620, delay: i * 45 }}
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
									<span class="ep-card-thumb-play" aria-hidden="true">
										<svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true">
											<path d="M8 5v14l11-7z" fill="currentColor" />
										</svg>
									</span>
									{#if isCurrent}
										<span class="ep-card-thumb-flag" aria-hidden="true">Now playing</span>
									{/if}
								</span>
								<span class="ep-card-foot">
									<span class="ep-card-foot-row">
										<span class="ep-card-foot-num">EP {n}</span>
										{#if ep.length}
											<span class="ep-card-foot-dot" aria-hidden="true">·</span>
											<span class="ep-card-foot-len">{ep.length}m</span>
										{/if}
									</span>
									<span class="ep-card-foot-title">
										{ep.canonical_title ?? `Episode ${n}`}
									</span>
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
				<Strip eyebrow="More like this" pad="0" cardWidth="11.25rem">
					{#each similar as hit (hit.id)}
						<PosterCard anime={hit} />
					{/each}
				</Strip>
			</div>
		{/if}
	</div>
</main>

<LoadingOverlay visible={switchBusy} progress={switchProgress} />

{#if playFailure}
	<ErrorOverlay
		headline={`Couldn't play episode ${playFailure.episode}`}
		body={playFailure.message}
		onDismiss={() => (playFailure = null)}
	/>
{/if}

<DownloadConfirm
	bind:open={downloadModalOpen}
	args={downloadArgs}
	defaultDir={downloadDefaultDir}
	onClose={() => (downloadModalOpen = false)}
/>

<style>
	/* Page is full content area; the inner .watch-column carries
	   the viewport-aware width cap. The page background spreads
	   the atmospheric glow across the full screen so wide /
	   ultrawide windows still feel "lit" outside the column. */
	.page {
		position: relative;
		display: flex;
		flex-direction: column;
		padding-block: var(--space-7) var(--space-8);
		padding-inline: var(--space-6);
		inline-size: 100%;
		isolation: isolate;
	}
	.page::before {
		content: '';
		position: absolute;
		inset-block-start: 0;
		inset-inline: 0;
		block-size: 50rem;
		z-index: -1;
		pointer-events: none;
		background: radial-gradient(
			ellipse 80rem 32rem at 50% 0%,
			color-mix(in oklab, var(--accent) 35%, transparent) 0%,
			color-mix(in oklab, var(--accent) 20%, transparent) 25%,
			color-mix(in oklab, var(--accent) 8%, transparent) 55%,
			transparent 100%
		);
		filter: blur(50px);
		opacity: 0.95;
	}

	/* Watch column: viewport-aware width cap so the player never
	   pushes the rest of the page off the fold on tall windows or
	   stretches absurdly wide on ultrawides. The min() of:
	     • 100% of the page padding box,
	     • 85rem (~1360px) — sane absolute ceiling,
	     • height-derived width (100dvh − chrome) × 16/9.
	   320px chrome reserve covers topbar + breadcrumb + hero +
	   player margins so the start of the Episodes section is
	   visible below the player on a typical viewport. */
	.watch-column {
		display: flex;
		flex-direction: column;
		gap: var(--space-7);
		/* Single source of truth for column width: same formula as the
		   player frame. Every child below stretches to 100% of this
		   column, so the now-playing row, the episodes toolbar/cards,
		   and the similar-titles strip all share one left/right edge
		   with the player above them. */
		inline-size: min(100%, 1360px, calc((100dvh - var(--player-reserved-height)) * 16 / 9));
		margin-inline: auto;
	}

	/* Header is a 2-column grid: show identity (poster + title +
	   meta) on the left, controls cluster on the right at the
	   same y. flex-wrap was letting the actions drop to a new
	   line when the title got long, which broke the screenshot's
	   layout. Below 900px it collapses to a single column. */
	/* Now-playing row — compact breadcrumb under the player. Replaces the
	   old multi-row hero (poster + meta + ep-nav + action-row) so the
	   page hierarchy reads "video → context → carousel → similar". */
	.now-playing {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-5);
		/* No-wrap: controls must never drop to a second line. The
		   np-link inside shrinks via flex + ellipsis so the row
		   stays on a single line at narrower widths. */
		flex-wrap: nowrap;
		margin-block-start: var(--space-2);
	}
	.np-link {
		display: inline-flex;
		align-items: baseline;
		flex: 1 1 0;
		min-inline-size: 0;
		flex-wrap: nowrap;
		gap: 0 var(--space-3);
		color: inherit;
		text-decoration: none;
		overflow: hidden;
	}
	.np-show {
		font-family: var(--font-body);
		font-size: var(--type-body-l);
		font-weight: 600;
		color: var(--bone-100);
		transition: color var(--dur-fast) var(--ease-out-soft);
		/* Truncates second — only after the episode title has been
		   fully clipped. flex-shrink: 1 (default) is dwarfed by the
		   ep-title's flex-shrink: 100, so the show name keeps its
		   width as long as possible. */
		min-inline-size: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.np-link:hover .np-show {
		color: var(--accent);
	}
	.np-sep {
		flex: 0 0 auto;
		color: color-mix(in oklab, var(--bone-100) 30%, transparent);
	}
	.np-ep {
		flex: 0 0 auto;
		font-family: var(--font-body);
		font-size: 0.75rem;
		font-weight: 600;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--accent);
	}
	.np-em-dash {
		flex: 0 0 auto;
		color: color-mix(in oklab, var(--bone-100) 30%, transparent);
	}
	.np-ep-title {
		font-family: var(--font-body);
		font-size: var(--type-meta);
		color: color-mix(in oklab, var(--bone-100) 78%, transparent);
		/* Truncates first when the row gets tight — the episode's own
		   title is the lowest-priority piece of context. flex-shrink
		   100 makes it lose width far ahead of the show name. */
		flex: 1 100 0;
		min-inline-size: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.np-actions {
		display: inline-flex;
		align-items: center;
		gap: var(--space-3);
		flex: 0 0 auto;
	}
	.ep-nav {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
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
		font-family: var(--font-body);
		font-size: 0.8125rem; /* 13px */
		font-weight: 500;
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

	/* Filled-accent current-episode pill — communicates "playing
	   right now" with the animated bars icon below. */
	.ep-current-btn {
		background: color-mix(in oklab, var(--accent) 65%, var(--ink-000));
		color: var(--bone-100);
		border-color: color-mix(in oklab, var(--accent) 90%, var(--bone-100));
	}
	.ep-current-btn:disabled {
		opacity: 1;
		cursor: default;
	}
	/* Icon-only buttons (auto-play toggle, kebab) — drop the textual
	   label's horizontal padding so they render as squares with the
	   18px icon centred. Width matches block-size so the hit target
	   stays comfortably tappable. */
	.ep-icon-btn {
		padding-inline: var(--space-2);
		min-inline-size: 2.25rem;
		justify-content: center;
	}
	/* Auto-play in the off state reads as a low-contrast outlined
	   icon — no accent, slightly faded. */
	.ep-toggle[aria-pressed='false'] {
		color: color-mix(in oklab, var(--bone-100) 60%, transparent);
	}
	/* aria-pressed=true paints the toggle button accent so the on
	   state reads at a glance — accent fill + filled icon (rendered
	   in markup) instead of an outlined glyph. */
	.ep-toggle[aria-pressed='true'] {
		background: color-mix(in oklab, var(--accent) 32%, var(--ink-050));
		border-color: color-mix(in oklab, var(--accent) 70%, var(--bone-400));
		color: var(--accent);
	}
	.ep-toggle[aria-pressed='true']:hover:not(:disabled) {
		background: color-mix(in oklab, var(--accent) 42%, var(--ink-050));
	}

	/* "..." overflow menu — anchored absolute relative to the wrapper
	   so the popover sits under the trigger without disturbing the
	   np-actions row's flexbox flow. */
	.more-wrap {
		position: relative;
	}
	.more-menu {
		position: absolute;
		inset-block-start: calc(100% + var(--space-2));
		inset-inline-end: 0;
		display: flex;
		flex-direction: column;
		min-inline-size: 12rem;
		padding: var(--space-2);
		background: color-mix(in oklab, var(--ink-050) 92%, var(--accent));
		border: 1px solid color-mix(in oklab, var(--accent) 25%, var(--bone-400));
		border-radius: var(--radius-card);
		box-shadow: var(--shadow-card-hover);
		z-index: 10;
	}
	.more-item {
		display: inline-flex;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-2) var(--space-3);
		background: transparent;
		border: 0;
		color: var(--bone-100);
		font-family: var(--font-body);
		font-size: var(--type-body-s);
		text-align: start;
		border-radius: calc(var(--radius-card) - var(--space-1));
		cursor: pointer;
		transition: background var(--dur-fast) var(--ease-out-soft);
	}
	.more-item:hover:not(:disabled),
	.more-item:focus-visible {
		background: color-mix(in oklab, var(--accent) 18%, transparent);
		outline: none;
	}
	.more-item:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
	.more-icon {
		flex-shrink: 0;
	}
	.bars {
		display: inline-flex;
		align-items: end;
		gap: 2px;
		block-size: 0.85rem;
	}
	.bars span {
		inline-size: 2px;
		background: var(--bone-100);
		border-radius: 1px;
		transform-origin: bottom;
		animation: bars-bounce 0.9s var(--ease-in-out) infinite;
	}
	.bars span:nth-child(1) {
		block-size: 50%;
		animation-delay: 0s;
	}
	.bars span:nth-child(2) {
		block-size: 90%;
		animation-delay: 0.18s;
	}
	.bars span:nth-child(3) {
		block-size: 65%;
		animation-delay: 0.36s;
	}
	@keyframes bars-bounce {
		0%,
		100% {
			transform: scaleY(0.4);
		}
		50% {
			transform: scaleY(1);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.bars span {
			animation: none;
			transform: scaleY(0.7);
		}
	}

	/* Square icon-only button (bookmark slot) — visual companion
	   to the external action button. */
	.external-notice {
		margin: var(--space-3) 0 0;
		padding: var(--space-2) var(--space-3);
		font-size: var(--type-meta);
		color: var(--bone-100);
		background: rgba(0, 0, 0, 0.4);
		border-radius: var(--radius-control);
	}
	.player-frame {
		position: relative;
		/* Full-bleed Patreon-style player: escapes both .page's
		   inline padding (--space-6 = 32px) AND the layout's
		   .main-area inline padding (--space-7 = 48px), so the
		   frame touches the rail on the left and the window edge
		   on the right. Negative block-start margin pulls it up to
		   touch the topbar's bottom edge (escaping .page's
		   padding-block-start). */
		--player-edge-escape: calc(var(--space-7) + var(--space-6));
		inline-size: calc(100% + 2 * var(--player-edge-escape));
		margin-inline: calc(-1 * var(--player-edge-escape));
		margin-block-start: calc(-1 * var(--space-7));
		block-size: clamp(20rem, calc(100dvh - var(--player-reserved-height)), 50rem);
		background: #000;
		/* Margin under the player so the now-playing row gets
		   breathing room. The watch-column gap normally handles
		   that, but the player is now its sibling — gap doesn't
		   apply. Tightened from space-7 to space-5 — the previous
		   gap dwarfed the now-playing row beneath. */
		margin-block-end: var(--space-5);
		/* No border / no radius / no glow — the frame is a true
		   full-bleed black surface, the way the Patreon player
		   reads. The ambient accent glow used to live on the
		   `.page::before` gradient instead. */
		overflow: hidden;
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
		cursor: pointer;
		/* Letterbox: video stays at 16:9 aspect, centered inside the
		   frame, with black bars on whichever axis has slack. Frame
		   is wider than 16:9 of its height on most desktop windows,
		   so the bars sit on the left + right. */
		object-fit: contain;
	}

	/* Custom controls overlay — Chromium's native media controls
	   timeline can't be styled (locked shadow DOM), so we render
	   our own bar at the bottom of the frame. The progress fill
	   uses var(--accent), so the timeline matches the rest of
	   the page's per-show theming. */
	.player-controls {
		position: absolute;
		inset-inline: 0;
		inset-block-end: 0;
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-3) var(--space-4);
		background: linear-gradient(180deg, transparent 0%, rgb(0 0 0 / 0.75) 100%);
		color: var(--bone-100);
		opacity: 0;
		transition: opacity var(--dur-fast) var(--ease-out-soft);
	}
	.player-frame:hover .player-controls,
	.player-controls:focus-within,
	.player-controls.scrubber-hover {
		opacity: 1;
	}
	.pc-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		inline-size: 2rem;
		block-size: 2rem;
		padding: 0;
		border: 0;
		border-radius: 50%;
		background: transparent;
		color: var(--bone-100);
		cursor: pointer;
		transition: background var(--dur-fast) var(--ease-out-soft);
	}
	.pc-btn:hover {
		background: color-mix(in oklab, var(--bone-100) 18%, transparent);
	}
	.pc-time {
		font-family: var(--font-body);
		font-size: 0.8125rem; /* 13px */
		font-variant-numeric: tabular-nums;
		color: var(--bone-100);
		min-inline-size: 8.5rem;
		text-align: center;
	}
	.pc-time-sep {
		color: color-mix(in oklab, var(--bone-100) 40%, transparent);
		margin-inline: 0.2em;
	}

	.pc-scrubber {
		flex: 1;
		display: flex;
		align-items: center;
		block-size: 1.5rem;
		cursor: pointer;
		min-inline-size: 0;
	}
	.pc-scrubber:focus-visible {
		outline: none;
	}
	.pc-scrubber-track {
		position: relative;
		inline-size: 100%;
		block-size: 4px;
		border-radius: 999px;
		background: color-mix(in oklab, var(--bone-100) 18%, transparent);
		transition: block-size var(--dur-fast) var(--ease-out-soft);
	}
	.pc-scrubber:hover .pc-scrubber-track,
	.pc-scrubber:focus-visible .pc-scrubber-track {
		block-size: 6px;
	}
	.pc-scrubber-fill {
		position: absolute;
		inset-block: 0;
		inset-inline-start: 0;
		background: var(--accent);
		border-radius: inherit;
	}
	.pc-scrubber-thumb {
		position: absolute;
		inset-block-start: 50%;
		inline-size: 12px;
		block-size: 12px;
		margin-inline-start: -6px;
		border-radius: 50%;
		background: var(--accent);
		transform: translateY(-50%) scale(0);
		transition: transform var(--dur-fast) var(--ease-out-soft);
		box-shadow: 0 2px 6px rgb(0 0 0 / 0.4);
	}
	.pc-scrubber:hover .pc-scrubber-thumb,
	.pc-scrubber:focus-visible .pc-scrubber-thumb {
		transform: translateY(-50%) scale(1);
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
		font-family: var(--font-body);
		font-size: var(--type-display-l);
		pointer-events: none;
	}

	/* Similar Titles wrapper — fills the watch-column (no per-section
	   cap). Zeroes Strip's internal --strip-pad (via :global pierce,
	   since the .strip element sets the variable on itself), so the
	   strip aligns flush with the player + episodes above. */
	.similar-wrap {
		inline-size: 100%;
	}
	/* "More like this" is demoted relative to the episodes above — it's
	   a recommendation, not a primary action. Eyebrow scales down from
	   the previous 28px display heading to a regular 20px section
	   heading (matching the Episodes toolbar h2 above), so the row
	   reads as supporting content, not a peer to the player. The
	   accent rule + bone-300 caption stay so the editorial voice
	   carries through. */
	.similar-wrap :global(.strip-header) {
		justify-content: flex-start;
	}
	.similar-wrap :global(.eyebrow) {
		font-family: var(--font-body);
		font-size: 1.25rem; /* 20px — matches Episodes toolbar h2 */
		font-weight: 600;
		line-height: 1.1;
		letter-spacing: -0.01em;
		color: var(--bone-100);
		text-transform: none;
	}
	.similar-wrap :global(.eyebrow-key) {
		color: var(--bone-100);
		font: inherit;
		letter-spacing: inherit;
		text-transform: inherit;
	}
	.similar-wrap :global(.eyebrow-rule) {
		background: var(--accent);
		inline-size: 2.5rem;
		block-size: 1px;
		align-self: center;
	}
	.similar-wrap :global(.eyebrow-value) {
		color: var(--bone-300);
		font-family: var(--font-body);
		font-style: normal;
		font-size: var(--type-meta);
		letter-spacing: var(--tracking-meta);
		text-transform: uppercase;
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
		/* Width comes from the watch-column above — no per-section cap
		   so the toolbar + card row align with the player + similar
		   strip on the same left/right edges. */
		inline-size: 100%;
	}
	/* Single horizontal toolbar — heading + range on the left, jump
	   pill + prev/next chevrons on the right. Same vocabulary as the
	   detail page (jump pill, accent halo on focus, weighted SVG
	   chevrons), minus the editorial scrubber — the /play page should
	   stay compact under the player. */
	.ep-toolbar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-4);
		flex-wrap: wrap;
		margin-block-end: var(--space-4);
	}
	.ep-toolbar-left {
		display: inline-flex;
		align-items: baseline;
		gap: var(--space-4);
		flex-wrap: wrap;
	}
	.ep-toolbar-right {
		display: inline-flex;
		align-items: center;
		gap: var(--space-3);
		flex-wrap: wrap;
	}
	.ep-section-heading {
		margin: 0;
		font-family: var(--font-body);
		font-size: 1.25rem; /* 20px — section, not display */
		font-weight: 600;
		line-height: 1.1;
		color: var(--bone-100);
		letter-spacing: -0.01em;
	}
	.ep-range {
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: var(--type-meta);
		font-weight: 500;
		letter-spacing: var(--tracking-meta);
		color: var(--bone-300);
	}

	/* Jump pill — same shape as the detail page version. JUMP eyebrow
	   outside, marquee numerals inside, accent halo on focus, embedded
	   chevron submit. */
	.ep-jump {
		display: inline-flex;
		align-items: center;
		gap: var(--space-3);
	}
	.ep-jump-key {
		font-family: var(--font-body);
		font-size: 0.75rem;
		font-weight: 600;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--bone-300);
		transition: color var(--dur-fast) var(--ease-out-soft);
	}
	.ep-jump:focus-within .ep-jump-key {
		color: var(--bone-200);
	}
	.ep-jump-pill {
		display: inline-flex;
		align-items: center;
		inline-size: 8.25rem; /* 132px */
		block-size: 2rem;
		padding-inline-start: var(--space-3);
		padding-inline-end: 4px;
		border-radius: var(--radius-pill);
		background: color-mix(in oklab, var(--bone-100) 7%, transparent);
		border: 1px solid color-mix(in oklab, var(--bone-100) 28%, transparent);
		transition:
			border-color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft),
			box-shadow var(--dur-med) var(--ease-out-soft);
	}
	.ep-jump-pill:focus-within {
		border-color: var(--accent);
		background: color-mix(in oklab, var(--bone-100) 10%, transparent);
		box-shadow:
			0 0 8px color-mix(in oklab, var(--accent) 50%, transparent),
			0 0 16px color-mix(in oklab, var(--accent) 25%, transparent);
	}
	.jump-input {
		flex: 1;
		min-inline-size: 0;
		padding: 0;
		background: transparent;
		border: 0;
		outline: 0;
		color: var(--bone-100);
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: var(--type-body);
		font-weight: 600;
	}
	.jump-input::placeholder {
		font-family: var(--font-body);
		font-weight: 400;
		font-size: var(--type-meta);
		letter-spacing: 0;
		color: color-mix(in oklab, var(--bone-100) 55%, transparent);
	}
	.jump-input::-webkit-inner-spin-button,
	.jump-input::-webkit-outer-spin-button {
		appearance: none;
	}
	.jump-input:focus,
	.jump-input:focus-visible {
		outline: 0;
		box-shadow: none;
	}
	.ep-jump-go {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		inline-size: 24px;
		block-size: 24px;
		padding: 0;
		border: 0;
		border-radius: var(--radius-pill);
		background: transparent;
		color: var(--bone-300);
		cursor: pointer;
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft),
			transform var(--dur-med) var(--ease-out-elastic);
	}
	.ep-jump-go:hover:not(:disabled),
	.ep-jump-go:focus-visible {
		color: var(--accent);
		background: color-mix(in oklab, var(--accent) 14%, transparent);
		transform: translateX(2px);
		outline: 0;
	}
	.ep-jump-go:disabled {
		color: color-mix(in oklab, var(--bone-100) 24%, transparent);
		cursor: not-allowed;
	}
	.ep-jump-pill:has(.jump-input:not(:placeholder-shown)) .ep-jump-go:not(:disabled) {
		color: var(--accent);
	}

	/* Prev/next chevrons — small bordered chips with weighted SVG
	   arrows. Hover lifts to accent + halo; chevron slides in its
	   pointing direction. */
	.ep-pager-mini {
		display: inline-flex;
		gap: var(--space-1);
	}
	.ep-pager-mini-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		inline-size: 2rem;
		block-size: 2rem;
		padding: 0;
		border: 1px solid color-mix(in oklab, var(--bone-100) 22%, transparent);
		border-radius: var(--radius-control);
		background: transparent;
		color: var(--bone-100);
		cursor: pointer;
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft),
			box-shadow var(--dur-fast) var(--ease-out-soft);
	}
	.ep-pager-mini-btn svg {
		transition: transform var(--dur-med) var(--ease-out-elastic);
	}
	.ep-pager-mini-btn:hover:not(:disabled) {
		color: var(--accent);
		border-color: color-mix(in oklab, var(--accent) 80%, transparent);
		background: color-mix(in oklab, var(--accent) 10%, transparent);
		box-shadow: 0 0 12px color-mix(in oklab, var(--accent) 30%, transparent);
	}
	.ep-pager-mini-btn:first-child:hover:not(:disabled) svg {
		transform: translateX(-2px);
	}
	.ep-pager-mini-btn:last-child:hover:not(:disabled) svg {
		transform: translateX(2px);
	}
	.ep-pager-mini-btn:disabled {
		color: var(--bone-400);
		border-color: color-mix(in oklab, var(--bone-100) 8%, transparent);
		cursor: not-allowed;
	}

	/* Horizontal scrolling episode strip — single row that slides
	   sideways. Each card is a fixed width so the row reads as a
	   strip of tiles, not a wrap-grid. Snap-stop on each card so
	   a flick lands cleanly. */
	/* Single fluid row of 5 cards. `repeat(5, 1fr)` keeps the count
	   fixed regardless of viewport; cards shrink/grow with the
	   container so all 5 always fit. Block padding gives breathing
	   room so the active card's accent ring isn't shaved. */
	.ep-row {
		list-style: none;
		margin: 0;
		/* Zero ALL padding (the user agent's default
		   `padding-inline-start: 40px` on <ol> was pushing the first
		   card right while the last sat flush with the row's right
		   edge). Block-axis padding is restored selectively below
		   for the active card's accent ring + hover lift. */
		padding: var(--space-2) 0;
		display: grid;
		grid-template-columns: repeat(5, 1fr);
		gap: var(--space-3);
	}
	.ep-row > li {
		display: block;
		min-inline-size: 0;
	}

	/* Spotlight after a Jump-to-ep submit — same vocabulary as the
	   detail page. Targeted card pulses an accent ring; sibling
	   cards dim to 0.35 with asymmetric timing (fast dim-in, slow
	   restore-out) so the spotlight engages decisively and unwinds
	   gracefully as the highlight ends. Reduced-motion users see a
	   static accent ring with no dim. */
	.ep-row:has(li.ep-highlight) > li:not(.ep-highlight) .ep-card {
		opacity: 0.35;
		transition: opacity 0.4s var(--ease-out-soft);
	}
	.ep-card {
		transition:
			transform var(--dur-fast) var(--ease-out-soft),
			opacity 1.4s var(--ease-out-soft);
	}
	.ep-row > li.ep-highlight .ep-card-thumb {
		box-shadow:
			0 0 0 2px var(--accent),
			0 14px 36px -6px color-mix(in oklab, var(--accent) 55%, transparent);
		animation: ep-jump-pulse 1.6s var(--ease-out-soft) 2;
	}
	@keyframes ep-jump-pulse {
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
		.ep-row > li.ep-highlight .ep-card-thumb {
			animation: none;
		}
		.ep-row:has(li.ep-highlight) > li:not(.ep-highlight) .ep-card {
			opacity: 1;
		}
	}

	.ep-list-empty {
		margin: 0;
		padding: var(--space-4);
		font-family: var(--font-body);
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
		font-family: var(--font-body);
		font-size: var(--type-display-m);
		font-weight: 600;
		font-variant-numeric: tabular-nums lining-nums;
		color: var(--bone-300);
	}

	/* Card foot — sits BELOW the 16:9 thumb. EP number + duration on
	   one row (mono, dense), short title clamped to 2 lines below. No
	   description, no large metadata — the row's job is "what is the
	   next/nearby episode", not "browse the catalogue". */
	.ep-card-foot {
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: var(--space-2) var(--space-2) 0;
	}
	.ep-card-foot-row {
		display: inline-flex;
		align-items: baseline;
		gap: var(--space-2);
		font-family: var(--font-body);
		font-size: 0.6875rem; /* 11px */
		font-weight: 600;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.ep-card-foot-num {
		font-variant-numeric: tabular-nums lining-nums;
	}
	.ep-card-foot-dot {
		color: color-mix(in oklab, var(--bone-100) 24%, transparent);
	}
	.ep-card-foot-len {
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		letter-spacing: 0;
		text-transform: none;
	}
	.ep-card-foot-title {
		font-family: var(--font-body);
		font-size: 0.875rem; /* 14px */
		font-weight: 500;
		line-height: 1.3;
		color: var(--bone-100);
		overflow: hidden;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
	}
	.ep-card-current .ep-card-foot-num {
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

	/* "Now playing" pill on the active episode (top-right corner).
	   The ::before dot is a "live" indicator that pulses opacity +
	   scale; the pill itself breathes via a subtle accent halo so
	   the row reads as actively playing rather than statically
	   labelled. Both animations stop under reduced-motion. */
	.ep-card-thumb-flag {
		position: absolute;
		inset-block-start: var(--space-2);
		inset-inline-end: var(--space-2);
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 4px 10px 4px 8px;
		font-family: var(--font-body);
		font-size: var(--type-micro);
		font-weight: 600;
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-100);
		background: linear-gradient(
			135deg,
			var(--accent),
			color-mix(in oklab, var(--accent) 70%, var(--ink-000))
		);
		border-radius: var(--radius-pill);
		box-shadow:
			0 4px 14px color-mix(in oklab, var(--accent) 50%, transparent),
			0 1px 2px rgb(0 0 0 / 0.4);
		animation: now-playing-breathe 2.4s var(--ease-in-out) infinite;
	}
	.ep-card-thumb-flag::before {
		content: '';
		display: inline-block;
		inline-size: 7px;
		block-size: 7px;
		border-radius: 50%;
		background: var(--bone-100);
		flex: 0 0 auto;
		animation: now-playing-pulse 1.4s var(--ease-in-out) infinite;
	}
	@keyframes now-playing-pulse {
		0%,
		100% {
			opacity: 0.55;
			transform: scale(0.9);
		}
		50% {
			opacity: 1;
			transform: scale(1.15);
		}
	}
	@keyframes now-playing-breathe {
		0%,
		100% {
			box-shadow:
				0 4px 14px color-mix(in oklab, var(--accent) 50%, transparent),
				0 1px 2px rgb(0 0 0 / 0.4),
				0 0 0 0 color-mix(in oklab, var(--accent) 60%, transparent);
		}
		50% {
			box-shadow:
				0 4px 14px color-mix(in oklab, var(--accent) 50%, transparent),
				0 1px 2px rgb(0 0 0 / 0.4),
				0 0 0 6px color-mix(in oklab, var(--accent) 0%, transparent);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.ep-card-thumb-flag,
		.ep-card-thumb-flag::before {
			animation: none;
		}
	}
</style>
