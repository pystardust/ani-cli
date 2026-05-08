<!--
  Root layout — the app shell.
  - Imports tokens.css globally (hotfix from M3.1: ensures the warm-ink
    baseline applies on /, not just on routes that imported it themselves).
  - Persistent narrow left rail with home / search / continue / settings
    / diagnostics. Active item gets a 2px accent rule and bone-100 type;
    everything else is hairlined and quiet.
  - Sticky glassy topbar across all routes: BackButton on the left
    (auto-hides when there's no history to go back to), persistent
    search input on the right. Pressing Enter routes to /search?q=…
    so the field works from any page. Removes per-route topbars whose
    BackButtons jumped around between pages.
  - Wires the favicon via <svelte:head>.
  - Same chrome (rail + topbar) applies everywhere, including /play, so
    navigation is consistent (the player page used to be full-bleed,
    which left users without a way to search or jump back to home).
-->
<script lang="ts">
	import '$lib/design/tokens.css';
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { afterNavigate, goto } from '$app/navigation';
	import Icon from '$lib/components/Icon.svelte';
	import Breadcrumb from '$lib/components/Breadcrumb.svelte';
	import { breadcrumb, defaultTrailFor } from '$lib/breadcrumb';
	import { imageProxyUrl, kitsuSearch, type KitsuAnimeRef } from '$lib/api';
	import { nextDepth, shouldShowBackButton, type NavType } from '$lib/history/nav-depth';
	import {
		RECENT_LIMIT,
		RECENT_STORAGE_KEY,
		cycleSelectedIdx,
		decideEnterAction,
		mergeRecents,
		parseStoredRecents,
		shouldRenderDropdown
	} from '$lib/topbar/dropdown';

	let { children } = $props();

	// Routes where the chrome should yield to content.
	// Use the route id (e.g. "/", "/search", "/anime/[id]") instead of the
	// raw pathname — the static adapter sometimes serves "/index.html" on
	// the very first paint, which made the Home rail item not light up
	// until the user clicked it. route.id always resolves to the matched
	// route pattern, including for the home page.
	const routeId = $derived<string>(page.route?.id ?? page.url.pathname);
	const isHome = $derived(routeId === '/');
	const isSearch = $derived(routeId.startsWith('/search'));
	const isSettings = $derived(routeId.startsWith('/settings'));
	const isDiagnostics = $derived(routeId.startsWith('/diagnostics'));

	// Back-stack depth tracker. Layout adapter; the rules live in
	// `$lib/history/nav-depth` so they're unit-testable. We pull
	// type + stamped depth out of the SvelteKit event, hand them to
	// nextDepth, and stamp the result back on forward navs so
	// popstate can read it later.
	let canGoBack = $state(false);
	let navDepth = 0;

	afterNavigate(({ type }) => {
		if (typeof window === 'undefined') return;
		const stamped = (window.history.state as { aniGuiDepth?: number } | null)?.aniGuiDepth;
		navDepth = nextDepth({
			type: type as NavType,
			stampedDepth: typeof stamped === 'number' ? stamped : null,
			prevDepth: navDepth
		});
		// Forward navs need their depth stamped onto the new history
		// entry so popstate can recover it. enter / popstate /
		// leave / replaceState don't push a new entry — no stamp needed.
		if (type === 'goto' || type === 'link' || type === 'form') {
			try {
				window.history.replaceState({ ...window.history.state, aniGuiDepth: navDepth }, '');
			} catch {
				// replaceState can throw in privacy modes; non-fatal.
			}
		}
		canGoBack = shouldShowBackButton(navDepth);
		// Reset the breadcrumb to a default URL-only trail on every
		// navigation. Routes with richer labels (anime title, episode
		// number) overwrite this in onMount once their data lands.
		breadcrumb.set(defaultTrailFor(page.route?.id ?? null));
	});

	let topbarQuery = $state('');
	let topbarInputEl: HTMLInputElement | undefined = $state();

	// — Live-results dropdown + recent searches + Cmd/Ctrl+K. —————————
	// Bundled together so a single `git revert HEAD` rolls them back if
	// they prove distracting. Each piece is small in isolation; the
	// volume of code here is mostly the dropdown UI.

	const LIVE_DEBOUNCE_MS = 250;
	const LIVE_MIN_CHARS = 2;
	const LIVE_MAX_HITS = 5;
	// RECENT_LIMIT + RECENT_STORAGE_KEY come from $lib/topbar/dropdown
	// so the constants live next to the helpers that consume them.

	let liveResults = $state<KitsuAnimeRef[] | null>(null);
	let liveBusy = $state(false);
	let liveError = $state(false);
	let dropdownOpen = $state(false);
	let selectedIdx = $state(-1);
	let recentSearches = $state<string[]>([]);
	let liveDebounce: ReturnType<typeof setTimeout> | null = null;
	let blurDismiss: ReturnType<typeof setTimeout> | null = null;

	onMount(() => {
		try {
			recentSearches = parseStoredRecents(window.localStorage.getItem(RECENT_STORAGE_KEY));
		} catch {
			// localStorage unavailable — leave recentSearches empty.
		}
	});

	function persistRecents(q: string) {
		const next = mergeRecents(recentSearches, q, RECENT_LIMIT);
		recentSearches = next;
		try {
			window.localStorage.setItem(RECENT_STORAGE_KEY, JSON.stringify(next));
		} catch {
			// Quota / disabled — accept; in-memory state still updates.
		}
	}

	function scheduleLive(q: string) {
		if (liveDebounce) clearTimeout(liveDebounce);
		if (q.length < LIVE_MIN_CHARS) {
			liveResults = null;
			liveBusy = false;
			liveError = false;
			return;
		}
		liveDebounce = setTimeout(async () => {
			liveBusy = true;
			liveError = false;
			try {
				const hits = await kitsuSearch(q);
				// If the user kept typing past this query, ignore stale results.
				if (q !== topbarQuery.trim()) return;
				liveResults = hits.slice(0, LIVE_MAX_HITS);
			} catch {
				liveResults = [];
				liveError = true;
			} finally {
				liveBusy = false;
			}
		}, LIVE_DEBOUNCE_MS);
	}

	// Sync the topbar input with ?q= so navigating to /search shows
	// what was searched and the field stays editable.
	$effect(() => {
		if (isSearch) {
			topbarQuery = page.url.searchParams.get('q') ?? '';
		}
	});

	// "/" or Cmd/Ctrl+K focuses the topbar search from anywhere; Esc
	// blurs it. Skip "/" when the user is already typing in another
	// field — Cmd/Ctrl+K still wins because it's a deliberate chord.
	$effect(() => {
		if (typeof window === 'undefined') return;
		const onKey = (e: KeyboardEvent) => {
			const t = e.target as HTMLElement | null;
			const inField =
				t &&
				(t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || (t as HTMLElement).isContentEditable);
			const isCmdK = (e.ctrlKey || e.metaKey) && (e.key === 'k' || e.key === 'K');
			if (isCmdK) {
				e.preventDefault();
				topbarInputEl?.focus();
				topbarInputEl?.select();
				dropdownOpen = true;
				return;
			}
			if (e.key === '/' && !inField) {
				e.preventDefault();
				topbarInputEl?.focus();
				topbarInputEl?.select();
				dropdownOpen = true;
			} else if (e.key === 'Escape' && document.activeElement === topbarInputEl) {
				topbarInputEl?.blur();
				dropdownOpen = false;
			}
		};
		window.addEventListener('keydown', onKey);
		return () => window.removeEventListener('keydown', onKey);
	});

	function onInput() {
		selectedIdx = -1;
		scheduleLive(topbarQuery.trim());
	}

	function onInputFocus() {
		if (blurDismiss) {
			clearTimeout(blurDismiss);
			blurDismiss = null;
		}
		dropdownOpen = true;
	}

	function onInputBlur() {
		// Defer dismissal so a click on a dropdown row registers before
		// the dropdown unmounts. 160ms is comfortably more than the
		// time between mousedown → click in modern browsers.
		blurDismiss = setTimeout(() => {
			dropdownOpen = false;
			selectedIdx = -1;
		}, 160);
	}

	function onInputKey(e: KeyboardEvent) {
		const items = liveResults ?? [];
		if (e.key === 'ArrowDown' && items.length > 0) {
			e.preventDefault();
			selectedIdx = cycleSelectedIdx(selectedIdx, 1, items.length);
		} else if (e.key === 'ArrowUp' && items.length > 0) {
			e.preventDefault();
			selectedIdx = cycleSelectedIdx(selectedIdx, -1, items.length);
		} else if (e.key === 'Enter' && selectedIdx >= 0 && items[selectedIdx]) {
			e.preventDefault();
			navigateToHit(items[selectedIdx]);
		}
	}

	function navigateToHit(hit: KitsuAnimeRef) {
		dropdownOpen = false;
		void goto(resolve('/anime/[id]', { id: hit.id }));
	}

	function navigateToSearch(q: string) {
		const target = new URL(resolve('/search'), window.location.origin);
		target.searchParams.set('q', q);
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		void goto(target);
		dropdownOpen = false;
	}

	function onTopbarSubmit(e: SubmitEvent) {
		e.preventDefault();
		const items = liveResults ?? [];
		const action = decideEnterAction(selectedIdx, items.length, topbarQuery);
		if (action.type === 'navigate-to-hit') {
			navigateToHit(items[action.idx]);
		} else if (action.type === 'submit-query') {
			const q = topbarQuery.trim();
			persistRecents(q);
			navigateToSearch(q);
		}
		// 'noop' — empty input + no selection.
	}

	function onRecentClick(q: string) {
		topbarQuery = q;
		persistRecents(q);
		navigateToSearch(q);
	}

	function hitPoster(hit: KitsuAnimeRef): string | null {
		const url = hit.poster_image?.small ?? hit.poster_image?.medium ?? null;
		return imageProxyUrl(url);
	}
	function hitMeta(hit: KitsuAnimeRef): string {
		const year = hit.start_date ? hit.start_date.slice(0, 4) : null;
		const subtype = (hit.subtype ?? 'TV').toUpperCase();
		return year ? `${year} · ${subtype}` : subtype;
	}
</script>

<svelte:head>
	<link rel="icon" type="image/svg+xml" href="/favicon.svg" />
</svelte:head>

<div class="shell">
	<aside class="rail" aria-label="Primary navigation">
		<a class="brand" href={resolve('/')} aria-label="ani-gui — home">
			<svg
				class="brand-mark"
				viewBox="0 0 32 32"
				width="40"
				height="40"
				fill="none"
				aria-hidden="true"
			>
				<!-- Filled brand-color tile reads as a logo, not another
					     hairline nav item. Inner perforation pattern in the
					     ink keeps the filmstrip motif visible. -->
				<rect x="2" y="2" width="28" height="28" rx="6" fill="var(--brand)" />
				<rect x="6" y="6" width="2.5" height="2.5" rx="0.5" fill="var(--brand-ink)" />
				<rect x="6" y="11" width="2.5" height="2.5" rx="0.5" fill="var(--brand-ink)" />
				<rect x="6" y="16" width="2.5" height="2.5" rx="0.5" fill="var(--brand-ink)" />
				<rect x="6" y="21" width="2.5" height="2.5" rx="0.5" fill="var(--brand-ink)" />
				<rect x="23.5" y="6" width="2.5" height="2.5" rx="0.5" fill="var(--brand-ink)" />
				<rect x="23.5" y="11" width="2.5" height="2.5" rx="0.5" fill="var(--brand-ink)" />
				<rect x="23.5" y="16" width="2.5" height="2.5" rx="0.5" fill="var(--brand-ink)" />
				<rect x="23.5" y="21" width="2.5" height="2.5" rx="0.5" fill="var(--brand-ink)" />
			</svg>
			<span class="brand-word" aria-hidden="true">
				<span class="brand-word-italic">ani</span><span class="brand-word-dot">·</span><span
					class="brand-word-italic">gui</span
				>
			</span>
		</a>

		<nav class="nav">
			<ul>
				<li>
					<a
						href={resolve('/')}
						class="nav-link"
						class:active={isHome}
						aria-current={isHome ? 'page' : undefined}
					>
						<span class="nav-mark"><Icon name="home" size={20} /></span>
						<span class="nav-label">Home</span>
					</a>
				</li>
				<li>
					<a
						href={resolve('/search')}
						class="nav-link"
						class:active={isSearch}
						aria-current={isSearch ? 'page' : undefined}
					>
						<span class="nav-mark"><Icon name="search" size={20} /></span>
						<span class="nav-label">Search</span>
					</a>
				</li>
				<li>
					<a
						href={resolve('/settings')}
						class="nav-link"
						class:active={isSettings}
						aria-current={isSettings ? 'page' : undefined}
					>
						<span class="nav-mark"><Icon name="settings" size={20} /></span>
						<span class="nav-label">Settings</span>
					</a>
				</li>
				<li class="small">
					<a
						href={resolve('/diagnostics')}
						class="nav-link"
						class:active={isDiagnostics}
						aria-current={isDiagnostics ? 'page' : undefined}
					>
						<span class="nav-mark"><Icon name="debug" size={18} /></span>
						<span class="nav-label">Debug</span>
					</a>
				</li>
			</ul>
		</nav>

		<footer class="rail-foot" aria-hidden="true">
			<span class="rail-foot-key">v</span>
			<span class="rail-foot-val">0.1</span>
		</footer>
	</aside>

	<div class="main-area">
		<header class="topbar">
			{#if canGoBack}
				<Breadcrumb segments={$breadcrumb} />
			{/if}
			<form
				class="topbar-search"
				class:topbar-search-filled={topbarQuery.trim().length > 0}
				onsubmit={onTopbarSubmit}
				role="search"
			>
				<span class="topbar-search-icon" aria-hidden="true">
					<Icon name="search" size={18} />
				</span>
				<input
					bind:this={topbarInputEl}
					bind:value={topbarQuery}
					type="search"
					autocomplete="off"
					spellcheck="false"
					placeholder={isSearch ? 'Refine your search…' : 'Search anime…'}
					aria-label="Search anime"
					oninput={onInput}
					onfocus={onInputFocus}
					onblur={onInputBlur}
					onkeydown={onInputKey}
				/>
				<span class="topbar-search-hint" aria-hidden="true">
					{#if liveBusy}
						<span class="topbar-search-busy">…</span>
					{:else}
						<kbd>/</kbd>
					{/if}
				</span>
				<!-- Submit on Enter; the explicit button is sr-only for a11y. -->
				<button type="submit" class="sr-only" disabled={topbarQuery.trim().length === 0}>
					Search
				</button>

				{#if shouldRenderDropdown({ dropdownOpen, liveResults, liveError, queryTrimmed: topbarQuery.trim(), recentsCount: recentSearches.length }, { liveMinChars: LIVE_MIN_CHARS })}
					<div class="topbar-dropdown" role="listbox" aria-label="Search suggestions">
						{#if liveResults && liveResults.length > 0}
							{#each liveResults as hit, i (hit.id)}
								{@const poster = hitPoster(hit)}
								<a
									class="topbar-hit"
									class:selected={i === selectedIdx}
									href={resolve('/anime/[id]', { id: hit.id })}
									role="option"
									aria-selected={i === selectedIdx}
									onmousedown={(e) => {
										// mousedown fires before blur, so this handler
										// runs while the dropdown is still open.
										e.preventDefault();
										navigateToHit(hit);
									}}
								>
									<span class="topbar-hit-poster">
										{#if poster}
											<img src={poster} alt="" loading="lazy" decoding="async" />
										{/if}
									</span>
									<span class="topbar-hit-text">
										<span class="topbar-hit-title">{hit.canonical_title}</span>
										<span class="topbar-hit-meta">{hitMeta(hit)}</span>
									</span>
								</a>
							{/each}
						{:else if liveError}
							<p class="topbar-dropdown-empty">Couldn't reach Kitsu.</p>
						{:else if liveResults?.length === 0 && topbarQuery.trim().length >= LIVE_MIN_CHARS}
							<p class="topbar-dropdown-empty">No matches.</p>
						{:else if !topbarQuery.trim() && recentSearches.length > 0}
							<p class="topbar-dropdown-section">Recent</p>
							{#each recentSearches as q (q)}
								<button
									type="button"
									class="topbar-recent"
									onmousedown={(e) => {
										e.preventDefault();
										onRecentClick(q);
									}}
								>
									<span aria-hidden="true">↩</span>
									<span>{q}</span>
								</button>
							{/each}
						{/if}
					</div>
				{/if}
			</form>
		</header>
		<main class="content">
			{@render children()}
		</main>
	</div>
</div>

<style>
	.shell {
		display: grid;
		grid-template-columns: var(--rail-width) 1fr;
		min-block-size: 100dvh;
	}
	@media (max-inline-size: 720px) {
		.shell {
			grid-template-columns: 1fr;
		}
	}

	/* The right column hosts the topbar above the routed content.
	   Flex column so the topbar sits at the top of the area and the
	   <main> fills below; the topbar's own sticky positioning keeps
	   it pinned against the viewport on scroll. */
	.main-area {
		display: flex;
		flex-direction: column;
		min-inline-size: 0; /* prevent overflow from blowing out the grid */
	}

	.rail {
		grid-row: 1;
		position: sticky;
		inset-block-start: 0;
		block-size: 100dvh;
		display: flex;
		flex-direction: column;
		align-items: stretch;
		padding-block: var(--space-5) var(--space-4);
		border-inline-end: 1px solid var(--ink-200);
		background: var(--ink-000);
		z-index: 20;
	}
	@media (max-inline-size: 720px) {
		.rail {
			position: relative;
			block-size: auto;
			flex-direction: row;
			align-items: center;
			padding: var(--space-3) var(--space-4);
			border-inline-end: 0;
			border-block-end: 1px solid var(--ink-200);
			gap: var(--space-4);
		}
	}

	/* — Brand mark in rail. */
	.brand {
		display: grid;
		justify-items: center;
		gap: var(--space-2);
		padding-block-end: var(--space-5);
		margin-block-end: var(--space-5);
		border-block-end: 1px solid var(--ink-200);
	}
	@media (max-inline-size: 720px) {
		.brand {
			padding: 0;
			margin: 0;
			border: 0;
			grid-auto-flow: column;
			align-items: center;
			gap: var(--space-3);
		}
	}
	.brand-mark {
		display: block;
		inline-size: 2.5rem;
		block-size: 2.5rem;
		filter: drop-shadow(0 4px 8px rgb(0 0 0 / 0.4));
	}
	.brand-word {
		display: inline-flex;
		align-items: baseline;
		gap: 2px;
		font-family: var(--font-display);
		font-size: var(--type-meta);
		color: var(--bone-200);
	}
	.brand-word-italic {
		font-style: italic;
	}
	.brand-word-dot {
		font-family: var(--font-mono);
		color: var(--bone-400);
		font-style: normal;
	}

	/* — Nav. */
	.nav {
		flex: 1;
	}
	.nav ul {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}
	@media (max-inline-size: 720px) {
		.nav ul {
			flex-direction: row;
			gap: var(--space-3);
			flex-wrap: wrap;
		}
	}
	.nav li.small .nav-link {
		opacity: 0.6;
	}
	.nav li.small .nav-link:hover,
	.nav li.small .nav-link.active {
		opacity: 1;
	}

	.nav-link {
		position: relative;
		isolation: isolate;
		display: grid;
		justify-items: center;
		gap: var(--space-1);
		padding-block: var(--space-3);
		padding-inline: var(--space-2);
		color: var(--bone-300);
		transition: color var(--dur-fast) var(--ease-out-soft);
	}
	@media (max-inline-size: 720px) {
		.nav-link {
			grid-auto-flow: column;
			align-items: center;
			padding-block: var(--space-2);
		}
	}
	.nav-link:hover {
		color: var(--bone-100);
	}
	.nav-link.active {
		color: var(--bone-100);
	}
	.nav-link.active::before {
		/* 2px accent rule on the inline-start edge marks the active item */
		content: '';
		position: absolute;
		inset-block: var(--space-2);
		inset-inline-start: 0;
		inline-size: 2px;
		background: var(--accent);
		z-index: 1;
	}
	@media (max-inline-size: 720px) {
		.nav-link.active::before {
			inset-block-end: 0;
			inset-block-start: auto;
			inset-inline: var(--space-2);
			inline-size: auto;
			block-size: 2px;
		}
	}

	/* Brand-tinted LED bloom centered behind the icon. Same idea as the
	   home hero glass button: a soft radial glow that pulses up on
	   hover (and stays lit on the active item). The rail itself is
	   opaque, so there's no glass blur — just the bloom. */
	.nav-link::after {
		content: '';
		position: absolute;
		inset: 10% 15%;
		background: radial-gradient(
			ellipse 70% 70% at 50% 50%,
			var(--brand) 0%,
			color-mix(in oklab, var(--brand) 50%, transparent) 35%,
			transparent 72%
		);
		opacity: 0;
		filter: blur(10px);
		transform: scale(0.7);
		transition:
			opacity var(--dur-med) var(--ease-out-soft),
			transform var(--dur-med) var(--ease-out-elastic);
		z-index: -1;
		pointer-events: none;
	}
	.nav-link:hover::after {
		opacity: 0.9;
		transform: scale(1.05);
	}
	.nav-link.active::after {
		/* Active item stays lit, slightly dimmer than hover so the
		   accent rule still wins as the primary "you-are-here" cue. */
		opacity: 0.7;
		transform: scale(1);
	}
	@media (prefers-reduced-motion: reduce) {
		.nav-link::after {
			transform: none;
			transition: opacity var(--dur-fast) linear;
		}
		.nav-link:hover::after,
		.nav-link.active::after {
			transform: none;
		}
	}

	.nav-mark {
		position: relative;
		z-index: 1;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		color: inherit;
		line-height: 0;
	}
	.nav-label {
		position: relative;
		z-index: 1;
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
	}

	.rail-foot {
		display: grid;
		justify-items: center;
		gap: 2px;
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		color: var(--bone-400);
		text-transform: uppercase;
	}
	@media (max-inline-size: 720px) {
		.rail-foot {
			margin-inline-start: auto;
			grid-auto-flow: column;
			gap: var(--space-1);
		}
	}
	.rail-foot-val {
		font-variant-numeric: tabular-nums lining-nums;
		color: var(--bone-300);
	}

	.content {
		min-inline-size: 0; /* prevent grid from blowing wide on long titles */
	}

	/* — Global topbar. Sticky against the viewport, glassy-translucent
	     so it reads as an overlay rather than a solid header. Same
	     gutter as the strip rail (--space-8) so the BackButton lines
	     up with the leading edge of every page's content. */
	.topbar {
		position: sticky;
		inset-block-start: 0;
		z-index: 15;
		display: flex;
		align-items: center;
		gap: var(--space-5);
		padding: var(--space-3) var(--space-8);
		background: color-mix(in oklab, var(--ink-000) 65%, transparent);
		backdrop-filter: blur(16px) saturate(1.3);
		-webkit-backdrop-filter: blur(16px) saturate(1.3);
		border-block-end: 1px solid color-mix(in oklab, var(--ink-200) 80%, transparent);
	}
	@media (max-inline-size: 720px) {
		.topbar {
			padding-inline: var(--space-4);
		}
	}

	/* Persistent search input. Pill-rounded to match /search; capped
	   so it doesn't dominate on widescreen, and pushed toward the
	   trailing edge so the BackButton stays anchored at the leading
	   edge regardless of the input's width. */
	.topbar-search {
		position: relative; /* dropdown anchors absolute to this */
		display: flex;
		align-items: center;
		gap: var(--space-3);
		flex: 0 1 28rem;
		margin-inline-start: auto;
		padding: var(--space-2) var(--space-4);
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-pill);
		background: color-mix(in oklab, var(--ink-050) 70%, transparent);
		transition:
			border-color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft),
			box-shadow var(--dur-fast) var(--ease-out-soft);
	}
	.topbar-search:focus-within {
		border-color: var(--bone-200);
		background: color-mix(in oklab, var(--ink-050) 90%, transparent);
		box-shadow: 0 0 0 3px color-mix(in oklab, var(--accent) 22%, transparent);
	}
	.topbar-search-filled {
		border-color: color-mix(in oklab, var(--bone-200) 70%, var(--ink-300));
	}
	.topbar-search-icon {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		color: var(--bone-300);
		transition: color var(--dur-fast) var(--ease-out-soft);
	}
	.topbar-search:focus-within .topbar-search-icon {
		color: var(--bone-100);
	}
	.topbar-search input {
		flex: 1;
		min-inline-size: 0;
		padding: 0;
		background: transparent;
		border: 0;
		outline: none;
		/* `appearance: none` strips the UA styling on `<input type="search">`,
		   which on Chromium otherwise paints an inset focus rectangle that
		   our outer wrapper's box-shadow ring already covers. */
		appearance: none;
		-webkit-appearance: none;
		-moz-appearance: none;
		box-shadow: none;
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		color: var(--bone-100);
	}
	.topbar-search input:focus,
	.topbar-search input:focus-visible {
		outline: none;
		box-shadow: none;
	}
	/* Webkit/Chromium type="search" pseudo-elements that can render
	   their own visuals on focus — kill all of them. */
	.topbar-search input::-webkit-search-decoration,
	.topbar-search input::-webkit-search-results-button,
	.topbar-search input::-webkit-search-results-decoration {
		display: none;
	}
	.topbar-search input::placeholder {
		color: var(--bone-400);
	}
	.topbar-search input::-webkit-search-cancel-button {
		appearance: none;
	}
	.topbar-search-hint {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-inline-size: 1.5rem;
	}
	.topbar-search-hint kbd {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		color: var(--bone-300);
		padding: 0 var(--space-2);
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-control);
	}
	.topbar-search:focus-within .topbar-search-hint kbd {
		opacity: 0;
	}
	.sr-only {
		position: absolute;
		inline-size: 1px;
		block-size: 1px;
		padding: 0;
		margin: -1px;
		overflow: hidden;
		clip: rect(0, 0, 0, 0);
		white-space: nowrap;
		border: 0;
	}

	/* Live-results / recent-searches dropdown. Sits below the topbar
	   pill; opaque enough to read against busy hero backgrounds. */
	.topbar-dropdown {
		position: absolute;
		inset-block-start: calc(100% + var(--space-2));
		inset-inline: 0;
		z-index: 16;
		padding: var(--space-2);
		background: color-mix(in oklab, var(--ink-050) 96%, transparent);
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-card);
		box-shadow: 0 18px 36px -12px rgb(0 0 0 / 0.6);
		max-block-size: 60vh;
		overflow-y: auto;
	}
	.topbar-search-busy {
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		color: var(--bone-200);
	}
	.topbar-hit {
		display: grid;
		grid-template-columns: 2.5rem 1fr;
		gap: var(--space-3);
		padding: var(--space-2);
		align-items: center;
		color: var(--bone-100);
		border-radius: calc(var(--radius-card) - 2px);
		text-decoration: none;
	}
	.topbar-hit.selected,
	.topbar-hit:hover {
		background: color-mix(in oklab, var(--accent) 14%, var(--ink-100));
	}
	.topbar-hit-poster {
		display: block;
		inline-size: 2.5rem;
		block-size: 3.5rem;
		overflow: hidden;
		border-radius: 4px;
		background: var(--ink-100);
	}
	.topbar-hit-poster img {
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
	}
	.topbar-hit-text {
		display: grid;
		gap: 2px;
		min-inline-size: 0;
	}
	.topbar-hit-title {
		font-family: var(--font-display);
		font-style: italic;
		font-size: var(--type-body);
		line-height: 1.2;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.topbar-hit-meta {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.topbar-dropdown-empty {
		margin: 0;
		padding: var(--space-3);
		font-family: var(--font-display);
		font-style: italic;
		color: var(--bone-300);
		text-align: center;
	}
	.topbar-dropdown-section {
		margin: 0;
		padding: var(--space-2) var(--space-3);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-400);
	}
	.topbar-recent {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		inline-size: 100%;
		padding: var(--space-2) var(--space-3);
		text-align: start;
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		color: var(--bone-200);
		border-radius: calc(var(--radius-card) - 2px);
		cursor: pointer;
	}
	.topbar-recent:hover {
		background: var(--ink-100);
		color: var(--bone-100);
	}
</style>
