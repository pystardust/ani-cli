/**
 * Throttle for the custom player scrubber's live drag seeks.
 *
 * `onScrubberPointerMove` updates the visual thumb on every pointer
 * event but the underlying `videoEl.currentTime` write is rate-
 * limited via this helper. The thumb stays 1:1 with the pointer
 * because it binds to `dragPreviewFraction`; only the network-side
 * HLS segment fetches get coalesced.
 */

/** Minimum gap between two issued seeks during a single drag. */
export const SCRUBBER_SEEK_MIN_INTERVAL_MS = 100;

/**
 * Whether the next seek call should be skipped to honour the
 * minimum-interval window since the last issued seek.
 *
 * - `lastSeekAt = null` → first seek of a drag, never throttled.
 *   The component resets to `null` on pointerdown and pointerup so
 *   each drag's opening (and the post-release reset) lands a real
 *   seek immediately.
 * - `now - lastSeekAt < minIntervalMs` → throttle (skip this one,
 *   the visual still updates).
 * - `now - lastSeekAt >= minIntervalMs` → allow the seek and let
 *   the caller update lastSeekAt.
 *
 * Pure with respect to time — caller passes `now` in. Tests use
 * deterministic numbers; production passes `performance.now()`.
 */
export function shouldThrottleSeek(
	lastSeekAt: number | null,
	now: number,
	minIntervalMs: number = SCRUBBER_SEEK_MIN_INTERVAL_MS
): boolean {
	if (lastSeekAt === null) return false;
	return now - lastSeekAt < minIntervalMs;
}
