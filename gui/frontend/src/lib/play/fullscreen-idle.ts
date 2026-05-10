/**
 * Fullscreen idle-hide for the player controls + cursor. See the
 * companion .test.ts file for the spec the component implements.
 *
 * Stubbed; real impl lands with the green commit.
 */

/** ms of mouse inactivity before fullscreen controls + cursor hide. */
export const FULLSCREEN_IDLE_HIDE_MS = 0;

/** Whether to hide controls (and cursor) in fullscreen given the
 *  current keep-alive state. */
export function shouldHideControlsInFullscreen(state: {
	mouseIdle: boolean;
	paused: boolean;
	scrubberHover: boolean;
	focusWithin: boolean;
}): boolean {
	void state;
	throw new Error('not yet implemented — green commit fills this in');
}
