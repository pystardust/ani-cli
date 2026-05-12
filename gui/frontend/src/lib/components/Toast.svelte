<!--
  Toast — a single ephemeral notification rendered by ToastHost.
  Thin presentational component: the toastStore owns the row's
  lifecycle (auto-dismiss timer, stack cap); this component only
  knows how to draw one card and how to surface user dismiss /
  action clicks.

  Kind drives the left accent bar's colour, mapped to the per-anime
  accents tokens: success → jade, info → ink, warning → ochre,
  error → oxblood. ErrorOverlay reuses the same oxblood for the
  error tier — failures stay consistent across modal + toast.
-->
<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import type { ToastKind } from '$lib/toasts/store.svelte';

	let { kind, message, actionLabel, onAction, onDismiss } = $props<{
		kind: ToastKind;
		message: string;
		actionLabel?: string | null;
		onAction?: (() => void) | null;
		onDismiss: () => void;
	}>();

	const hasAction = $derived(
		typeof actionLabel === 'string' && actionLabel.length > 0 && typeof onAction === 'function'
	);

	function handleAction() {
		// Action runs first so the parent can capture any state it
		// needs before the toast disappears. Then dismiss — most
		// action callbacks (Retry, Open settings) are "do the thing
		// AND clear the toast" semantics.
		onAction?.();
		onDismiss();
	}
</script>

<div class="toast" data-kind={kind} role="status" aria-live="polite">
	<span class="bar" aria-hidden="true"></span>
	<span class="message">{message}</span>
	{#if hasAction}
		<button type="button" class="action" onclick={handleAction}>
			{actionLabel}
		</button>
	{/if}
	<button
		type="button"
		class="dismiss"
		aria-label={m.toast_dismiss_aria()}
		onclick={() => onDismiss()}
	>
		<span aria-hidden="true">×</span>
	</button>
</div>

<style>
	.toast {
		--toast-accent: var(--accent-ink);
		position: relative;
		display: inline-flex;
		align-items: center;
		gap: var(--space-3, 0.75rem);
		min-inline-size: 18rem;
		max-inline-size: 26rem;
		padding-block: var(--space-3, 0.75rem);
		padding-inline: calc(var(--space-3, 0.75rem) + 4px) var(--space-3, 0.75rem);
		background: color-mix(in oklab, var(--toast-accent) 8%, var(--ink-100, #181814));
		color: var(--bone-100, #f4ece1);
		border: 1px solid
			color-mix(in oklab, var(--toast-accent) 28%, var(--ink-200, rgba(255, 255, 255, 0.12)));
		border-radius: var(--radius-card, 8px);
		box-shadow: var(--shadow-card-hover, 0 12px 24px -8px rgb(0 0 0 / 0.55));
		pointer-events: auto;
	}
	.toast[data-kind='success'] {
		--toast-accent: var(--accent-jade);
	}
	.toast[data-kind='info'] {
		--toast-accent: var(--accent-ink);
	}
	.toast[data-kind='warning'] {
		--toast-accent: var(--accent-ochre);
	}
	.toast[data-kind='error'] {
		--toast-accent: var(--accent-oxblood);
	}
	.bar {
		position: absolute;
		inset-block: 0;
		inset-inline-start: 0;
		inline-size: 4px;
		background: var(--toast-accent);
		border-start-start-radius: var(--radius-card, 8px);
		border-end-start-radius: var(--radius-card, 8px);
	}
	.message {
		flex: 1 1 auto;
		font-family: var(--font-body, system-ui);
		font-size: var(--type-meta, 0.875rem);
		font-weight: 500;
		line-height: 1.4;
	}
	.action {
		flex: 0 0 auto;
		padding: 0.4rem 0.85rem;
		font-family: var(--font-mono, monospace);
		font-size: var(--type-micro, 0.75rem);
		font-weight: 600;
		letter-spacing: var(--tracking-micro, 0.08em);
		text-transform: uppercase;
		color: var(--bone-100, #f4ece1);
		background: color-mix(in oklab, var(--toast-accent) 35%, transparent);
		border: 1px solid color-mix(in oklab, var(--toast-accent) 50%, transparent);
		border-radius: var(--radius-pill, 999px);
		cursor: pointer;
	}
	.action:hover {
		background: color-mix(in oklab, var(--toast-accent) 50%, transparent);
	}
	.action:focus-visible {
		outline: 2px solid var(--toast-accent);
		outline-offset: 2px;
	}
	.dismiss {
		flex: 0 0 auto;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		inline-size: 1.5rem;
		block-size: 1.5rem;
		padding: 0;
		font-family: var(--font-body, system-ui);
		font-size: 1.1rem;
		line-height: 1;
		color: var(--bone-300, #b6ad9f);
		background: transparent;
		border: none;
		border-radius: var(--radius-pill, 999px);
		cursor: pointer;
	}
	.dismiss:hover {
		color: var(--bone-100, #f4ece1);
		background: color-mix(in oklab, var(--bone-100, #f4ece1) 8%, transparent);
	}
	.dismiss:focus-visible {
		outline: 2px solid var(--toast-accent);
		outline-offset: 2px;
	}
</style>
