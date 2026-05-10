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
	import { m } from '$lib/paraglide/messages';

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

	function confirmCancel(item: DownloadItem) {
		// Native confirm — minimal but unambiguous. Dock has no
		// other modal pattern yet; can be swapped for an inline
		// "Confirm? [yes][no]" later if multiple destructive
		// affordances accumulate.
		const ok =
			typeof window !== 'undefined' ? window.confirm(`Cancel download of "${item.title}"?`) : true;
		if (ok) downloadStore.cancel(item.id);
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
			class:open
			onclick={() => (open = !open)}
			aria-haspopup="menu"
			aria-expanded={open}
			aria-label={activeCount > 0
				? m.download_dock_active_label({ count: activeCount })
				: unseenCount > 0
					? m.download_dock_unseen_label({ count: unseenCount })
					: m.download_dock_idle_label()}
			title={m.download_dock_title()}
		>
			<svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true">
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
			<div id="dl-dock-pop" class="dl-dock-pop" role="menu" aria-label={m.download_dock_title()}>
				{#if itemsView.length === 0}
					<p class="dl-dock-empty">{m.download_dock_empty()}</p>
				{:else}
					<ul class="dl-dock-list">
						{#each itemsView as item (item.id)}
							{@const epLabel =
								item.rangeTotal && item.currentEp != null
									? m.download_dock_ep_label_range_progress({
											current: item.currentEp,
											total: item.rangeTotal
										})
									: item.rangeTotal
										? m.download_dock_ep_label_range_static({ episode: item.episode })
										: m.download_dock_ep_label_single({ episode: item.episode })}
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
										onclick={() => confirmCancel(item)}
										aria-label={m.download_dock_cancel_aria_label()}
										title={m.download_dock_cancel_title()}
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
										aria-label={m.download_dock_reveal_aria_label()}
										title={m.download_dock_reveal_title()}
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
										aria-label={m.download_dock_dismiss_aria_label()}
										title={m.download_dock_dismiss_title()}
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
								{:else if item.errorKind === 'ffmpeg_missing'}
									<!-- ffmpeg-missing CTA: ani-cli's `dep_ch "ffmpeg" "aria2c"`
									     dies the moment downloads start without ffmpeg, and
									     bundling ffmpeg blows the installer up to ~190 MB. The
									     backend now surfaces a typed FfmpegMissing error; the
									     row replaces the bare "!" with a one-link install
									     pointer at the official Windows builds. The lazy in-app
									     fetch flow lands later — for now this is the manual
									     escape hatch. -->
									<span class="dl-row-error" aria-hidden="true">!</span>
									<a
										class="dl-row-act dl-row-link"
										href="https://www.gyan.dev/ffmpeg/builds/"
										target="_blank"
										rel="noopener noreferrer"
										title={m.download_dock_ffmpeg_missing_help()}
									>
										{m.download_dock_ffmpeg_install_link()}
									</a>
									<button
										type="button"
										class="dl-row-act"
										onclick={() => downloadStore.dismiss(item.id)}
										aria-label={m.download_dock_dismiss_aria_label()}
										title={m.download_dock_dismiss_title()}
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
									<span class="dl-row-error" title={item.error ?? m.errors_failed_default()}>!</span
									>
									<button
										type="button"
										class="dl-row-act"
										onclick={() => downloadStore.dismiss(item.id)}
										aria-label={m.download_dock_dismiss_aria_label()}
										title={m.download_dock_dismiss_title()}
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
		inline-size: 2.5rem;
		block-size: 2.5rem;
		background: transparent;
		border: 1px solid transparent;
		/* Rounded square — radius-card (8px) gives noticeable corner
		   rounding without going full circle. var(--radius-sm) was
		   an undefined token leaving the active state square at 0. */
		border-radius: var(--radius-card);
		color: var(--bone-300);
		cursor: pointer;
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft);
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
	/* Open state — same surface as the popover so the trigger
	   reads as the popover's anchor handle rather than a separate
	   chip floating in the topbar. */
	.dl-dock-trigger.open {
		background: var(--ink-050);
		border-color: var(--ink-300);
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
	/* Popover — opaque surface so list contents read clearly without
	   page imagery bleeding through the rows. Same radius/border/
	   shadow vocabulary as the topbar search-preview. */
	.dl-dock-pop {
		position: absolute;
		inset-block-start: calc(100% + var(--space-2));
		inset-inline-end: 0;
		inline-size: 24rem;
		max-block-size: 28rem;
		overflow-x: hidden;
		overflow-y: auto;
		padding: var(--space-2);
		background: var(--ink-050);
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-card);
		box-shadow: 0 18px 36px -12px rgb(0 0 0 / 0.6);
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
		min-inline-size: 0;
		overflow: hidden;
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
