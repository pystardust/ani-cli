/**
 * Custom-scrubber drag math. Pinned in a unit test because the
 * happy path (click in the middle of the bar) hides three real
 * edge cases that the live component has to handle correctly:
 *
 *   1. the user starts a drag inside the track but pulls the
 *      pointer outside the player (or past the bar's edge) —
 *      the seek must clamp to 0/1 instead of producing
 *      negative/over-1 fractions that send `currentTime` to
 *      NaN or beyond duration;
 *   2. degenerate layouts (rect.width = 0) — can happen during
 *      a fade-in transition or before the first frame paints —
 *      must not divide by zero;
 *   3. the symmetry the user expects: dragging exactly to the
 *      midpoint of the bar is 0.5, not 0.49999, so the
 *      thumb position visually matches the seek result.
 *
 * The component imports this helper for both `onclick` (instant
 * seek) and `onpointerdown` + `onpointermove` (drag) so click and
 * drag can never disagree about where in the timeline a given
 * clientX maps to.
 */

import { describe, it, expect } from 'vitest';
import { clientXToFraction, displayedScrubFraction } from './scrubber';

describe('clientXToFraction', () => {
	const rect = { left: 100, width: 200 } as const;

	it('returns 0 at the left edge of the track', () => {
		expect(clientXToFraction(100, rect)).toBe(0);
	});

	it('returns 1 at the right edge of the track', () => {
		expect(clientXToFraction(300, rect)).toBe(1);
	});

	it('returns 0.5 exactly at the midpoint', () => {
		// Symmetry matters — if midpoint maps to 0.4999 the thumb
		// snaps a pixel off after a release and the user can see
		// the "wrong" position briefly. Pin exact 0.5.
		expect(clientXToFraction(200, rect)).toBe(0.5);
	});

	it('clamps to 0 when the pointer is left of the bar', () => {
		// Drag started inside, dragged out the left side: stay
		// pinned at 0 instead of producing a negative seek.
		expect(clientXToFraction(50, rect)).toBe(0);
		expect(clientXToFraction(-9999, rect)).toBe(0);
	});

	it('clamps to 1 when the pointer is right of the bar', () => {
		// Same, the other direction. Without clamping, currentTime
		// = fraction * duration would land past `duration` and the
		// `<video>` element handles that by snapping back to 0,
		// which feels broken.
		expect(clientXToFraction(400, rect)).toBe(1);
		expect(clientXToFraction(99999, rect)).toBe(1);
	});

	it('returns 0 when the track has zero width', () => {
		// Defensive: layout transitions can briefly flash a
		// zero-width rect. Don't divide by zero — surface 0 so
		// the seek is a no-op visually until the rect settles.
		expect(clientXToFraction(150, { left: 100, width: 0 })).toBe(0);
	});

	it('always returns a value in [0, 1] for arbitrary inputs', () => {
		// Property-style sanity check — protects against a future
		// "small" tweak that accidentally drops the clamp.
		const samples = [-1000, 0, 50, 100, 150, 200, 250, 300, 500, 1e9];
		for (const x of samples) {
			const f = clientXToFraction(x, rect);
			expect(f).toBeGreaterThanOrEqual(0);
			expect(f).toBeLessThanOrEqual(1);
		}
	});
});

describe('displayedScrubFraction', () => {
	// Why this helper exists: setting `videoEl.currentTime = X` doesn't
	// emit `timeupdate` at every keyframe boundary (especially during
	// rapid seeks on a paused video), so a thumb/fill bound straight
	// to currentTime/duration appears frozen during a drag and only
	// snaps to the new spot on release. This helper lets the
	// component show a `dragPreviewFraction` (updated synchronously
	// on every pointermove) until the drag ends, then fall through
	// to the live currentTime/duration.

	it('uses dragPreviewFraction verbatim when supplied', () => {
		// Drag wins — the user is actively scrubbing and they expect
		// the thumb to track their pointer 1:1, not the underlying
		// video's seek progress.
		expect(displayedScrubFraction(0.42, 30, 100)).toBe(0.42);
	});

	it('falls back to currentTime/duration when no drag is active', () => {
		// Normal playback path: dragPreviewFraction is null because
		// the user isn't holding the thumb. The live time wins.
		expect(displayedScrubFraction(null, 30, 100)).toBe(0.3);
	});

	it('returns 0 when duration is zero or non-finite', () => {
		// Pre-loadedmetadata: `duration === NaN` or 0. Don't divide
		// by zero — surface 0 so the bar renders empty, not infinite.
		expect(displayedScrubFraction(null, 0, 0)).toBe(0);
		expect(displayedScrubFraction(null, 30, NaN)).toBe(0);
		expect(displayedScrubFraction(null, 30, -5)).toBe(0);
	});

	it('clamps the fallback to [0, 1] when currentTime exceeds duration', () => {
		// Defensive: the player can briefly report currentTime past
		// duration during ended/seeking edges. A fill > 100% looks
		// glitchy, so clamp at 1.
		expect(displayedScrubFraction(null, 110, 100)).toBe(1);
		expect(displayedScrubFraction(null, -5, 100)).toBe(0);
	});

	it('clamps an out-of-range drag preview too', () => {
		// dragPreviewFraction *should* always be in [0, 1] from
		// clientXToFraction, but this is the same belt-and-braces
		// clamp as the fallback path — a future caller passing a
		// raw value can't render a glitchy out-of-range bar.
		expect(displayedScrubFraction(1.5, 30, 100)).toBe(1);
		expect(displayedScrubFraction(-0.2, 30, 100)).toBe(0);
	});
});
