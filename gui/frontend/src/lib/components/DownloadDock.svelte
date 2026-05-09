<!--
  DownloadDock — topbar icon + popover dock for the download list.
  Mounted globally from +layout.svelte; observes the shared
  downloadStore so it can show progress regardless of which route
  the user is on.

  States:
    - Idle (no items): faint icon, no badge.
    - Active (≥1 pending/active item): accent-tinted icon + count
      badge.
    - Click: opens a popover anchored to the icon listing every
      item with progress / status / per-row actions.
-->
<script lang="ts">
	import { slide } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { downloadStore, type DownloadItem } from '$lib/download/store.svelte';

	let open = $state(false);
	let trigger = $state<HTMLButtonElement | null>(null);

	const itemsView = $derived(downloadStore.items as DownloadItem[]);
	const activeCount = $derived(downloadStore.active.length);
	const unseenCount = $derived(downloadStore.unseenCount);
	// Hide the dock entirely when there are no downloads — the icon
	// only earns space in the topbar when there's something to surface.
	// Slide transition animates the inline-size, so the topbar's search
	// flex sibling expands/contracts smoothly as the dock enters/leaves.
	const hasItems = $derived(itemsView.length > 0);

	// Clear the unseen flag whenever the dock opens — opening the
	// popover counts as the user seeing the latest completions.
	$effect(() => {
		if (open) downloadStore.markAllSeen();
	});

	$effect(() => {
		if (!open) return;
		const onPointerDown = (e: PointerEvent) => {
			const target = e.target as Node | null;
			if (!target) return;
			if (trigger?.contains(target)) return;
			const dock = document.getElementById('dl-dock-pop');
			if (dock?.contains(target)) return;
			open = false;
		};
		const onKey = (e: KeyboardEvent) => {
			if (e.key === 'Escape') open = false;
		};
		document.addEventListener('pointerdown', onPointerDown);
		document.addEventListener('keydown', onKey);
		return () => {
			document.removeEventListener('pointerdown', onPointerDown);
			document.removeEventListener('keydown', onKey);
		};
	});

	function reveal(dir: string) {
		const open = typeof window !== 'undefined' ? window.aniGui?.revealInFolder : null;
		if (open) void open(dir);
	}
</script>

