import { describe, expect, it } from 'vitest';
import { defaultRangeOnEnter } from './default-range';

describe('defaultRangeOnEnter', () => {
	it('returns 1..maxEpisode when episode count is known', () => {
		// On a 13-episode show, clicking Range should default to the
		// full season — that's the most-likely intent and lets the user
		// narrow either bound from a sensible starting point. The
		// pre-fix bug defaulted to clicked-ep..clicked-ep (13..13 on
		// episode 13), which forced the user to retype both fields.
		expect(defaultRangeOnEnter(13, 10)).toEqual({ start: 1, end: 13 });
	});

	it('returns 1..rangeFallbackCap when episode count is unknown', () => {
		// Currently-airing shows or OVAs sometimes lack a known count
		// (Kitsu hasn't indexed them or didn't announce). The cap is
		// the same fallback DownloadConfirm uses for rangeMax — keeps
		// a stray 200 in the To input from kicking off a runaway loop.
		expect(defaultRangeOnEnter(null, 10)).toEqual({ start: 1, end: 10 });
	});

	it('returns 1..1 for a single-episode show', () => {
		// Range and This collapse to the same episode arg here, but
		// Range should still render with a sane value rather than 0..0
		// or empty.
		expect(defaultRangeOnEnter(1, 10)).toEqual({ start: 1, end: 1 });
	});

	it('respects the cap value passed in', () => {
		// Belt-and-braces: the helper should not hardcode the fallback;
		// the caller decides. Passing a different cap should propagate
		// to the end value when maxEpisode is null.
		expect(defaultRangeOnEnter(null, 25)).toEqual({ start: 1, end: 25 });
	});
});
