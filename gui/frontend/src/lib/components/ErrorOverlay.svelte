<!--
  Fullscreen error overlay used when a primary action fails (the
  click → play → error path). Mirrors LoadingOverlay's geometry —
  fixed-position dim backdrop, centred band — so a scrolled page
  doesn't hide the message. Distinct from LoadingOverlay in three
  ways: an Accent-coloured rule on the band (vs. the Lottie), a
  body line for the human-readable copy, and a dismiss button so
  the user can clear it without navigating away.

  Dismissal is driven by `onDismiss` so the parent owns whether the
  state goes back to "idle" or "open another path" (e.g. a Try
  External Player button could replace the dismiss in a future
  iteration). The Escape key + clicking the backdrop are wired to
  the same handler.
-->
<script lang="ts">
	import { m } from '$lib/paraglide/messages';

	let { headline, body, dismissLabel, onDismiss, actionLabel, actionHref } = $props<{
		/** Short eyebrow line — `Couldn't play episode N` style. */
		headline: string;
		/** Full sentence the user reads. Should suggest what to try
		 *  next ("try again in a few minutes", "check connection"). */
		body: string;
		/** Override the default dismiss label. Useful if a future
		 *  variant wants "Try external player" or similar. When
		 *  omitted, falls back to the localized `errors.dismiss_default_label`. */
		dismissLabel?: string;
		/** Called when the user clicks the button, presses Escape, or
		 *  clicks the dim backdrop. Parent should reset the error
		 *  state in the handler. */
		onDismiss: () => void;
		/** Optional secondary action — when both `actionLabel` and
		 *  `actionHref` are set, an `<a>` renders next to dismiss
		 *  pointing at an external resource (e.g. "Download ffmpeg"
		 *  → ffmpeg.org/download). Electron's setWindowOpenHandler
		 *  routes target=_blank links through shell.openExternal. */
		actionLabel?: string;
		actionHref?: string;
	}>();
	const displayDismissLabel = $derived(dismissLabel ?? m.errors_dismiss_default_label());
	const hasAction = $derived(
		typeof actionLabel === 'string' &&
			actionLabel.length > 0 &&
			typeof actionHref === 'string' &&
			actionHref.length > 0
	);

	function onBackdropClick(e: MouseEvent) {
		// Only fire when the click hit the backdrop itself, not the
		// card. Allows clicks-through prevention without blocking
		// the dismiss button from working.
		if (e.target === e.currentTarget) onDismiss();
	}

	function onKey(e: KeyboardEvent) {
		if (e.key === 'Escape') onDismiss();
	}
</script>

<svelte:window on:keydown={onKey} />

<!-- The role=alertdialog backdrop is the dismiss surface; tabindex=-1
     keeps it unfocusable but reachable for the global Escape handler
     wired via svelte:window above. Keyboard activation flows through
     the dismiss <button> below — the rule wants a keydown on the div,
     but Escape-handling already covers the keyboard path globally. -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
	class="backdrop"
	role="alertdialog"
	aria-modal="true"
	aria-labelledby="error-headline"
	tabindex="-1"
	onclick={onBackdropClick}
