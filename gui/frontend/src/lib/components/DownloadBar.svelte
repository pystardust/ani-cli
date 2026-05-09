<!--
  DownloadBar — slim accent strip across the bottom of the screen,
  visible while one or more downloads are in flight. Gated behind
  Config.download_bottom_bar_enabled — when off, the topbar dock is
  the only surface.

  One thin progress row per active download. Indeterminate shimmer
  for now; once we parse aria2c's percentage out of the stderr line
  we can swap to a true progress fill.
-->
<script lang="ts">
	import { downloadStore } from '$lib/download/store';

	const active = $derived(downloadStore.active);
	const visible = $derived(active.length > 0);
</script>

{#if visible}
	<aside class="dl-bar" aria-label="Active downloads">
		{#each active as item (item.id)}
			<div class="dl-bar-row" title={item.progress ?? ''}>
				<span class="dl-bar-title">
					<span class="dl-bar-mark" aria-hidden="true">↓</span>
					<span class="dl-bar-text">{item.title}</span>
					<span class="dl-bar-ep">Ep {item.episode}</span>
				</span>
				<span class="dl-bar-progress" aria-hidden="true">
					<span></span>
				</span>
			</div>
		{/each}
	</aside>
{/if}

<style>
	.dl-bar {
		position: fixed;
		inset-inline: 0;
		inset-block-end: 0;
		display: flex;
		flex-direction: column;
		gap: 1px;
		padding: var(--space-2) var(--space-5);
		background: color-mix(in oklab, var(--ink-000) 92%, var(--accent));
		border-block-start: 1px solid color-mix(in oklab, var(--accent) 30%, var(--bone-400));
		z-index: 40;
		animation: dl-bar-rise var(--dur-med) var(--ease-out-soft) both;
	}
	@keyframes dl-bar-rise {
		from {
			transform: translateY(100%);
			opacity: 0;
		}
		to {
			transform: none;
			opacity: 1;
		}
	}
	.dl-bar-row {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		font-family: var(--font-body);
		font-size: var(--type-body-s);
		color: var(--bone-200);
	}
	.dl-bar-title {
		display: inline-flex;
		align-items: baseline;
		gap: var(--space-2);
		flex: 0 1 auto;
		min-inline-size: 0;
	}
	.dl-bar-mark {
		color: var(--accent);
		font-family: var(--font-mono);
	}
	.dl-bar-text {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		max-inline-size: 24rem;
	}
	.dl-bar-ep {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.dl-bar-progress {
		position: relative;
		flex: 1 1 auto;
		min-inline-size: 6rem;
		block-size: 3px;
		background: var(--ink-200);
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
</style>
