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
		// Pointer capture is deliberately deferred to pointermove —
		// see the comment there. Capturing here breaks plain clicks
		// in Chromium.
	}

	function onPointerMove(e: PointerEvent) {
		if (!isDragging || !scrollerEl) return;
		const dx = e.clientX - dragOriginX;
		if (!didMove && Math.abs(dx) < DRAG_DEADZONE_PX) return;
		if (!didMove) {
			// First time across the deadzone — now we're definitely
			// dragging, not clicking. Capture so the drag keeps
			// tracking even if the cursor leaves the scroller.
			//
			// Capturing on pointerdown (the obvious-looking spot)
			// breaks navigation in Chromium: the synthesized click
			// for a no-move release is dispatched to the captured
			// element, not to the <a> under the cursor — so the
			// poster cards became unclickable under Electron even
			// though they worked under Tauri's webkit2gtk webview.
			didMove = true;
			try {
				scrollerEl.setPointerCapture(e.pointerId);
			} catch {
				// Some browsers reject capture mid-gesture; the drag
				// still works without it, just less smoothly off-edge.
			}
		}
		scrollerEl.scrollLeft = dragOriginScroll - dx;
	}

	function onPointerUp(e: PointerEvent) {
		if (!isDragging || !scrollerEl) return;
		isDragging = false;
		try {
			scrollerEl.releasePointerCapture(e.pointerId);
		} catch {
			// Capture might never have been set (pure click) or
			// already released; either way, nothing to do.
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

		<!-- Edge fades + arrow buttons. Split into two layers so the
		     gradient stays a non-interactive backdrop ("more content
		     this way") and the arrow is a small, deliberately-styled
		     click target floating above the fade. Fades use a
		     horizontal linear gradient against the page background
		     so the falloff reads clearly even on dark posters. -->
		<div class="strip-fade strip-fade-start" class:visible={canScrollLeft} aria-hidden="true"></div>
		<div class="strip-fade strip-fade-end" class:visible={canScrollRight} aria-hidden="true"></div>
		<button
			type="button"
			class="strip-arrow strip-arrow-start"
			class:visible={canScrollLeft}
			onclick={() => nudge(-1)}
			aria-label="Previous"
			tabindex="-1"
		>
			<svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
				<path
					d="M15 6l-6 6 6 6"
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
			class="strip-arrow strip-arrow-end"
			class:visible={canScrollRight}
			onclick={() => nudge(1)}
			aria-label="Next"
			tabindex="-1"
		>
			<svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
				<path
					d="M9 6l6 6-6 6"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
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
		font-family: var(--font-body);
		/* Keep the tight uppercase eyebrow shape, just lift contrast
		   on the active part (key) and let the caption drop into
		   muted territory so the hierarchy reads at a glance. */
		font-size: 0.8125rem; /* 13px */
		letter-spacing: 0.14em;
		text-transform: uppercase;
		font-weight: 600;
		color: color-mix(in oklab, var(--bone-100) 82%, transparent);
	}
	.eyebrow-key {
		color: color-mix(in oklab, var(--bone-100) 82%, transparent);
	}
	.eyebrow-rule {
		inline-size: 2.5rem;
		block-size: 1px;
		background: color-mix(in oklab, var(--bone-100) 22%, transparent);
	}
	.eyebrow-value {
		font-weight: 500;
		color: color-mix(in oklab, var(--bone-100) 38%, transparent);
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

	/* Edge fade — non-interactive gradient backdrop that says "more
	   content this way". Wider (9rem ≈ 144px) and stronger than the
	   previous radial: solid page-background at the edge, semi-solid
	   in the middle, transparent past 70%. Reads clearly even when
	   the cards underneath are dark posters. */
	.strip-fade {
		position: absolute;
		inset-block: 0;
		inline-size: 9rem;
		opacity: 0;
		pointer-events: none;
		transition: opacity var(--dur-med) var(--ease-out-soft);
		z-index: 2;
	}
	.strip-fade.visible {
		opacity: 1;
	}
	.strip-fade-start {
		inset-inline-start: 0;
		background: linear-gradient(
			to right,
			var(--ink-000) 0%,
			color-mix(in oklab, var(--ink-000) 90%, transparent) 30%,
			color-mix(in oklab, var(--ink-000) 60%, transparent) 60%,
			transparent 100%
		);
	}
	.strip-fade-end {
		inset-inline-end: 0;
		background: linear-gradient(
			to left,
			var(--ink-000) 0%,
			color-mix(in oklab, var(--ink-000) 90%, transparent) 30%,
			color-mix(in oklab, var(--ink-000) 60%, transparent) 60%,
			transparent 100%
		);
	}

	/* Arrow buttons — light glass surface so the control is
	   visible against the dark page background. Black-on-dark
	   was nearly invisible; a 14% bone-tinted fill plus a 30%
	   bone border + halo shadow lifts it clearly. Hover swaps
	   to the per-show accent for unmistakable affordance. */
	.strip-arrow {
		position: absolute;
		inset-block-start: 50%;
		transform: translateY(-50%);
		display: inline-flex;
		align-items: center;
		justify-content: center;
		inline-size: 3rem;
		block-size: 3rem;
		padding: 0;
		border: 1px solid color-mix(in oklab, var(--bone-100) 30%, transparent);
		border-radius: var(--radius-pill);
		background: color-mix(in oklab, var(--bone-100) 14%, var(--ink-100));
		color: var(--bone-100);
		backdrop-filter: blur(10px);
		box-shadow:
			0 12px 28px -6px rgb(0 0 0 / 0.7),
			0 0 0 1px rgb(0 0 0 / 0.4);
		opacity: 0;
		pointer-events: none;
		cursor: pointer;
		transition:
			opacity var(--dur-med) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft),
			transform var(--dur-fast) var(--ease-out-soft);
		z-index: 3;
	}
	.strip-arrow.visible {
		opacity: 1;
		pointer-events: auto;
	}
	.strip-arrow-start {
		inset-inline-start: var(--space-4);
	}
	.strip-arrow-end {
		inset-inline-end: var(--space-4);
	}
	.strip-arrow:hover {
		background: var(--accent);
		border-color: var(--accent);
		color: var(--ink-000);
		transform: translateY(-50%) scale(1.08);
		box-shadow:
			0 14px 32px -6px color-mix(in oklab, var(--accent) 60%, transparent),
			0 0 0 1px rgb(0 0 0 / 0.4);
	}
	.strip-arrow:active {
		transform: translateY(-50%) scale(0.96);
	}
	@media (prefers-reduced-motion: reduce) {
		.strip-arrow:hover,
		.strip-arrow:active {
			transform: translateY(-50%);
		}
	}
</style>
