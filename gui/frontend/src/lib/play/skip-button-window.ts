/**
 * Display window for the Skip OP / Skip Outro button. The button
 * was sticking around for the entire OP/ED interval, which
 * cluttered the chrome long after the user had a fair chance to
 * see and click it. Spec: visible for the first 5 seconds of the
 * interval, hidden after.
 *
 * Stubbed; real impl lands with the green commit.
 */
import type { SkipInterval } from '$lib/api';

/** Default visibility window for the Skip button, in seconds.
 *  Stubbed at 0 so the const-pin test fails red. Green sets it
 *  to the intended 5. */
export const SKIP_BUTTON_WINDOW_SEC = 0;

/**
 * Whether to render the Skip OP / Skip Outro button at the
 * given playback time for the given active interval.
 *
 * Stub throws; the green commit replaces the body with the real
 * boundary math (`elapsed = currentTime - skip.start_time`,
 * inclusive at 0, exclusive at `windowSec`).
 */
export function shouldShowSkipButton(
	skip: SkipInterval | null,
	currentTime: number,
	windowSec: number = SKIP_BUTTON_WINDOW_SEC
): boolean {
	void skip;
	void currentTime;
	void windowSec;
	throw new Error('not yet implemented — green commit fills this in');
}
