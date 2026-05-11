/**
 * Throttle for scrubber-drag live seeks. The custom player scrubber
 * sets `videoEl.currentTime` on every pointermove during a drag
 * (~60 events/s); HLS reacts to each by cancelling the current
 * segment fetch and queuing a new one. Without throttling that
 * thrashes the network and stutters the drag itself on slower
 * connections.
 *
 * Spec: throttle to ~10 seeks/s (100ms minimum interval). Drag
 * visuals still follow the pointer 1:1 via `dragPreviewFraction`;
 * only the underlying video seek is rate-limited. The first seek
 * of a drag is always allowed (`lastSeekAt = null`); the final
 * release seek bypasses this helper entirely so the user always
 * lands exactly where they let go.
 */

import { describe, it, expect } from 'vitest';
import { shouldThrottleSeek, SCRUBBER_SEEK_MIN_INTERVAL_MS } from './seek-throttle';

describe('shouldThrottleSeek', () => {
	it('never throttles the first seek (no prior timestamp)', () => {
		// `null` signals "no seek issued yet this drag." The component
		// resets lastSeekAt to null on pointerdown / pointerup so a
		// new drag always lands its first seek immediately.
		expect(shouldThrottleSeek(null, 1000)).toBe(false);
	});

	it('throttles seeks fired inside the 100ms minimum interval', () => {
		// 60Hz pointermove fires roughly every 16ms; the ~5 events
		// after the first should all be skipped until the window
		// elapses. Pins behaviour for the canonical "user drags
		// quickly" case.
		expect(shouldThrottleSeek(1000, 1016)).toBe(true);
		expect(shouldThrottleSeek(1000, 1050)).toBe(true);
		expect(shouldThrottleSeek(1000, 1099)).toBe(true);
	});

	it('allows a seek exactly at the 100ms boundary', () => {
		// Inclusive at the boundary — a steady-state 10Hz drag with
		// perfect timing must not get every other tick stuck behind
		// the throttle.
		expect(shouldThrottleSeek(1000, 1100)).toBe(false);
	});

	it('allows seeks past the 100ms window', () => {
		expect(shouldThrottleSeek(1000, 1200)).toBe(false);
		expect(shouldThrottleSeek(1000, 5000)).toBe(false);
	});

	it('honours a custom minimum interval', () => {
		// Exposing the parameter keeps the helper composable for
		// callers that want a different rate (e.g. a future
		// non-scrubber consumer).
		expect(shouldThrottleSeek(1000, 1050, 50)).toBe(false);
		expect(shouldThrottleSeek(1000, 1049, 50)).toBe(true);
	});

	it('exports the canonical 100ms interval as a named constant', () => {
		// Pin the value so a future tweak surfaces here instead of
		// quietly changing perf behaviour in the component.
		expect(SCRUBBER_SEEK_MIN_INTERVAL_MS).toBe(100);
	});
});
