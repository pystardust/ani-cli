<!--
  DownloadBar — slim accent strip across the bottom-right of the
  screen, visible while one or more downloads are in flight. Gated
  behind Config.download_bottom_bar_enabled — when off, the topbar
  dock is the only surface.

  Each row is a compact right-aligned cluster: short progress bar
  on top, episode caption beneath. Stacks vertically when multiple
  downloads run.
-->
<script lang="ts">
	import { fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { downloadStore } from '$lib/download/store.svelte';
	import { m } from '$lib/paraglide/messages';

	const active = $derived(downloadStore.active);
	const visible = $derived(active.length > 0);
</script>

{#if visible}
	<aside
		class="dl-bar"
		aria-label={m.download_bar_aria_label()}
		transition:fly={{ y: 16, duration: 220, easing: cubicOut }}
	>
		{#each active as item (item.id)}
			<div
				class="dl-bar-row"
				title={item.progress ?? ''}
				transition:fly={{ y: 8, duration: 180, easing: cubicOut }}
			>
				<span class="dl-bar-progress" aria-hidden="true">
					<span></span>
				</span>
				<span class="dl-bar-caption">
					<span class="dl-bar-text">{item.title}</span>
					<span class="dl-bar-sep" aria-hidden="true">·</span>
					<span class="dl-bar-ep">
						{#if item.rangeTotal && item.currentEp != null}
							{m.download_bar_ep_label_range_progress({
								current: item.currentEp,
								total: item.rangeTotal
							})}
						{:else if item.rangeTotal}
							{m.download_bar_ep_label_range_static({ episode: item.episode })}
						{:else}
							{m.download_bar_ep_label_single({ episode: item.episode })}
						{/if}
					</span>
				</span>
			</div>
		{/each}
	</aside>
{/if}

<style>
	.dl-bar {
		position: fixed;
		inset-inline-end: var(--space-5);
		inset-block-end: var(--space-3);
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		z-index: 40;
		pointer-events: none;
	}
	.dl-bar-row {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: 4px;
		padding: var(--space-2) var(--space-3);
		background: var(--ink-050);
		border: 1px solid var(--ink-200);
		border-radius: var(--radius-sm);
		box-shadow: 0 4px 14px -4px rgb(0 0 0 / 0.4);
	}
	.dl-bar-progress {
		position: relative;
		inline-size: 7rem;
		block-size: 2px;
		background: color-mix(in oklab, var(--ink-200) 70%, transparent);
		border-radius: 999px;
		overflow: hidden;
	}
	.dl-bar-progress span {
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
	.dl-bar-caption {
		display: inline-flex;
		align-items: baseline;
		gap: var(--space-2);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.dl-bar-text {
		max-inline-size: 18rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.dl-bar-sep {
		color: var(--bone-400);
	}
	.dl-bar-ep {
		color: var(--accent);
	}
</style>
