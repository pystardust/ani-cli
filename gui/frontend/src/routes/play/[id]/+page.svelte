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
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import Hls from 'hls.js';
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

	const currentEpisodeMeta = $derived.by(() => {
		if (!episodes) return null;
		return episodes.find((e) => (e.number ?? e.relative_number ?? -1) === episodeNum) ?? null;
	});

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

		// Pull episode page covering the current episode so the strip
		// below the player has the right tiles ready.
		const UI_PAGE_SIZE = 12;
		const desiredPage = Math.max(1, Math.ceil(episodeNum / UI_PAGE_SIZE));
		void kitsuEpisodes(id, desiredPage)
			.then((eps) => (episodes = eps))
			.catch(() => (episodes = []));

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
	<!-- Player stage: video on the left, episode sidebar on the right.
	     CSS grid only kicks the two-column layout in at >=1280px;
	     below that the sidebar drops below the video as a horizontal
	     scroll, matching the small-window stack the page used to have. -->
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

			<!-- Show / episode metadata under the video. Title is the
			     primary heading on the page (the topbar already shows
			     the breadcrumb). Synopsis lives here so the user can
			     read context next to what they're watching. -->
			{#if detail}
				<section class="show-info">
					<a
						class="show-info-title-link"
						href={resolve('/anime/[id]', { id })}
						onclick={(e) => {
							e.preventDefault();
							void goto(resolve('/anime/[id]', { id }), { replaceState: true });
						}}
					>
						<h1 class="show-info-title">{detail.canonical_title}</h1>
					</a>
					{#if currentEpisodeMeta?.canonical_title}
						<p class="show-info-ep">
							<span class="show-info-ep-key">Episode {episodeNum}</span>
							<span class="show-info-ep-rule" aria-hidden="true"></span>
							<span class="show-info-ep-title">{currentEpisodeMeta.canonical_title}</span>
						</p>
					{/if}
					<p class="show-info-meta">
						{#if detail.subtype}<span>{detail.subtype.toUpperCase()}</span>{/if}
						{#if detail.start_date}
							<span class="show-info-meta-sep" aria-hidden="true">·</span>
							<span>{detail.start_date.slice(0, 4)}</span>
						{/if}
						{#if detail.episode_count}
							<span class="show-info-meta-sep" aria-hidden="true">·</span>
							<span><span class="num">{detail.episode_count}</span> episodes</span>
						{/if}
						{#if detail.average_rating}
							<span class="show-info-meta-sep" aria-hidden="true">·</span>
							<span class="show-info-rating">★ {(detail.average_rating / 10).toFixed(1)}</span>
						{/if}
					</p>
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

		{#if episodes && episodes.length > 0}
			<aside class="ep-sidebar" aria-label="Episodes">
				<!-- Show identity card. Restored from the previous layout
				     so the cover + title still anchor the page; placed at
				     the top of the sidebar instead of as a header band so
				     the player is the first thing the eye lands on. -->
				{#if detail}
					<a
						class="show-card"
						href={resolve('/anime/[id]', { id })}
						onclick={(e) => {
							e.preventDefault();
							void goto(resolve('/anime/[id]', { id }), { replaceState: true });
						}}
					>
						{#if showThumb}
							<img
								class="show-card-cover"
								src={showThumb}
								alt={`Cover art for ${detail.canonical_title}`}
								loading="lazy"
							/>
						{:else}
							<span class="show-card-cover show-card-cover-placeholder" aria-hidden="true"></span>
						{/if}
						<span class="show-card-text">
							<span class="show-card-eyebrow">Now watching</span>
							<span class="show-card-title">{detail.canonical_title}</span>
							{#if detail.episode_count}
								<span class="show-card-meta">
									<span class="num">{detail.episode_count}</span> episodes
								</span>
							{/if}
						</span>
					</a>
				{/if}

				<h2 class="ep-sidebar-title">
					<span class="eyebrow-key">Episodes</span>
					<span class="eyebrow-rule" aria-hidden="true"></span>
					<span class="eyebrow-value">{episodes.length}</span>
				</h2>
				<ol class="ep-list">
					{#each episodes as ep (ep.id)}
						{@const n = ep.number ?? ep.relative_number ?? 0}
						{@const isCurrent = n === episodeNum}
						{@const epThumb = imageProxyUrl(ep.thumbnail?.original ?? null)}
						<li>
							<button
								type="button"
								class="ep-card"
								class:ep-card-current={isCurrent}
								disabled={switchBusy && !isCurrent}
								onclick={() => onPickEpisode(ep)}
							>
								<span class="ep-card-thumb" aria-hidden="true">
									{#if epThumb}
										<img src={epThumb} alt="" loading="lazy" />
									{/if}
									<span class="ep-card-thumb-num">{n}</span>
								</span>
								<span class="ep-card-text">
									<span class="ep-card-num">Ep {n}</span>
									<span class="ep-card-title">{ep.canonical_title ?? `Episode ${n}`}</span>
									{#if ep.length || ep.airdate}
										<span class="ep-card-meta">
											{#if ep.length}<span>{ep.length}m</span>{/if}
											{#if ep.length && ep.airdate}
												<span class="ep-card-meta-sep" aria-hidden="true">·</span>
											{/if}
											{#if ep.airdate}<span>{ep.airdate}</span>{/if}
										</span>
									{/if}
								</span>
							</button>
						</li>
					{/each}
				</ol>
			</aside>
		{/if}
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
		gap: var(--space-7);
		padding-block: var(--space-6) var(--space-9);
		padding-inline: var(--space-8);
		max-inline-size: var(--content-max-wide);
		margin-inline: auto;
	}

	/* Theater mode: hide the sidebar, drop the page's max-width and
	   inline padding so the player consumes the whole window. The
	   show-info section stays at editorial width below the video so
	   the prose doesn't run off the screen. */
	.page.theater {
		max-inline-size: none;
		padding-inline: 0;
		gap: var(--space-5);
	}
	.page.theater .player-controls {
		padding-inline: var(--space-8);
	}
	.page.theater .ep-sidebar {
		display: none;
	}
	.page.theater .show-info {
		padding-inline: var(--space-8);
	}
	.page.theater .player-frame {
		border-radius: 0;
		box-shadow: none;
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
	}
	.theater-toggle.theater-on {
		border-color: var(--accent);
		color: var(--bone-100);
		background: color-mix(in oklab, var(--accent) 14%, var(--ink-050));
	}

	/* Two-column stage on wide screens: video + show info on the
	   left, episode list pinned to the right. Single column below
	   1280px so cramped layouts stack instead of crowding. */
	.player-stage {
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
	}
	.player-column {
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
		min-inline-size: 0;
	}
	@media (min-inline-size: 1280px) {
		.player-stage {
			display: grid;
			/* Wider sidebar — the page no longer caps at the player's
			   right edge, so the sidebar can spread into real estate
			   the previous layout left empty. clamp at 24rem floor /
			   34rem ceiling keeps ep cards readable across screens. */
			grid-template-columns: minmax(0, 1fr) clamp(24rem, 30vw, 34rem);
			gap: var(--space-7);
			align-items: start;
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
	@media (min-inline-size: 1280px) {
		.ep-sidebar {
			position: sticky;
			inset-block-start: var(--space-7);
			max-block-size: calc(100dvh - 8rem);
		}
	}
	.ep-sidebar-title {
		display: inline-flex;
		align-items: center;
		gap: var(--space-3);
		margin: 0;
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		color: var(--bone-300);
		text-transform: uppercase;
		letter-spacing: 0.06em;
		font-weight: 500;
	}
	.ep-sidebar-title .eyebrow-key {
		color: var(--accent);
	}
	.ep-sidebar-title .eyebrow-rule {
		flex: 1;
		block-size: 1px;
		background: var(--ink-300);
	}
	.ep-sidebar-title .eyebrow-value {
		color: var(--bone-200);
		font-variant-numeric: tabular-nums lining-nums;
	}
	.ep-list {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		list-style: none;
		margin: 0;
		padding: 0;
		min-inline-size: 0;
	}
	@media (min-inline-size: 1280px) {
		.ep-list {
			overflow-y: auto;
			padding-inline-end: var(--space-2);
		}
	}
	@media (max-inline-size: 1279px) {
		.ep-list {
			flex-direction: row;
			overflow-x: auto;
			overflow-y: visible;
			scroll-snap-type: x mandatory;
			padding-block-end: var(--space-2);
		}
		.ep-list li {
			scroll-snap-align: start;
			flex: 0 0 auto;
		}
	}
	.ep-list li {
		display: block;
		min-inline-size: 0;
	}
	@media (max-inline-size: 1279px) {
		.ep-list li {
			min-inline-size: 11rem;
		}
	}

	/* Show + episode metadata under the video. The show title is
	   the page's H1 — the topbar already renders the breadcrumb,
	   so there's no second copy of the same text. */
	.show-info {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		max-inline-size: 60ch;
	}
	.show-info-title-link {
		text-decoration: none;
		color: inherit;
		display: inline-block;
	}
	.show-info-title {
		margin: 0;
		font-family: var(--font-display);
		font-style: italic;
		font-size: var(--type-display-l);
		line-height: 1.05;
		color: var(--bone-100);
	}
	.show-info-title-link:hover .show-info-title {
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
		letter-spacing: 0.06em;
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
	.show-info-meta {
		display: inline-flex;
		align-items: baseline;
		gap: var(--space-2);
		margin: 0;
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		color: var(--bone-300);
		text-transform: uppercase;
		letter-spacing: 0.06em;
	}
	.show-info-meta .num {
		color: var(--bone-100);
		font-variant-numeric: tabular-nums lining-nums;
	}
	.show-info-meta-sep {
		color: var(--bone-400);
	}
	.show-info-rating {
		color: var(--accent);
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

	/* Show identity card at the top of the sidebar — restored from
	   the previous layout's player-header. Cover poster + title +
	   ep count, the whole thing is a link to the detail page. */
	.show-card {
		display: grid;
		grid-template-columns: 4.5rem 1fr;
		gap: var(--space-4);
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
	.show-card:hover {
		border-color: var(--accent);
		background: color-mix(in oklab, var(--accent) 10%, var(--ink-050));
	}
	.show-card-cover {
		display: block;
		inline-size: 100%;
		aspect-ratio: 5 / 7;
		object-fit: cover;
		border-radius: var(--radius-control);
		background: var(--ink-100);
	}
	.show-card-cover-placeholder {
		background: linear-gradient(135deg, var(--ink-100), var(--ink-200));
	}
	.show-card-text {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		min-inline-size: 0;
	}
	.show-card-eyebrow {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--accent);
	}
	.show-card-title {
		font-family: var(--font-display);
		font-style: italic;
		font-size: var(--type-display-m);
		line-height: 1.15;
		color: var(--bone-100);
		overflow: hidden;
		text-overflow: ellipsis;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
	}
	.show-card-meta {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-meta);
		color: var(--bone-300);
		text-transform: uppercase;
	}
	.show-card-meta .num {
		color: var(--bone-100);
		font-variant-numeric: tabular-nums lining-nums;
	}

	/* Horizontal episode card: thumbnail (16:9) on the left, ep
	   number + title + meta on the right. Reads like the
	   YouTube/Crunchyroll watch-next list — recognition by image
	   first, scan by title second. The thumb shows the ep number
	   in the corner so even the placeholder (no thumb) still
	   identifies the row. */
	.ep-card {
		display: grid;
		grid-template-columns: 7rem 1fr;
		gap: var(--space-3);
		align-items: stretch;
		padding: var(--space-2);
		inline-size: 100%;
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-card);
		background: color-mix(in oklab, var(--ink-050) 60%, transparent);
		color: inherit;
		text-align: start;
		cursor: pointer;
		transition:
			border-color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft);
	}
	@media (max-inline-size: 1279px) {
		/* Under 1280px the sidebar reverts to a horizontal scroll
		   strip; cap card width so each tile keeps its shape. */
		.ep-card {
			min-inline-size: 18rem;
			max-inline-size: 22rem;
		}
	}
	.ep-card:hover:not(:disabled) {
		border-color: var(--accent);
	}
	.ep-card:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
	.ep-card-current {
		border-color: var(--accent);
		background: color-mix(in oklab, var(--accent) 14%, var(--ink-050));
	}
	.ep-card-thumb {
		position: relative;
		display: block;
		aspect-ratio: 16 / 9;
		background: linear-gradient(135deg, var(--ink-100), var(--ink-200));
		border-radius: var(--radius-control);
		overflow: hidden;
	}
	.ep-card-thumb img {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
	}
	.ep-card-thumb-num {
		position: absolute;
		inset-block-end: 0.25rem;
		inset-inline-start: 0.4rem;
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		font-variant-numeric: tabular-nums lining-nums;
		color: var(--bone-100);
		text-shadow: 0 1px 3px rgb(0 0 0 / 0.7);
		letter-spacing: 0.04em;
	}
	.ep-card-text {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-inline-size: 0;
		padding-block: var(--space-1);
	}
	.ep-card-num {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-meta);
		text-transform: uppercase;
		color: var(--accent);
	}
	.ep-card-title {
		font-size: var(--type-body);
		color: var(--bone-100);
		line-height: 1.25;
		overflow: hidden;
		text-overflow: ellipsis;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
	}
	.ep-card-meta {
		display: inline-flex;
		align-items: baseline;
		gap: var(--space-2);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		color: var(--bone-300);
		margin-block-start: 2px;
	}
	.ep-card-meta-sep {
		color: var(--bone-400);
	}
</style>
