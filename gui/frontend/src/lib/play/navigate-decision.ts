/**
 * Pure decision helper for what the play page's `beforeNavigate` hook
 * should do with the singleton `<video>` when the user routes away.
 *
 * Extracted so the four-way branching ("same-show swap", "already
 * paused", "already in PiP", "auto-PiP gated by config") can be unit-
 * tested as a single switch on inputs, instead of being threaded
 * through Svelte's effect system. A bug in this branching is what
 * caused the "I leave the page and hear audio with no video"
 * regression — the test for it lives next to this file.
 */

/** Inputs the play page's `beforeNavigate` collects from `to`, the
 *  singleton video, and the runtime config. */
export interface NavigateDecisionInput {
	/** SvelteKit route id of the navigation target (`/play/[id]`,
	 *  `/anime/[id]`, `/`, …). Empty string for cancelled navigations. */
	targetRoute: string;
	/** `params.id` of the navigation target (only meaningful for
	 *  `/play/[id]` and `/anime/[id]` routes). */
	targetShowId: string;
	/** Show id the play page is currently rendering. */
	currentShowId: string;
	/** `videoEl.paused` snapshot. When true there's nothing to keep
	 *  alive — neither PiP nor an explicit pause makes sense. */
	videoPaused: boolean;
	/** True when `document.pictureInPictureElement` already points at
	 *  the singleton — the user manually PiP'd before clicking away.
	 *  We don't issue a duplicate request. */
	alreadyInPip: boolean;
	/** Settings flag: when true, navigation pauses instead of
	 *  requesting PiP. Inverted polarity ("disable") so the default
	 *  (false) keeps auto-PiP on without user opt-in. */
	disableAutoPip: boolean;
}

/** What the caller should do with the singleton:
 *
 *   • `noop`        — leave it alone (same-show swap, already paused
 *                     or already in PiP)
 *   • `request-pip` — call `requestPictureInPicture()`; if it rejects,
 *                     fall back to pausing
 *   • `pause`       — pause the singleton so audio stops on leave */
export type NavigateAction = 'noop' | 'request-pip' | 'pause';

/** Compute the navigation action. The mapping:
 *
 *   ┌─────────────────────────────────────────────────────────────┐
 *   │ Same show on /play/[id]?                  → noop            │
 *   │ Already paused?                           → noop            │
 *   │ Already in PiP?                           → noop            │
 *   │ Auto-PiP disabled in settings?            → pause           │
 *   │ Otherwise                                 → request-pip     │
 *   └─────────────────────────────────────────────────────────────┘
 */
export function decideNavigateAction(input: NavigateDecisionInput): NavigateAction {
	const sameShowSwap =
		input.targetRoute === '/play/[id]' && input.targetShowId === input.currentShowId;
	if (sameShowSwap) return 'noop';
	if (input.videoPaused) return 'noop';
	if (input.alreadyInPip) return 'noop';
	if (input.disableAutoPip) return 'pause';
	return 'request-pip';
}
