<!--
  ToastHost — bottom-right anchored container for the toast stack.
  Mounted once in +layout.svelte after DownloadBar so toasts stack
  in DOM order above the dock.

  Dock-aware offset: when DownloadBar is on-screen (active
  downloads AND download_bottom_bar_enabled), the host lifts its
  bottom anchor so toasts don't overlap. Pure offset math lives in
  $lib/toasts/dock-offset.ts and is unit-tested.

  Animation: each row uses Svelte's `fly` with the same easing as
  DownloadBar so the two surfaces feel coordinated when both
  appear at once.
-->
<script lang="ts">
	import { fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import Toast from './Toast.svelte';
	import { toastStore } from '$lib/toasts/store.svelte';
	import { downloadStore } from '$lib/download/store.svelte';
	import { computeToastBottomOffset, dockHeightForRows } from '$lib/toasts/dock-offset';

	// Calibrated against DownloadBar's `.dl-bar-row` geometry — see
	// dock-offset.ts. One row ≈ 2.6rem (padding-block + progress
	// strip + caption line-box + 1px border × 2); the dl-bar's flex
	// `gap: var(--space-2)` is 0.5rem between rows.
	const DOCK_ROW_REM = 2.6;
	const DOCK_INTER_ROW_GAP_REM = 0.5;

	let { downloadBarEnabled = true } = $props<{
		/** Mirrors the layout's `config.download_bottom_bar_enabled`.
		 *  When `false`, downloads are surfaced only through the topbar
		 *  dock and the bottom progress strip never appears — so the
		 *  toast can sit at its base offset regardless of in-flight
		 *  downloads. Defaults to `true` for parity with the config
		 *  default. */
		downloadBarEnabled?: boolean;
	}>();

	const dockRows = $derived(downloadBarEnabled ? downloadStore.active.length : 0);
	const dockVisible = $derived(dockRows > 0);
	const dockHeightRem = $derived(
		dockHeightForRows({
			rows: dockRows,
			rowRem: DOCK_ROW_REM,
			interRowGapRem: DOCK_INTER_ROW_GAP_REM
		})
	);
	const offsetRem = $derived(
		computeToastBottomOffset({
			dockVisible,
			baseRem: 0.75,
			dockHeightRem,
			gapRem: 0.75
		})
	);
</script>

<aside class="toast-host" aria-live="polite" style:--toast-bottom-offset="{offsetRem}rem">
	{#each toastStore.items as toast (toast.id)}
		<div class="toast-slot" transition:fly={{ y: 16, duration: 220, easing: cubicOut }}>
			<Toast
				kind={toast.kind}
				message={toast.message}
				actionLabel={toast.actionLabel}
				onAction={toast.onAction}
				onDismiss={() => toastStore.dismiss(toast.id)}
			/>
		</div>
	{/each}
</aside>

<style>
	.toast-host {
		position: fixed;
		inset-block-end: var(--toast-bottom-offset, 0.75rem);
		inset-inline-end: var(--space-5, 1.5rem);
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: var(--space-2, 0.5rem);
		z-index: 50;
		pointer-events: none;
		transition: inset-block-end var(--dur-med, 260ms) var(--ease-out-soft, ease-out);
	}
	.toast-slot {
		pointer-events: auto;
	}
</style>
