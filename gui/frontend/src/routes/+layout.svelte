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

	let { children } = $props();

	// Routes where the chrome should yield to content.
	const isPlayer = $derived(page.url.pathname.startsWith('/play'));

	const path = $derived(page.url.pathname);
	const isHome = $derived(path === '/');
	const isSearch = $derived(path.startsWith('/search'));
	const isSettings = $derived(path.startsWith('/settings'));
	const isDiagnostics = $derived(path.startsWith('/diagnostics'));
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
				<span class="brand-frame" aria-hidden="true">
					<span class="brand-amp">&amp;</span>
				</span>
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
							<span class="nav-mark" aria-hidden="true">◇</span>
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
							<span class="nav-mark" aria-hidden="true">/</span>
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
							<span class="nav-mark" aria-hidden="true">·</span>
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
							<span class="nav-mark" aria-hidden="true">#</span>
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
	.brand-frame {
		position: relative;
		display: grid;
		place-items: center;
		inline-size: 2.25rem;
		block-size: 2.25rem;
		border: 1.5px solid var(--bone-100);
	}
	.brand-frame::before {
		/* registration tick, top-left corner — same as the favicon mark */
		content: '';
		position: absolute;
		inset-block-start: 4px;
		inset-inline-start: 4px;
		inline-size: 4px;
		block-size: 4px;
		border-block-start: 1px solid var(--bone-100);
		border-inline-start: 1px solid var(--bone-100);
	}
	.brand-amp {
		font-family: var(--font-display);
		font-style: italic;
		font-size: 1.1rem;
		line-height: 1;
		color: var(--bone-100);
		transform: translate(1px, -1px);
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

	.nav-mark {
		font-family: var(--font-display);
		font-style: italic;
		font-size: var(--type-body-l);
		color: inherit;
		line-height: 1;
	}
	.nav-label {
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
