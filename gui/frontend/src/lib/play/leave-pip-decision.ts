/**
 * Pure decision helper for what the layout's `leavepictureinpicture`
 * handler should do when the PiP window closes.
 *
 * Two ways the user can close PiP:
 *
 *   • **X button** (close in place). The W3C PiP spec requires the
 *     UA to pause the video synchronously as part of the close
 *     path. The user dismissed the floating thumbnail — leave them
 *     where they are.
 *
 *   • **Return-to-tab** (the icon in the centre of the PiP frame).
 *     The spec keeps playback state intact — no UA-issued pause.
 *     The user explicitly asked to come back, so navigate to the
 *     play page.
 *
 * `videoPaused` alone isn't enough: a user who paused the video
 * mid-PiP and then clicks return-to-tab also has paused=true at
 * leave time, but their intent is to navigate. The discriminator
 * is the *recency* of the pause: an X-close pause lands within
 * milliseconds of leave; a manual mid-PiP pause is typically
 * seconds old.
 *
 * Edge case: user pauses manually, then clicks return-to-tab
 * within the X-close window (~100 ms). Misclassified as X-close —
 * unusual fast-fingers sequence, accepted as the price of getting
 * the common cases right.
 */

export interface LeavePipDecisionInput {
	/** `videoEl.paused` snapshot, read after a one-tick defer so any
	 *  spec-mandated UA pause has had time to settle. */
	videoPaused: boolean;
	/** Milliseconds between the most recent `pause` event on the
	 *  singleton video and the leave decision. `Number.POSITIVE_INFINITY`
	 *  if no pause has fired in this PiP session. */
	msSincePauseEvent: number;
}

export type LeavePipAction = 'stay' | 'navigate';

/** Tight window during which a pause event is attributed to the UA's
 *  X-close path rather than a user-initiated mid-PiP pause. */
export const X_CLOSE_PAUSE_WINDOW_MS = 100;

/** Compute whether the leave handler should keep the user in place
 *  (X-close) or navigate them back to the play page (return-to-tab). */
export function decideLeavePipAction(input: LeavePipDecisionInput): LeavePipAction {
	if (input.videoPaused && input.msSincePauseEvent < X_CLOSE_PAUSE_WINDOW_MS) {
		return 'stay';
	}
	return 'navigate';
}
