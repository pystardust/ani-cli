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
	<div class="backdrop" role="status" aria-live="polite" aria-label="Loading">
		<div class="band">
			<div bind:this={container} class="anim"></div>
		</div>
	</div>
{/if}

<style>
	/* Two layers:
	   1. backdrop — fullscreen rgba dim that puts the rest of the page
	      out of focus while we resolve.
	   2. band     — full-width opaque-black row centred on the viewport
	      that frames the Lottie. The page above and below the band
	      remains dimly visible, so spatial context is preserved. */
	.backdrop {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.6);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
		animation: fade-in 160ms ease-out;
	}
	.band {
		width: 100%;
		background: rgb(0, 0, 0);
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 1.5rem 0;
	}
	.anim {
		height: 180px;
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
		.backdrop {
			animation: none;
		}
	}
</style>
