/**
 * Pure decision helper for what the layout's `leavepictureinpicture`
 * handler should do when the PiP window closes.
 *
 * Two ways the user can close PiP:
 *
 *   • **X button** (close in place). The W3C PiP spec requires the
 *     UA to pause the video before exiting, so the `pause` event
 *     fires synchronously immediately before
 *     `leavepictureinpicture`. We want to leave the user where
 *     they are — they hit X to dismiss the floating thumbnail, not
 *     to be teleported back to the play page.
 *
 *   • **Return-to-tab** (the icon in the centre of the PiP frame).
 *     The spec says the video keeps its current playing/paused
 *     state — no pause from the UA. The user explicitly asked to
 *     come back to the player, so we navigate to the play page.
 *
 * The discriminator is the time delta between the most recent
 * `pause` event and the `leavepictureinpicture` event. Within a
 * tight window (< 100 ms) we treat the leave as X-close. Anything
 * older — including a manual pause earlier in the session — is
 * return-to-tab.
 *
 * Edge case: user pauses manually then immediately clicks
 * return-to-tab within 100 ms. Misclassified as X-close. The
 * sequence is unusual; the alternative (no discrimination) breaks
 * the much more common "X means close, period."
 */

export interface LeavePipDecisionInput {
	/** Time of the most recent `pause` event on the singleton video,
	 *  in milliseconds since the epoch. 0 if no pause has fired in
	 *  this PiP session. */
	lastPauseAtMs: number;
	/** Time the `leavepictureinpicture` event fired, in milliseconds
	 *  since the epoch. */
	leftAtMs: number;
}

export type LeavePipAction = 'stay' | 'navigate';

/** Tight window after a pause event during which we attribute a
 *  subsequent leavepictureinpicture to the X button. */
const X_CLOSE_PAUSE_WINDOW_MS = 100;

/** Compute whether the leave handler should keep the user in place
 *  (X-close) or navigate them back to the play page (return-to-tab). */
export function decideLeavePipAction(input: LeavePipDecisionInput): LeavePipAction {
	const sinceLastPause = input.leftAtMs - input.lastPauseAtMs;
	if (sinceLastPause >= 0 && sinceLastPause < X_CLOSE_PAUSE_WINDOW_MS) {
		return 'stay';
	}
	return 'navigate';
}