{#if hasItems}
	<div class="dl-dock-wrap" transition:slide={{ axis: 'x', duration: 220, easing: cubicOut }}>
		<button
			bind:this={trigger}
			type="button"
			class="dl-dock-trigger"
			class:has-active={activeCount > 0}
			class:has-unseen={activeCount === 0 && unseenCount > 0}
			onclick={() => (open = !open)}
			aria-haspopup="menu"
			aria-expanded={open}
			aria-label={activeCount > 0
				? `${activeCount} download(s) in progress`
				: unseenCount > 0
					? `${unseenCount} download(s) finished`
					: 'Downloads'}
			title="Downloads"
		>
			<svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
				<path
					d="M12 4v12m0 0l-4-4m4 4l4-4M5 20h14"
					fill="none"
					stroke="currentColor"
					stroke-width="2.25"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
			{#if activeCount > 0}
				<span class="dl-badge" aria-hidden="true">{activeCount}</span>
			{:else if unseenCount > 0}
				<span class="dl-dot" aria-hidden="true"></span>
			{/if}
		</button>

		{#if open}
			<div id="dl-dock-pop" class="dl-dock-pop" role="menu" aria-label="Downloads">
				{#if itemsView.length === 0}
					<p class="dl-dock-empty">No downloads yet.</p>
				{:else}
					<ul class="dl-dock-list">
						{#each itemsView as item (item.id)}
							{@const epLabel =
								item.rangeTotal && item.currentEp != null
									? `Ep ${item.currentEp} of ${item.rangeTotal}`
									: item.rangeTotal
										? `Eps ${item.episode}`
										: `Ep ${item.episode}`}
							<li class="dl-row dl-row-{item.status}" title={item.title}>
								<span class="dl-row-text">
									<span class="dl-row-title">{item.title}</span>
									<span class="dl-row-ep">{epLabel}</span>
								</span>

								<!-- Active/pending: thin indeterminate bar fills the
								     remaining inline space + a single cancel (X). -->
								{#if item.status === 'active' || item.status === 'pending'}
									<span
										class="dl-row-bar dl-row-bar-indet"
										aria-hidden="true"
										title={item.progress ?? ''}
									>
										<span></span>
									</span>
									<button
										type="button"
										class="dl-row-act"
										onclick={() => downloadStore.cancel(item.id)}
										aria-label="Cancel download"
										title="Cancel"
									>
										<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
											<path
												d="M6 6l12 12M6 18L18 6"
												fill="none"
												stroke="currentColor"
												stroke-width="2"
												stroke-linecap="round"
											/>
										</svg>
									</button>
								{:else if item.status === 'done'}
									<button
										type="button"
										class="dl-row-act"
										onclick={() => reveal(item.destDir)}
										aria-label="Reveal in folder"
										title="Reveal in folder"
									>
										<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
											<path
												d="M3 7h6l2 2h10v10H3z"
												fill="none"
												stroke="currentColor"
												stroke-width="2"
												stroke-linecap="round"
												stroke-linejoin="round"
											/>
										</svg>
									</button>
									<button
										type="button"
										class="dl-row-act"
										onclick={() => downloadStore.dismiss(item.id)}
										aria-label="Dismiss"
										title="Dismiss"
									>
										<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
											<path
												d="M6 6l12 12M6 18L18 6"
												fill="none"
												stroke="currentColor"
												stroke-width="2"
												stroke-linecap="round"
											/>
										</svg>
									</button>
								{:else}
									<span class="dl-row-error" title={item.error ?? 'Failed'}>!</span>
									<button
										type="button"
										class="dl-row-act"
										onclick={() => downloadStore.dismiss(item.id)}
										aria-label="Dismiss"
										title="Dismiss"
									>
										<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
											<path
												d="M6 6l12 12M6 18L18 6"
												fill="none"
												stroke="currentColor"
												stroke-width="2"
												stroke-linecap="round"
											/>
										</svg>
									</button>
								{/if}
							</li>
						{/each}
					</ul>
				{/if}
			</div>
		{/if}
	</div>
{/if}

<style>
	.dl-dock-wrap {
		position: relative;
	}
	.dl-dock-trigger {
		position: relative;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		inline-size: 2.25rem;
		block-size: 2.25rem;
		background: transparent;
		border: 1px solid transparent;
		border-radius: var(--radius-sm);
		color: var(--bone-300);
		cursor: pointer;
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft);
	}
	.dl-dock-trigger:hover {
		color: var(--bone-100);
		background: color-mix(in oklab, var(--bone-100) 6%, transparent);
	}
	.dl-dock-trigger.has-active {
		color: var(--accent);
	}
	.dl-dock-trigger.has-unseen {
		color: var(--bone-100);
	}
	/* Notification dot for completed/errored downloads the user
	   hasn't seen yet. Smaller than the active-count badge so the
	   two states are visually distinct: badge = "in progress",
	   dot = "ready for you". */
	.dl-dot {
		position: absolute;
		inset-block-start: 4px;
		inset-inline-end: 4px;
		inline-size: 8px;
		block-size: 8px;
		background: var(--accent);
		border-radius: 50%;
		box-shadow: 0 0 0 2px var(--ink-000);
	}
	.dl-badge {
		position: absolute;
		inset-block-start: 2px;
		inset-inline-end: 2px;
		min-inline-size: 1rem;
		block-size: 1rem;
		padding: 0 4px;
		background: var(--accent);
		color: var(--ink-000);
		border-radius: 999px;
		font-family: var(--font-mono);
		font-size: 0.6875rem;
		font-weight: 600;
		display: inline-flex;
		align-items: center;
		justify-content: center;
	}
	/* Popover matches the topbar's glassy translucent treatment so
	   the dock reads as an extension of the chrome rather than a
	   floating card on top of the page. */
	.dl-dock-pop {
		position: absolute;
		inset-block-start: calc(100% + var(--space-2));
		inset-inline-end: 0;
		inline-size: 24rem;
		max-block-size: 28rem;
		overflow-y: auto;
		padding: var(--space-2);
		background: color-mix(in oklab, var(--ink-000) 65%, transparent);
		backdrop-filter: blur(16px) saturate(1.3);
		-webkit-backdrop-filter: blur(16px) saturate(1.3);
		border: 1px solid color-mix(in oklab, var(--ink-200) 80%, transparent);
		border-radius: var(--radius-sm);
		box-shadow: 0 10px 28px -8px rgb(0 0 0 / 0.55);
		z-index: 50;
	}
	.dl-dock-empty {
		margin: 0;
		padding: var(--space-3) var(--space-2);
		color: var(--bone-300);
		font-family: var(--font-body);
		font-size: var(--type-body-s);
		text-align: center;
	}
	.dl-dock-list {
		list-style: none;
		padding: 0;
		margin: 0;
		display: flex;
		flex-direction: column;
	}
	/* One row per item — horizontal layout, browser-style. Title
	   shrinks via ellipsis; trailing icon-only buttons sit at the
	   inline-end. Active rows fill the gap between text and buttons
	   with a thin indeterminate progress bar. */
	.dl-row {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-sm);
		transition: background var(--dur-fast) var(--ease-out-soft);
	}
	.dl-row:hover {
		background: color-mix(in oklab, var(--bone-100) 6%, transparent);
	}
	.dl-row + .dl-row {
		border-block-start: 1px solid color-mix(in oklab, var(--ink-200) 50%, transparent);
	}
	.dl-row-text {
		display: inline-flex;
		flex: 0 1 auto;
		min-inline-size: 0;
		align-items: baseline;
		gap: var(--space-2);
		overflow: hidden;
	}
	.dl-row-title {
		font-family: var(--font-body);
		font-size: var(--type-body-s);
		font-weight: 500;
		color: var(--bone-100);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		max-inline-size: 14rem;
	}
	.dl-row-ep {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
		flex-shrink: 0;
	}
	.dl-row-bar {
		position: relative;
		flex: 1 1 auto;
		min-inline-size: 2rem;
		block-size: 2px;
		background: color-mix(in oklab, var(--ink-200) 60%, transparent);
		border-radius: 999px;
		overflow: hidden;
	}
	.dl-row-bar-indet span {
		position: absolute;
		inset-block: 0;
		inline-size: 30%;
		background: var(--accent);
		animation: dl-indet 1.4s var(--ease-in-out) infinite;
	}
	@keyframes dl-indet {
		0% {
			inset-inline-start: -30%;
		}
		100% {
			inset-inline-start: 100%;
		}
	}
	.dl-row-error {
		margin-inline-start: auto;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		inline-size: 1.25rem;
		block-size: 1.25rem;
		background: color-mix(in oklab, var(--oxblood, #b11a1a) 25%, transparent);
		border-radius: 50%;
		color: var(--oxblood, #b11a1a);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		font-weight: 700;
		flex-shrink: 0;
		cursor: help;
	}
	/* For done rows, push the icon buttons to the inline-end (no
	   progress bar to fill the space). */
	.dl-row-done .dl-row-text {
		flex: 1 1 auto;
	}
	.dl-row-act {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		inline-size: 1.75rem;
		block-size: 1.75rem;
		padding: 0;
		background: transparent;
		border: 0;
		border-radius: var(--radius-sm);
		color: var(--bone-300);
		cursor: pointer;
		flex-shrink: 0;
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft);
	}
	.dl-row-act:hover {
		color: var(--bone-100);
		background: color-mix(in oklab, var(--bone-100) 10%, transparent);
	}
</style>
