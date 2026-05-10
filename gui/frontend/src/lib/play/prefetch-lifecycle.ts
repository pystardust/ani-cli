/**
 * Decision helpers and registry for the play page's prefetch lifecycle.
 *
 * The play page warms adjacent episodes via play-cache.getOrFire,
 * keyed by show id. Cancellation is gated by `clearForShow`, which
 * normally fires on component destroy.
 *
 * The wrinkle: when the user navigates away with the video still
 * playing in PiP, the play *component* is destroyed but the user is
 * still engaged with the show. Cancelling prefetches on destroy
 * would mean auto-play-next stutters when the next episode boundary
 * hits in the floating thumbnail. So:
 *
 *   • Destroy with no PiP        → clear immediately (legacy path)
 *   • Destroy while in PiP       → defer; let PiP own the lifecycle
 *   • PiP closes while still on  → noop; the remount took ownership
 *     /play/[id] for same show
 *   • PiP closes elsewhere       → clear; user truly disengaged
 *   • Different show mounts on   → fire the deferred cancel eagerly
 *     /play during PiP             so we don't run two shows' prefetches
 *                                  concurrently against the rate limit
 */

export type DestroyPrefetchAction = 'clear-now' | 'defer';

export interface DestroyPrefetchInput {
	/** True when `document.pictureInPictureElement === videoEl` at
	 *  the moment the play component is being destroyed. */
	videoIsInPip: boolean;
}

/** Decide what `onDestroy` should do with the show's in-flight
 *  prefetches. */
export function decideOnDestroyPrefetch(input: DestroyPrefetchInput): DestroyPrefetchAction {
	return input.videoIsInPip ? 'defer' : 'clear-now';
}

export type OrphanedPrefetchAction = 'clear' | 'noop';

export interface OrphanedPrefetchInput {
	/** Show id whose play component was destroyed. */
	destroyedShowId: string;
	/** SvelteKit route id of the page the user is on at decision
	 *  time (when `leavepictureinpicture` fires). */
	currentRouteId: string;
	/** `params.id` of the page the user is on at decision time —
	 *  meaningful only when `currentRouteId === '/play/[id]'`. */
	currentShowId: string;
}

/** When a deferred-cancel listener finally fires (PiP closed),
 *  decide whether to actually clear the prefetches. The answer is
 *  "noop" only when the user is back on /play/[id] for the same
 *  show — that mount has already taken ownership. */
export function decideOnPipCloseOrphanedPrefetch(
	input: OrphanedPrefetchInput
): OrphanedPrefetchAction {
	const remountedSameShow =
		input.currentRouteId === '/play/[id]' && input.currentShowId === input.destroyedShowId;
	return remountedSameShow ? 'noop' : 'clear';
}

/* ----------------------------------------------------------------
 * Deferred-cancel registry
 * ----------------------------------------------------------------
 *
 * When a play page unmounts during PiP, it stores its cancel
 * function here keyed by show id. Two consumers fire entries:
 *
 *   1. The component's own `leavepictureinpicture` listener — runs
 *      its cancel and unregisters itself.
 *   2. `fireDeferredCancelsExcept(currentShowId)`, called from a
 *      *new* play page's mount when the new show differs. This
 *      flushes any lingering cancel for shows the user has clearly
 *      moved on from, so we don't double-prefetch.
 *
 * The registry is module-level singleton state — there's only one
 * PiP host at a time. */
const deferredCancels: Map<string, () => void> = new Map();

/** Store a cancel function for `showId`. A second registration for
 *  the same id replaces the previous entry (the older deferred
 *  cancel is no longer load-bearing). */
export function registerDeferredCancel(showId: string, cancelFn: () => void): void {
	deferredCancels.set(showId, cancelFn);
}

/** Drop the entry for `showId` without firing it. Used when the
 *  component's own listener has just fired — the cancel is already
 *  handled and we just need to clean up the registry. */
export function unregisterDeferredCancel(showId: string): void {
	deferredCancels.delete(showId);
}

/** Fire and drop every deferred-cancel entry whose id differs from
 *  `keepShowId`. Used when a new play page mounts: any show that's
 *  no longer in focus should have its prefetches cancelled now,
 *  not later. */
export function fireDeferredCancelsExcept(keepShowId: string): void {
	for (const [showId, cancel] of [...deferredCancels.entries()]) {
		if (showId === keepShowId) continue;
		deferredCancels.delete(showId);
		try {
			cancel();
		} catch {
			// Cancel functions are user-supplied; never let one
			// throwing prevent the others from firing.
		}
	}
}

/** Test-only: wipe the registry between cases. */
export function __resetDeferredCancelsForTests(): void {
	deferredCancels.clear();
}
