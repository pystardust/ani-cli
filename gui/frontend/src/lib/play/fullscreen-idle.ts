/**
 * Fullscreen idle-hide for the player controls + cursor. See the
 * companion .test.ts file for the spec the component implements.
 *
 * Outside fullscreen the existing CSS `.player-frame:hover` rule
 * is enough; in fullscreen the frame *is* the screen, so :hover
 * stays true and the controls never go away. The component layer
 * tracks mouse motion + pause + scrubber-hover + focus-within
 * and asks this helper for a boolean it maps to a class on
 * `.player-frame`. CSS reads the class and overrides :hover plus
 * sets `cursor: none`.
 */

/** ms of mouse inactivity before fullscreen controls + cursor hide. */
export const FULLSCREEN_IDLE_HIDE_MS = 2500;

/** Whether to hide controls (and cursor) in fullscreen given the
 *  current keep-alive state. Idle hides only when no other reason
 *  is keeping the chrome live: any of paused / scrubber-hover /
 *  focus-within / recent mouse motion wins over idle. */
export function shouldHideControlsInFullscreen(state: {
	mouseIdle: boolean;
	paused: boolean;
	scrubberHover: boolean;
	focusWithin: boolean;
}): boolean {
	if (!state.mouseIdle) return false;
	if (state.paused) return false;
	if (state.scrubberHover || state.focusWithin) return false;
	return true;
}
