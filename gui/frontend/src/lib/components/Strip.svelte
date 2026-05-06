<!--
  Strip — horizontal-scroll section used by the home page. NOT a carousel:
  the user is in charge of advancing it. Scroll-snap, hidden scrollbar,
  arrow buttons that fade in on hover for mouse users. Arrow keys scroll the
  list a card width when the strip itself is focused.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		eyebrow: string;
		caption?: string | null;
		children: Snippet;
	}

	let { eyebrow, caption = null, children }: Props = $props();

	let scrollerEl: HTMLDivElement | undefined = $state();
	let canScrollLeft = $state(false);
	let canScrollRight = $state(false);

	function updateScrollState() {
		if (!scrollerEl) return;
		canScrollLeft = scrollerEl.scrollLeft > 4;
		canScrollRight = scrollerEl.scrollLeft + scrollerEl.clientWidth < scrollerEl.scrollWidth - 4;
	}

	$effect(() => {
		if (!scrollerEl) return;
		updateScrollState();
		const el = scrollerEl;
		const onScroll = () => updateScrollState();
		el.addEventListener('scroll', onScroll, { passive: true });
		const ro = new ResizeObserver(updateScrollState);
		ro.observe(el);
		return () => {
			el.removeEventListener('scroll', onScroll);
			ro.disconnect();
		};
	});

	function nudge(dir: 1 | -1) {
		if (!scrollerEl) return;
		// page by ~75% of the visible width so a card doesn't sit half-cut.
		const dx = scrollerEl.clientWidth * 0.75 * dir;
		scrollerEl.scrollBy({ left: dx, behavior: 'smooth' });
	}

	function onKey(e: KeyboardEvent) {
		if (e.key === 'ArrowRight') {
			e.preventDefault();
			nudge(1);
		} else if (e.key === 'ArrowLeft') {
			e.preventDefault();
			nudge(-1);
		}
	}
</script>

<section class="strip">
	<header class="strip-header">
		<h2 class="eyebrow">
			<span class="eyebrow-key">{eyebrow}</span>
			<span class="eyebrow-rule" aria-hidden="true"></span>
			{#if caption}<span class="eyebrow-value">{caption}</span>{/if}
		</h2>
		<div class="strip-nav" aria-hidden="true">
			<button
				type="button"
				class="strip-nav-btn"
				onclick={() => nudge(-1)}
				disabled={!canScrollLeft}
				tabindex="-1"
			>
				←
			</button>
			<button
				type="button"
				class="strip-nav-btn"
				onclick={() => nudge(1)}
				disabled={!canScrollRight}
				tabindex="-1"
			>
				→
			</button>
		</div>
	</header>

	<!--
	  Region with tabindex=0 so keyboard users can focus the strip and
	  page through it with ←/→. The arrow-key handler on the scroller is
	  the whole point — silence the lint rules that flag it as "non-
	  interactive element with keyboard handler". This is a deliberate
	  scroll-container affordance, not a click stand-in.
	-->
	<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<div
		class="strip-scroll"
		bind:this={scrollerEl}
		onkeydown={onKey}
		role="region"
		aria-label={eyebrow}
		tabindex="0"
	>
		{@render children()}
	</div>
</section>

<style>
	.strip {
		/* Wider gutter than space-6 so the first card on the inline-start
		   edge has room to breathe — at 88px rail + 48px strip-pad the
		   content begins ~136px from the viewport edge, which reads as
		   "considered" rather than "glued to the rail". */
		--strip-pad: var(--space-7);
		margin-block-end: var(--space-7);
	}

	.strip-header {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-5);
		padding-inline: var(--strip-pad);
		margin-block-end: var(--space-4);
	}

	.eyebrow {
		margin: 0;
		display: inline-flex;
		align-items: center;
		gap: var(--space-3);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
		font-weight: 500;
	}
	.eyebrow-key {
		color: var(--bone-200);
	}
	.eyebrow-rule {
		inline-size: 2.5rem;
		block-size: 1px;
		background: var(--bone-400);
	}
	.eyebrow-value {
		color: var(--bone-300);
	}

	.strip-nav {
		display: flex;
		gap: var(--space-2);
		opacity: 0;
		transition: opacity var(--dur-fast) var(--ease-out-soft);
	}
	.strip:hover .strip-nav,
	.strip:focus-within .strip-nav {
		opacity: 1;
	}
	.strip-nav-btn {
		inline-size: 2rem;
		block-size: 2rem;
		display: grid;
		place-items: center;
		font-family: var(--font-display);
		font-size: var(--type-body-l);
		color: var(--bone-200);
		border: 1px solid var(--ink-300);
		border-radius: 2px;
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft);
	}
	.strip-nav-btn:hover:not(:disabled) {
		color: var(--bone-100);
		border-color: var(--bone-300);
	}
	.strip-nav-btn:disabled {
		color: var(--bone-400);
		border-color: var(--ink-200);
		cursor: not-allowed;
	}

	.strip-scroll {
		display: grid;
		grid-auto-flow: column;
		grid-auto-columns: var(--strip-card);
		gap: var(--space-5);
		padding-inline: var(--strip-pad);
		padding-block-end: var(--space-3);
		overflow-x: auto;
		scroll-snap-type: inline mandatory;
		scrollbar-width: thin;
		scrollbar-color: var(--ink-300) transparent;
	}
	.strip-scroll:focus-visible {
		outline: none;
		box-shadow: var(--ring);
		border-radius: 2px;
	}
	.strip-scroll::-webkit-scrollbar {
		block-size: 6px;
	}
	.strip-scroll::-webkit-scrollbar-thumb {
		background: var(--ink-300);
		border-radius: 999px;
	}
	.strip-scroll::-webkit-scrollbar-track {
		background: transparent;
	}
</style>
