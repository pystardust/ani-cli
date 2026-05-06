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

	// Click + drag to scroll horizontally — desktop UX pattern users
	// expect from Netflix-style row layouts. We track the pointerdown
	// position, switch to a "dragging" cursor while held, and translate
	// pointer movement into scrollLeft. Anchor clicks during a drag are
	// suppressed so the user doesn't accidentally navigate.
	let isDragging = $state(false);
	let dragOriginX = 0;
	let dragOriginScroll = 0;
	let didMove = false;
	const DRAG_DEADZONE_PX = 6;

	function onPointerDown(e: PointerEvent) {
		// Left button only; ignore touch (the browser already handles
		// touch-scroll natively and this would interfere with it).
		if (e.button !== 0 || e.pointerType !== 'mouse' || !scrollerEl) return;
		isDragging = true;
		didMove = false;
		dragOriginX = e.clientX;
		dragOriginScroll = scrollerEl.scrollLeft;
		scrollerEl.setPointerCapture(e.pointerId);
	}

	function onPointerMove(e: PointerEvent) {
		if (!isDragging || !scrollerEl) return;
		const dx = e.clientX - dragOriginX;
		if (!didMove && Math.abs(dx) < DRAG_DEADZONE_PX) return;
		didMove = true;
		scrollerEl.scrollLeft = dragOriginScroll - dx;
	}

	function onPointerUp(e: PointerEvent) {
		if (!isDragging || !scrollerEl) return;
		isDragging = false;
		try {
			scrollerEl.releasePointerCapture(e.pointerId);
		} catch {
			// Pointer might already be released; ignore.
		}
	}

	// Suppress anchor clicks if the user dragged. Stop propagation in
	// capture phase so the click never reaches the <a> inside the card.
	function onClickCapture(e: MouseEvent) {
		if (didMove) {
			e.preventDefault();
			e.stopPropagation();
			didMove = false;
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
	</header>

	<div class="strip-frame">
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
			class:dragging={isDragging}
			bind:this={scrollerEl}
			onkeydown={onKey}
			onpointerdown={onPointerDown}
			onpointermove={onPointerMove}
			onpointerup={onPointerUp}
			onpointercancel={onPointerUp}
			onclickcapture={onClickCapture}
			ondragstart={(e) => e.preventDefault()}
			role="region"
			aria-label={eyebrow}
			tabindex="0"
		>
			<!--
			  Inner rail holds the gutter padding, NOT the scroll container.
			  This keeps the first card aligned with the eyebrow above it
			  on initial render, and lets the leading gutter scroll OFF the
			  viewport when the user pages right (so cards can touch the
			  inline-start edge once scrolled, per user feedback).
			-->
			<div class="strip-rail">
				{@render children()}
			</div>
		</div>

		<!-- Edge fades + page buttons: the visible scrollbar is gone, so
		     the row's edges have to do the indicating. The fades say "more
		     content this way" without taking up vertical real estate; the
		     chevron buttons let the user advance without dragging. -->
		<button
			type="button"
			class="strip-edge strip-edge-start"
			class:visible={canScrollLeft}
			onclick={() => nudge(-1)}
			aria-label="Previous"
			tabindex="-1"
		>
			<span class="strip-edge-chev" aria-hidden="true">‹</span>
		</button>
		<button
			type="button"
			class="strip-edge strip-edge-end"
			class:visible={canScrollRight}
			onclick={() => nudge(1)}
			aria-label="Next"
			tabindex="-1"
		>
			<span class="strip-edge-chev" aria-hidden="true">›</span>
		</button>
	</div>
</section>

<style>
	.strip {
		/* Generous gutter so the first card has real breathing room from
		   the rail's vertical hairline — at 88px rail + 72px strip-pad,
		   the content begins ~160px from the viewport edge, which reads
		   as "designed" rather than "glued to the rail". */
		--strip-pad: var(--space-8);
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

	.strip-frame {
		position: relative;
	}

	.strip-scroll {
		overflow-x: auto;
		/* No snap. Mandatory snap was hijacking initial layout — pulling
		   the first card flush to the container edge (snapport defaults
		   to scroll-padding 0, ignoring the rail's leading padding).
		   It also caused the "snaps to next item on release" feel the
		   user flagged. Free scroll, native momentum, user parks where
		   they want. */
		/* Hide the native scrollbar — the edge fades + chevron buttons
		   below do the indicating. The bar took up a vertical band of
		   space and felt utilitarian; this row should feel curated. */
		scrollbar-width: none;
		cursor: grab;
		-webkit-user-select: none;
		user-select: none;
	}
	.strip-scroll::-webkit-scrollbar {
		display: none;
	}
	.strip-rail {
		display: grid;
		grid-auto-flow: column;
		grid-auto-columns: var(--strip-card);
		gap: var(--space-5);
		/* Padding lives on the rail, not the scroll container — keeps
		   first-card alignment matching the eyebrow on initial render. */
		padding-inline: var(--strip-pad);
		padding-block-end: var(--space-3);
	}
	/* Suppress native HTML5 image drag inside cards — without this, mousing
	   on a poster triggers the browser's image-drag ghost, which fights
	   our pointer-drag scroll and kills its perf. pointer-events: none
	   forwards clicks to the parent <a>, which still navigates fine. */
	.strip-scroll :global(img) {
		-webkit-user-drag: none;
		pointer-events: none;
	}
	.strip-scroll.dragging {
		cursor: grabbing;
	}
	.strip-scroll:focus-visible {
		outline: none;
		box-shadow: var(--ring);
		border-radius: 2px;
	}

	/* Edge-mounted page buttons. They sit absolutely positioned over the
	   scroller, with a horizontal gradient fading the strip's background
	   color in over the trailing cards. This both hints at the next set
	   of items and gives the chevron a substrate to land on. */
	.strip-edge {
		position: absolute;
		inset-block: 0;
		display: grid;
		place-items: center;
		inline-size: 4rem;
		padding: 0;
		border: 0;
		opacity: 0;
		pointer-events: none;
		transition: opacity var(--dur-med) var(--ease-out-soft);
		z-index: 2;
	}
	.strip-edge.visible {
		opacity: 1;
		pointer-events: auto;
	}
	.strip-edge-start {
		inset-inline-start: 0;
		background: linear-gradient(
			to right,
			var(--ink-000) 0%,
			color-mix(in oklab, var(--ink-000) 80%, transparent) 55%,
			transparent 100%
		);
	}
	.strip-edge-end {
		inset-inline-end: 0;
		background: linear-gradient(
			to left,
			var(--ink-000) 0%,
			color-mix(in oklab, var(--ink-000) 80%, transparent) 55%,
			transparent 100%
		);
	}
	.strip-edge-chev {
		display: grid;
		place-items: center;
		inline-size: 2.25rem;
		block-size: 2.25rem;
		font-family: var(--font-display);
		font-size: 1.75rem;
		line-height: 1;
		color: var(--bone-100);
		background: color-mix(in oklab, var(--ink-100) 80%, transparent);
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-pill);
		backdrop-filter: blur(4px);
		transition:
			background var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft),
			transform var(--dur-fast) var(--ease-out-soft);
	}
	.strip-edge:hover .strip-edge-chev {
		background: var(--ink-100);
		border-color: var(--bone-300);
		transform: scale(1.08);
	}
	.strip-edge:active .strip-edge-chev {
		transform: scale(0.96);
	}
</style>
