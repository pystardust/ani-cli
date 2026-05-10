import { describe, it, expect } from 'vitest';
import { decideLeavePipAction, type LeavePipDecisionInput } from './leave-pip-decision';

const base: LeavePipDecisionInput = {
	lastPauseAtMs: 0,
	leftAtMs: 1000
};

describe('decideLeavePipAction', () => {
	it('stays when pause fired immediately before leave (X close)', () => {
		// Chromium's PiP X button calls video.pause() synchronously
		// then exits PiP, so the pause event lands < 100 ms before
		// leavepictureinpicture. That tight window identifies the
		// X-close path; we want to stay where the user is.
		expect(decideLeavePipAction({ lastPauseAtMs: 1000, leftAtMs: 1010 })).toBe('stay');
	});

	it('stays when pause and leave fire on the same tick', () => {
		// Synchronous ordering — pause() and leavepictureinpicture
		// can land on the exact same Date.now() reading.
		expect(decideLeavePipAction({ lastPauseAtMs: 1000, leftAtMs: 1000 })).toBe('stay');
	});

	it('navigates when no pause has fired in this PiP session', () => {
		// User clicked "return to tab" while the video was still
		// playing. lastPauseAtMs stays at the sentinel 0.
		expect(decideLeavePipAction({ ...base })).toBe('navigate');
	});

	it('navigates when the most recent pause was long ago', () => {
		// User paused manually mid-session, watched the still frame
		// for a while, then clicked return-to-tab. The pause event
		// is far enough in the past that we treat the leave as
		// return-to-tab, not X-close.
		expect(decideLeavePipAction({ lastPauseAtMs: 100, leftAtMs: 5000 })).toBe('navigate');
	});

	it('navigates exactly at the 100 ms boundary', () => {
		// 100 ms is the cutoff. Boundary lands on the navigate side
		// to avoid clipping a slow X-close on a busy machine without
		// pulling in honest return-to-tab edges.
		expect(decideLeavePipAction({ lastPauseAtMs: 1000, leftAtMs: 1100 })).toBe('navigate');
	});

	it('navigates when leftAtMs is somehow before lastPauseAtMs', () => {
		// Defensive: if monotonic-clock weirdness produces a negative
		// delta, fall through to navigate rather than treating it as
		// "very recent pause".
		expect(decideLeavePipAction({ lastPauseAtMs: 2000, leftAtMs: 1000 })).toBe('navigate');
	});
});
