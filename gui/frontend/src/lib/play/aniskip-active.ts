/**
 * Pure helper: pick the skip interval the player should show
 * Skip-button UI for at the current playback time. Returns the
 * first matching interval (start_time <= currentTime < end_time)
 * or null if none match.
 *
 * Boundary semantics:
 *   - inclusive at start_time: matching equality avoids a 1-frame
 *     blink when the playhead lands exactly on startTime.
 *   - exclusive at end_time: prevents the button from sticking a
 *     frame past the interval, which would seek to a position the
 *     playhead is already at (no-op click).
 */
import type { SkipInterval } from '$lib/api';

export function pickActiveSkip(
	intervals: SkipInterval[],
	currentTime: number
): SkipInterval | null {
	for (const i of intervals) {
		if (currentTime >= i.start_time && currentTime < i.end_time) {
			return i;
		}
	}
	return null;
}