>
	<div class="card" role="document">
		<p id="error-headline" class="headline">{headline}</p>
		<p class="body">{body}</p>
		<div class="actions">
			{#if hasAction}
				<!-- External URL — `resolve()` is for SvelteKit route
				     ids, not arbitrary outbound links. Electron's
				     setWindowOpenHandler hands target=_blank to
				     shell.openExternal, which is what we want. -->
				<!-- eslint-disable svelte/no-navigation-without-resolve -->
				<a
					class="action"
					href={actionHref}
					target="_blank"
					rel="noopener noreferrer"
					data-testid="error-overlay-action"
				>
					{actionLabel}
				</a>
				<!-- eslint-enable svelte/no-navigation-without-resolve -->
			{/if}
			<button type="button" class="dismiss" onclick={onDismiss}>{displayDismissLabel}</button>
		</div>
	</div>
</div>

<style>
	/* Backdrop: a tinted scrim with a touch of blur softens the
	   transition between page and dialog. Page text remains
	   recognisable underneath, so spatial context isn't lost. */
	.backdrop {
		position: fixed;
		inset: 0;
		background: rgb(0 0 0 / 0.55);
		backdrop-filter: blur(6px);
		-webkit-backdrop-filter: blur(6px);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
		animation: fade-in var(--dur-med, 260ms) var(--ease-out-soft, ease-out);
		padding: var(--space-6, 1.5rem);
	}
	/* Errors pin to oxblood so failures read as alarms across the app,
	   not as the current show's brand colour. The rest of the dialog's
	   accent-mix logic (border, dot, button outline) reads off this
	   local override, leaving --accent untouched outside the overlay. */
	.card {
		--alarm: var(--accent-oxblood, #c45947);
		max-width: 30rem;
		background: color-mix(in oklab, var(--alarm) 5%, var(--ink-100, #181814));
		color: var(--bone-100, #f3eee5);
		border: 1px solid
			color-mix(in oklab, var(--alarm) 30%, var(--bone-500, rgba(255, 255, 255, 0.12)));
		border-radius: var(--radius-card, 8px);
		padding: var(--space-5, 1.5rem) var(--space-6, 2rem) var(--space-5, 1.5rem);
		display: flex;
		flex-direction: column;
		gap: var(--space-3, 0.75rem);
		box-shadow: var(--shadow-card-hover, 0 12px 24px -8px rgb(0 0 0 / 0.55));
		animation: card-in var(--dur-med, 260ms) var(--ease-out-elastic, ease-out);
	}
	/* Eyebrow row: a small accent dot before the headline echoes the
	   dot motifs used elsewhere (strip eyebrows, status pills) and
	   carries the per-show accent into the dialog. Bumped to bone-100
	   + weight 600 so it has presence — the dimmer bone-300 here read
	   as "decorative" against the body copy below. */
	.headline {
		margin: 0;
		display: inline-flex;
		align-items: center;
		gap: var(--space-2, 0.5rem);
		font-family: var(--font-mono, monospace);
		font-size: var(--type-micro, 0.75rem);
		font-weight: 600;
		letter-spacing: var(--tracking-micro, 0.08em);
		text-transform: uppercase;
		color: var(--bone-100, #f3eee5);
	}
	.headline::before {
		content: '';
		inline-size: 0.5rem;
		block-size: 0.5rem;
		border-radius: var(--radius-pill, 999px);
		background: var(--alarm, var(--accent-oxblood, #c45947));
	}
	/* Body copy: weight 500 + the larger body-l size for a card the
	   user has to read carefully. The body is the main message; pure
	   regular weight made it feel like footer text. */
	.body {
		margin: 0;
		font-family: var(--font-body, system-ui);
		font-size: var(--type-body-l, 1.125rem);
		font-weight: 500;
		line-height: 1.5;
		color: var(--bone-100, #f4ece1);
	}
	/* Action row: right-aligned cluster holding the optional external
	   link + the dismiss button. Gap matches the rest of the card so
	   the row sits naturally below the body without an extra rule. */
	.actions {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		align-items: center;
		gap: var(--space-3, 0.75rem);
		margin-top: var(--space-2, 0.5rem);
	}
	/* Action link: visually paired with the dismiss button (same
	   pill geometry, same focus ring) but filled with the alarm
	   colour so it reads as the recommended next step. The
	   underlying anchor stays an <a target=_blank> so Electron's
	   setWindowOpenHandler hands it to shell.openExternal. */
	.action {
		padding: 0.6rem 1.5rem;
		font-family: var(--font-mono, monospace);
		font-size: var(--type-meta, 0.875rem);
		font-weight: 600;
		letter-spacing: var(--tracking-micro, 0.08em);
		text-transform: uppercase;
		text-decoration: none;
		color: var(--bone-100, #f4ece1);
		background: color-mix(in oklab, var(--alarm, #c45947) 38%, transparent);
		border: 1px solid var(--alarm, #c45947);
		border-radius: var(--radius-pill, 999px);
		transition:
			background var(--dur-fast, 140ms) var(--ease-out-soft, ease-out),
			transform var(--dur-fast, 140ms) var(--ease-out-soft, ease-out);
	}
	.action:hover {
		background: color-mix(in oklab, var(--alarm, #c45947) 55%, transparent);
	}
	.action:focus-visible {
		outline: none;
		box-shadow:
			0 0 0 2px var(--ink-000, #000),
			0 0 0 4px var(--alarm, var(--accent-oxblood, #c45947));
	}
	.action:active {
		transform: translateY(1px);
	}
	.dismiss {
		padding: 0.6rem 1.5rem;
		font-family: var(--font-mono, monospace);
		font-size: var(--type-meta, 0.875rem);
		font-weight: 600;
		letter-spacing: var(--tracking-micro, 0.08em);
		text-transform: uppercase;
		color: var(--bone-100, #f4ece1);
		background: transparent;
		border: 1px solid
			color-mix(in oklab, var(--alarm, #c45947) 50%, var(--bone-400, rgba(255, 255, 255, 0.3)));
		border-radius: var(--radius-pill, 999px);
		cursor: pointer;
		transition:
			background var(--dur-fast, 140ms) var(--ease-out-soft, ease-out),
			border-color var(--dur-fast, 140ms) var(--ease-out-soft, ease-out),
			transform var(--dur-fast, 140ms) var(--ease-out-soft, ease-out);
	}
	.dismiss:hover {
		background: color-mix(in oklab, var(--alarm, #c45947) 22%, transparent);
		border-color: var(--alarm, var(--accent-oxblood, #c45947));
	}
	.dismiss:focus-visible {
		outline: none;
		box-shadow:
			0 0 0 2px var(--ink-000, #000),
			0 0 0 4px var(--alarm, var(--accent-oxblood, #c45947));
	}
	.dismiss:active {
		transform: translateY(1px);
	}
	@keyframes fade-in {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}
	@keyframes card-in {
		from {
			opacity: 0;
			transform: translateY(8px) scale(0.98);
		}
		to {
			opacity: 1;
			transform: translateY(0) scale(1);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.backdrop,
		.card {
			animation: none;
		}
		.dismiss {
			transition: none;
		}
	}
</style>
