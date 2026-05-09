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
	import { downloadStore } from '$lib/download/store.svelte';

	const active = $derived(downloadStore.active);
	const visible = $derived(active.length > 0);
</script>

{#if visible}
	<aside class="dl-bar" aria-label="Active downloads">
		{#each active as item (item.id)}
			<div class="dl-bar-row" title={item.progress ?? ''}>
				<span class="dl-bar-progress" aria-hidden="true">
					<span></span>
				</span>
				<span class="dl-bar-caption">
					<span class="dl-bar-text">{item.title}</span>
					<span class="dl-bar-sep" aria-hidden="true">·</span>
					<span class="dl-bar-ep">Ep {item.episode}</span>
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
		animation: dl-bar-rise var(--dur-med) var(--ease-out-soft) both;
	}
	@keyframes dl-bar-rise {
		from {
			transform: translateY(8px);
			opacity: 0;
		}
		to {
			transform: none;
			opacity: 1;
		}
	}
	.dl-bar-row {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: 4px;
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
