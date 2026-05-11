/**
 * Drag-generation guard for the scrubber's release-time callbacks.
 *
 * The scrubber's `onScrubberPointerUp` schedules two clears of
 * `dragPreviewFraction`: a `seeked` listener and a 500 ms safety
 * timeout. If the user releases and re-grabs the scrubber before
 * either fires, the stale callbacks would clobber the new drag's
 * preview state, causing a one-frame thumb snap mid-drag.
 *
 * Each pointerdown bumps a `dragGeneration` counter. The schedulers
 * capture the counter's current value; the callbacks read it back
 * when they fire and consult this helper to decide whether they're
 * still authoritative.
 */

import { describe, it, expect } from 'vitest';
import { isStaleDragCallback } from './drag-generation';

describe('isStaleDragCallback', () => {
	it('returns false when the captured generation matches the live one', () => {
		// Same drag still in flight (no new pointerdown bumped the
		// counter). Callback should apply its clear normally.
		expect(isStaleDragCallback(0, 0)).toBe(false);
		expect(isStaleDragCallback(7, 7)).toBe(false);
	});

	it('returns true when a new pointerdown has bumped the live generation', () => {
		// Drag 1 released, drag 2 already started before drag 1's
		// `seeked`/safety fired. The captured value is 1, the live
		// value is 2. The callback is stale; it must no-op so it
		// doesn't clobber drag 2's `dragPreviewFraction`.
		expect(isStaleDragCallback(1, 2)).toBe(true);
		// Larger jumps (rapid re-drags) also count as stale.
		expect(isStaleDragCallback(1, 5)).toBe(true);
	});

	it('treats a live-generation rollback as stale (defensive)', () => {
		// Live counter can't logically go backwards (pointerdown only
		// increments), but the predicate is symmetric: any mismatch
		// is stale. Pins the contract so a future refactor doesn't
		// quietly switch to `scheduled > current`-style asymmetric
		// logic that breaks on the wraparound edge.
		expect(isStaleDragCallback(5, 4)).toBe(true);
	});
});
