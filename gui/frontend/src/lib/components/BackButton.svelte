<!--
  BackButton — the "obvious" back affordance the user asked for.
  Renders only when there's somewhere to go back to (history.length > 1).
  Falls back to a "/" link otherwise so a hard-loaded route still has a way home.
  Visual: large ← glyph + mono uppercase label. Hairline rule that grows on
  hover (consistent with the back style already on /search and /anime/[id]).
-->
<script lang="ts">
	import { resolve } from '$app/paths';

	interface Props {
		/** Label after the arrow. Defaults to "Back". */
		label?: string;
		/** Fallback href when there's no history to pop. Defaults to "/". */
		fallback?: string;
	}

	let { label = 'Back', fallback = '/' }: Props = $props();

	// Visibility is decided upstream (in +layout.svelte's nav-depth
	// tracker) — by the time this component is rendered, the user is
	// somewhere with a non-empty back stack. The click handler always
	// calls history.back(); the href is a defensive fallback for edge
	// cases (browser refresh mid-flight, etc.).
	function onClick(e: MouseEvent) {
		e.preventDefault();
		window.history.back();
	}
</script>

<a class="back" href={resolve(fallback as '/')} onclick={onClick} aria-label={label}>
	<svg class="back-arrow" viewBox="0 0 16 16" aria-hidden="true">
		<path
			d="M10.5 3.5 6 8l4.5 4.5"
			fill="none"
			stroke="currentColor"
			stroke-width="2"
			stroke-linecap="round"
			stroke-linejoin="round"
		/>
	</svg>
	<span class="back-label">{label}</span>
</a>

<style>
	/* Pill-shaped control. Bone-100 + weight 600 matches .btn copy
	   weight elsewhere; the chevron is a 16x16 SVG so it's tightly
	   sized to the cap-height instead of using a display-serif `←`
	   that overshoots the type baseline. */
	.back {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		padding: 0.5rem 0.95rem 0.5rem 0.7rem;
		margin-inline-start: calc(-1 * var(--space-2));
		font-family: var(--font-mono);
		/* Bumped from --type-meta (12px) to --type-body-s (14px) so
		   "Back" reads at a comfortable size in the topbar — matches
		   the breadcrumb bump. */
		font-size: var(--type-body-s);
		font-weight: 600;
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-100);
		background: transparent;
		border: 1px solid transparent;
		border-radius: var(--radius-pill, 999px);
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft),
			transform var(--dur-fast) var(--ease-out-soft);
	}
	.back:hover {
		background: color-mix(in oklab, var(--bone-100) 8%, transparent);
		border-color: color-mix(in oklab, var(--bone-100) 22%, transparent);
	}
	.back:focus-visible {
		outline: none;
		background: color-mix(in oklab, var(--bone-100) 10%, transparent);
		border-color: color-mix(in oklab, var(--bone-100) 50%, transparent);
	}
	.back:active {
		transform: translateY(1px);
	}
	/* Chevron: stroked SVG so the line weight matches the type weight
	   (2px stroke ≈ font-weight 600 letterforms). The hover nudge
	   slides it left to suggest direction. */
	.back-arrow {
		inline-size: 1rem;
		block-size: 1rem;
		color: currentColor;
		transition: transform var(--dur-fast) var(--ease-out-soft);
	}
	.back:hover .back-arrow {
		transform: translateX(-2px);
	}
	.back-label {
		font-style: normal;
	}
</style>
