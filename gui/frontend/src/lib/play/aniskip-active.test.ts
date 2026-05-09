/**
 * Pure logic for picking the active skip interval at a given
 * playback time. The Player calls this on every `timeupdate` —
 * if the result is non-null it renders a Skip OP / Skip Outro
 * button overlay; clicking the button seeks to `end_time`.
 *
 * The helper has no DOM or async surface; the player owns that.
 * Keeping it pure makes the boundary semantics testable: edge
 * cases at the start / end of an interval, multiple
 * non-overlapping intervals, empty input.
 */
import { describe, it, expect } from 'vitest';
import type { SkipInterval } from '$lib/api';
import { pickActiveSkip } from './aniskip-active';

const op: SkipInterval = { skip_type: 'op', start_time: 1.0, end_time: 90.0 };
const ed: SkipInterval = { skip_type: 'ed', start_time: 1325.0, end_time: 1440.0 };

describe('pickActiveSkip', () => {
	it('returns null on empty interval list', () => {
		expect(pickActiveSkip([], 30)).toBeNull();
	});

	it('returns the interval when currentTime is inside it', () => {
		expect(pickActiveSkip([op, ed], 45)).toEqual(op);
		expect(pickActiveSkip([op, ed], 1400)).toEqual(ed);
	});

	it('matches at the inclusive start boundary', () => {
		// currentTime equal to startTime should still be considered
		// inside — otherwise the button blinks for a frame at boundary.
		expect(pickActiveSkip([op], op.start_time)).toEqual(op);
	});

	it('does not match at the exclusive end boundary', () => {
		// currentTime equal to endTime should NOT match — otherwise
		// the button stays a frame past the interval and the seek
		// click would be a no-op.
		expect(pickActiveSkip([op], op.end_time)).toBeNull();
	});

	it('returns null when currentTime is between intervals', () => {
		expect(pickActiveSkip([op, ed], 600)).toBeNull();
	});

	it('returns the first matching interval if more than one would match', () => {
		// Pathological data — overlapping intervals shouldn't happen
		// from aniskip, but if they do, the renderer needs stable
		// behavior (no random flicker between two buttons).
		const overlap: SkipInterval = { skip_type: 'mixed-op', start_time: 50, end_time: 100 };
		expect(pickActiveSkip([op, overlap], 60)).toEqual(op);
	});
});
