/**
 * Display window for the Skip OP / Skip Outro button. The button
 * was sticking around for the entire OP/ED interval, which
 * cluttered the chrome long after the user had a fair chance to
 * see and click it. Spec: visible for the first 5 seconds of the
 * interval, hidden after. Auto-skip is unaffected — it binds to
 * `pickActiveSkip` so users with the matching toggle still jump
 * past the interval the moment they enter it.
 */
import type { SkipInterval } from '$lib/api';

/** Default visibility window for the Skip button, in seconds. */
export const SKIP_BUTTON_WINDOW_SEC = 5;

/**
 * Whether to render the Skip OP / Skip Outro button at the
 * given playback time for the given active interval.
 *
 * Inclusive at `start_time` (no 1-frame blink as `pickActiveSkip`
 * flips on); exclusive at `start_time + windowSec` (clean fade-out
 * boundary). Returns false when `skip` is null or the playhead is
 * somehow before the interval start (defensive against
 * out-of-sync $derived recomputations during a seek).
 */
export function shouldShowSkipButton(
	skip: SkipInterval | null,
	currentTime: number,
	windowSec: number = SKIP_BUTTON_WINDOW_SEC
): boolean {
	if (!skip) return false;
	const elapsed = currentTime - skip.start_time;
	return elapsed >= 0 && elapsed < windowSec;
}
