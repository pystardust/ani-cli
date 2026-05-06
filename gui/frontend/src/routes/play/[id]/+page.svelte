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
		imageProxyUrl,
		kitsuAnimeDetail,
		kitsuEpisodes,
		kitsuSearch,
		play,
		settingsGet,
		type Config,
		type KitsuAnimeRef,
		type KitsuEpisode,
		type MediaKind
	} from '$lib/api';
	import { accentFor } from '$lib/design/accent';
	import { buildMediaUrl } from '$lib/play/media-url';
	import { getOrFire, makeKey } from '$lib/play/play-cache';
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
	const accent = $derived(id ? accentFor(id) : 'var(--accent-ink)');

	let detail = $state<KitsuAnimeRef | null>(null);
	let episodes = $state<KitsuEpisode[] | null>(null);
	let similar = $state<KitsuAnimeRef[] | null>(null);
	let config = $state<Config | null>(null);
	let detailError = $state<string | null>(null);
	let playerError = $state<string | null>(null);
	let switchBusy = $state(false);

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
			playerError = `Playback error: ${codeName}${err?.message ? ` (${err.message})` : ''}`;
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
				if (data.fatal) {
					playerError = `Playback error: ${data.type} / ${data.details}`;
				}
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

	onDestroy(teardown);

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

	// Background prefetch: warm episode N+1 as soon as the player
	// mounts so the next-episode click feels instant. Reruns on every
	// episode swap (since episodeNum is in the dep set), so each new
	// landing page warms the next one. Skips when there's no next ep
	// (current == episode_count). The play-cache dedupes across
	// triggers — a click before the prefetch finishes shares the same
	// promise.
	$effect(() => {
		if (!detail || !config || !hasNext) return;
		const title = detail.canonical_title;
		if (!title) return;
		const mode = (config.mode === 'dub' ? 'dub' : 'sub') as 'sub' | 'dub';
		const quality = config.quality ?? 'best';
		const targetEp = episodeNum + 1;
		void getOrFire(makeKey(id, targetEp, mode, quality), () =>
			play({
				title,
				episode: String(targetEp),
				mode,
				quality,
				episode_count: detail?.episode_count ?? null
			})
		).catch(() => {
			/* the prev/next click handler surfaces errors when it fires */
		});
	});

	async function switchToEpisode(targetEp: number) {
		if (!detail || switchBusy) return;
		const title = detail.canonical_title;
		if (!title) return;
		const mode = (config?.mode === 'dub' ? 'dub' : 'sub') as 'sub' | 'dub';
		const quality = config?.quality ?? 'best';
		switchBusy = true;
		playerError = null;
		try {
			// Hits the play-cache: ep+1 was prefetched on mount, so the
			// next-episode click is usually instant.
			const session = await getOrFire(makeKey(id, targetEp, mode, quality), () =>
				play({
					title,
					episode: String(targetEp),
					mode,
					quality,
					episode_count: detail?.episode_count ?? null
				})
			);
			// goto navigates within the same route, so the page doesn't
			// fully unmount — `$effect` above re-fires with the new
			// session, and hls.js swaps source. The target is built
			// from `resolve()` plus a query string; the no-resolve
			// lint rule's pattern matcher only recognises a literal
			// `goto(resolve(...))` call, so we suppress around the call.
			/* eslint-disable svelte/no-navigation-without-resolve */
			void goto(
				resolve('/play/[id]', { id }) +
					`?session=${encodeURIComponent(session.session_id)}` +
					`&episode=${targetEp}&kind=${session.media_kind}`
			);
			/* eslint-enable svelte/no-navigation-without-resolve */
		} catch (e) {
			playerError = describeError(e);
		} finally {
			switchBusy = false;
		}
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

<main class="page" style:--accent={accent}>
	<!-- Header strip: show context + prev/next + episode label. -->
	<header class="player-header">
		<a class="show-link" href={resolve('/anime/[id]', { id })}>
			<span class="show-thumb" aria-hidden="true">
				{#if showThumb}
					<img src={showThumb} alt="" loading="lazy" decoding="async" />
				{/if}
			</span>
			<span class="show-meta">
				<span class="eyebrow">
					<span class="eyebrow-key">Now playing</span>
				</span>
				<span class="show-title">{detail?.canonical_title ?? 'Loading…'}</span>
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
	</header>

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

	<!-- Episode strip: highlights the current episode. Same Strip
	     component the detail page uses, repurposed with episode tiles
	     instead of poster cards. -->
	{#if episodes && episodes.length > 0}
		<Strip eyebrow="Episodes">
			{#each episodes as ep (ep.id)}
				{@const n = ep.number ?? ep.relative_number ?? 0}
				{@const isCurrent = n === episodeNum}
				<button
					type="button"
					class="ep-card"
					class:ep-card-current={isCurrent}
					disabled={switchBusy && !isCurrent}
					onclick={() => onPickEpisode(ep)}
				>
					<span class="ep-card-num">Ep {n}</span>
					<span class="ep-card-title">{ep.canonical_title ?? `Episode ${n}`}</span>
				</button>
			{/each}
		</Strip>
	{/if}

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

<LoadingOverlay visible={switchBusy} />

<style>
	.page {
		display: flex;
		flex-direction: column;
		gap: var(--space-7);
		padding-block: var(--space-6) var(--space-9);
		padding-inline: var(--space-8);
		max-inline-size: 92rem;
		margin-inline: auto;
	}

	.player-header {
		display: flex;
		align-items: center;
		gap: var(--space-7);
		flex-wrap: wrap;
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
		border-radius: var(--radius-2);
		overflow: hidden;
		background: color-mix(in oklab, var(--accent) 18%, var(--ink-100));
	}
	.show-thumb img {
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
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
		font-size: var(--type-h2);
		line-height: 1.1;
		color: var(--bone-100);
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.ep-nav {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		margin-inline-start: auto;
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
		border-radius: var(--radius-3);
		overflow: hidden;
		box-shadow: 0 4px 24px color-mix(in oklab, var(--accent) 18%, transparent);
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
		color: var(--bone-300);
		font-family: var(--font-mono);
		font-size: var(--type-meta);
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
		font-size: var(--type-h1);
		pointer-events: none;
	}

	/* Episode tiles in the strip below the player. Smaller than poster
	   cards and oriented around the episode number. */
	.ep-card {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		padding: var(--space-4);
		min-inline-size: 11rem;
		max-inline-size: 14rem;
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-2);
		background: color-mix(in oklab, var(--ink-050) 60%, transparent);
		color: inherit;
		text-align: start;
		cursor: pointer;
		transition:
			border-color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft);
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
	.ep-card-num {
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		color: var(--accent);
	}
	.ep-card-title {
		font-size: var(--type-body);
		color: var(--bone-200);
		overflow: hidden;
		text-overflow: ellipsis;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
	}
</style>
