<!--
  Fullscreen loading overlay used while the backend resolves a stream.
  Black-transparent backdrop, a centered Lottie animation, no text — the
  intent is to communicate "we're working on it" without competing with
  the show metadata that's still visible underneath.

  Lottie is imported lazily (`import('lottie-web')`) so the 70 KB
  runtime only enters the bundle when an overlay actually mounts. The
  animation JSON is bundled as a JSON module — small enough that
  inlining it beats a runtime fetch, and packaged Electron's file://
  loader has no idea what to do with `/anim/loading.json` paths.
-->
<script lang="ts">
	import { onDestroy } from 'svelte';
	import loadingJson from '$lib/anim/loading.json';

	let { visible = false } = $props<{ visible?: boolean }>();

	let container: HTMLDivElement | undefined = $state();
	type LottieInstance = { destroy: () => void };
	let animation: LottieInstance | null = null;

	$effect(() => {
		if (!visible || !container || animation) return;
		void (async () => {
			const lottie = (await import('lottie-web')).default;
			if (!container) return;
			animation = lottie.loadAnimation({
				container,
				renderer: 'svg',
				loop: true,
				autoplay: true,
				animationData: loadingJson
			});
		})();
	});

	$effect(() => {
		if (!visible && animation) {
			animation.destroy();
			animation = null;
		}
	});

	onDestroy(() => {
		if (animation) {
			animation.destroy();
			animation = null;
		}
	});
</script>

{#if visible}
	<div class="overlay" role="status" aria-live="polite" aria-label="Loading">
		<div bind:this={container} class="anim"></div>
	</div>
{/if}

<style>
	/* Horizontal band — full-width opaque black row centred on the
	   viewport, vertical padding so the lottie has breathing room
	   above and below. The page underneath stays visible top + bottom,
	   only this row gets blocked while the resolution runs. */
	.overlay {
		position: fixed;
		left: 0;
		right: 0;
		top: 50%;
		transform: translateY(-50%);
		background: rgb(0, 0, 0);
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 1.25rem 0;
		z-index: 1000;
		animation: fade-in 160ms ease-out;
	}
	.anim {
		height: 96px;
		aspect-ratio: 1 / 1;
	}
	@keyframes fade-in {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.overlay {
			animation: none;
		}
	}
</style>
