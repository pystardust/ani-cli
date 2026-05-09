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
	import { downloadStore, type DownloadItem } from '$lib/download/store.svelte';

	let open = $state(false);
	let trigger = $state<HTMLButtonElement | null>(null);

	const itemsView = $derived(downloadStore.items as DownloadItem[]);
	const activeCount = $derived(downloadStore.active.length);
	const unseenCount = $derived(downloadStore.unseenCount);

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

<div class="dl-dock-wrap">
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
						<li class="dl-dock-item dl-{item.status}">
							<div class="dl-dock-meta">
								<span class="dl-dock-title">{item.title}</span>
								<span class="dl-dock-ep">Ep {item.episode}</span>
							</div>
							{#if item.status === 'active' || item.status === 'pending'}
								<div class="dl-dock-line" title={item.progress ?? ''}>
									{item.progress ?? 'Resolving…'}
								</div>
								<div class="dl-dock-bar dl-dock-bar-indet" aria-hidden="true">
									<span></span>
								</div>
							{:else if item.status === 'done'}
								<p class="dl-dock-done">
									Saved to <code>{item.destDir}</code>
								</p>
							{:else}
								<p class="dl-dock-error">{item.error ?? 'Failed'}</p>
							{/if}
							<div class="dl-dock-actions">
								{#if item.status === 'active' || item.status === 'pending'}
									<button
										type="button"
										class="dl-dock-btn"
										onclick={() => downloadStore.cancel(item.id)}
									>
										Cancel
									</button>
								{:else if item.status === 'done'}
									<button type="button" class="dl-dock-btn" onclick={() => reveal(item.destDir)}>
										Reveal
									</button>
									<button
										type="button"
										class="dl-dock-btn dl-dock-btn-quiet"
										onclick={() => downloadStore.dismiss(item.id)}
									>
										Dismiss
									</button>
								{:else}
									<button
										type="button"
										class="dl-dock-btn dl-dock-btn-quiet"
										onclick={() => downloadStore.dismiss(item.id)}
									>
										Dismiss
									</button>
								{/if}
							</div>
						</li>
					{/each}
				</ul>
			{/if}
		</div>
	{/if}
</div>

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
	.dl-dock-pop {
		position: absolute;
		inset-block-start: calc(100% + var(--space-2));
		inset-inline-end: 0;
		inline-size: 22rem;
		max-block-size: 28rem;
		overflow-y: auto;
		padding: var(--space-3);
		background: color-mix(in oklab, var(--ink-050) 92%, var(--accent));
		border: 1px solid color-mix(in oklab, var(--accent) 25%, var(--bone-400));
		border-radius: var(--radius-card);
		box-shadow: var(--shadow-card-hover);
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
		gap: var(--space-3);
	}
	.dl-dock-item {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-3);
		background: var(--ink-000);
		border-radius: var(--radius-sm);
		border-inline-start: 2px solid transparent;
	}
	.dl-active,
	.dl-pending {
		border-inline-start-color: var(--accent);
	}
	.dl-done {
		border-inline-start-color: color-mix(in oklab, var(--accent) 50%, var(--bone-400));
	}
	.dl-error {
		border-inline-start-color: var(--oxblood, #b11a1a);
	}
	.dl-dock-meta {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		gap: var(--space-3);
	}
	.dl-dock-title {
		font-family: var(--font-body);
		font-size: var(--type-body-s);
		font-weight: 600;
		color: var(--bone-100);
		flex: 1 1 auto;
		min-inline-size: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.dl-dock-ep {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.dl-dock-line {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		color: var(--bone-300);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.dl-dock-bar {
		position: relative;
		block-size: 3px;
		background: var(--ink-200);
		border-radius: 999px;
		overflow: hidden;
	}
	.dl-dock-bar-indet span {
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
	.dl-dock-done,
	.dl-dock-error {
		margin: 0;
		font-family: var(--font-body);
		font-size: var(--type-meta);
		color: var(--bone-200);
	}
	.dl-dock-done code {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		color: var(--bone-300);
	}
	.dl-dock-error {
		color: var(--oxblood, #b11a1a);
	}
	.dl-dock-actions {
		display: flex;
		gap: var(--space-2);
		justify-content: flex-end;
	}
	.dl-dock-btn {
		padding: 2px var(--space-2);
		background: transparent;
		border: 1px solid var(--ink-200);
		border-radius: var(--radius-sm);
		color: var(--bone-200);
		font-family: var(--font-body);
		font-size: var(--type-micro);
		cursor: pointer;
		transition: background var(--dur-fast) var(--ease-out-soft);
	}
	.dl-dock-btn:hover {
		background: color-mix(in oklab, var(--accent) 18%, transparent);
		color: var(--bone-100);
	}
	.dl-dock-btn-quiet {
		border-color: transparent;
		color: var(--bone-300);
	}
</style>
