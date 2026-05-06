<!--
  BackButton — the "obvious" back affordance the user asked for.
  Renders only when there's somewhere to go back to (history.length > 1).
  Falls back to a "/" link otherwise so a hard-loaded route still has a way home.
  Visual: large ← glyph + mono uppercase label. Hairline rule that grows on
  hover (consistent with the back style already on /search and /anime/[id]).
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import { afterNavigate } from '$app/navigation';

	interface Props {
		/** Label after the arrow. Defaults to "Back". */
		label?: string;
		/** Fallback href when there's no history to pop. Defaults to "/". */
		fallback?: string;
	}

	let { label = 'Back', fallback = '/' }: Props = $props();

	// `window.history.length` lies on app launch — Tauri's WebView often
	// reports >=2 even when nothing else is in the back stack, which is
	// why "as soon as we open the app, the button is visible" was the
	// reported bug. SvelteKit's afterNavigate is the source of truth:
	// it fires for every in-app navigation with a `from` route id once
	// at least one nav has happened. Until then, hide the button.
	let canGoBack = $state(false);

	afterNavigate(({ from, type }) => {
		if (from && type !== 'enter') {
			canGoBack = true;
		}
	});

	function onClick(e: MouseEvent) {
		if (!canGoBack) return; // let the anchor href take over
		e.preventDefault();
		window.history.back();
	}
</script>

{#if canGoBack}
	<a class="back" href={resolve(fallback as '/')} onclick={onClick} aria-label={label}>
		<span class="back-arrow" aria-hidden="true">←</span>
		<span class="back-label">{label}</span>
	</a>
{/if}

<style>
	.back {
		display: inline-flex;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-2) var(--space-3);
		margin-inline-start: calc(-1 * var(--space-3));
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-200);
		transition: color var(--dur-fast) var(--ease-out-soft);
	}
	.back:hover {
		color: var(--bone-100);
	}
	.back-arrow {
		font-family: var(--font-display);
		font-size: var(--type-display-s);
		line-height: 1;
		color: var(--bone-100);
		font-style: normal;
		transition: transform var(--dur-fast) var(--ease-out-soft);
	}
	.back:hover .back-arrow {
		transform: translateX(-2px);
	}
	.back-label {
		font-style: normal;
	}
</style>
