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
	.overlay {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.6);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
		/* Smooth fade-in so it doesn't feel like a popup glitch on a
		   short resolution (e.g. cached pre-fetch landing instantly). */
		animation: fade-in 160ms ease-out;
	}
	.anim {
		width: min(220px, 28vmin);
		height: min(220px, 28vmin);
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
