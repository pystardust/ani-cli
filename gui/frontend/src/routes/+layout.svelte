<!--
  Root layout — the app shell.
  - Imports tokens.css globally (hotfix from M3.1: ensures the warm-ink
    baseline applies on /, not just on routes that imported it themselves).
  - Persistent narrow left rail with home / search / continue / settings
    / diagnostics. Active item gets a 2px accent rule and bone-100 type;
    everything else is hairlined and quiet.
  - Wires the favicon via <svelte:head>.
  - Hides the rail on /play so the player gets the full viewport.
-->
<script lang="ts">
	import '$lib/design/tokens.css';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import Icon from '$lib/components/Icon.svelte';

	let { children } = $props();

	// Routes where the chrome should yield to content.
	const isPlayer = $derived(page.url.pathname.startsWith('/play'));

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
</script>

<svelte:head>
	<link rel="icon" type="image/svg+xml" href="/favicon.svg" />
</svelte:head>

{#if isPlayer}
	<main class="player-shell">
		{@render children()}
	</main>
{:else}
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

		<main class="content">
			{@render children()}
		</main>
	</div>
{/if}

<style>
	.player-shell {
		min-block-size: 100dvh;
	}

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
</style>
