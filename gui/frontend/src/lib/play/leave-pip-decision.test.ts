import { describe, it, expect } from 'vitest';
import { decideLeavePipAction } from './leave-pip-decision';

describe('decideLeavePipAction', () => {
	it('stays when paused and the pause was very recent (X-close on playing video)', () => {
		// X-close path: UA paused the video right around the leave
		// event; we see the pause within ~ms of the decision tick.
		expect(decideLeavePipAction({ videoPaused: true, msSincePauseEvent: 5 })).toBe('stay');
	});

	it('navigates when playing at decision time (return-to-tab on playing)', () => {
		// Spec: return-to-tab keeps playback state intact.
		expect(decideLeavePipAction({ videoPaused: false, msSincePauseEvent: 5 })).toBe('navigate');
	});

	it('navigates when paused but the pause was long ago (return-to-tab on user-paused video)', () => {
		// User paused manually mid-PiP, then clicked return-to-tab.
		// videoPaused=true but the pause event is far in the past —
		// not the UA's X-close pause, so navigate.
		expect(decideLeavePipAction({ videoPaused: true, msSincePauseEvent: 5000 })).toBe('navigate');
	});

	it('navigates when no pause has fired (return-to-tab on a video that started paused)', () => {
		// Sentinel: msSincePauseEvent = Infinity means no pause
		// events have landed in this PiP session.
		expect(
			decideLeavePipAction({
				videoPaused: false,
				msSincePauseEvent: Number.POSITIVE_INFINITY
			})
		).toBe('navigate');
	});

	it('navigates exactly at the 100 ms boundary', () => {
		// Boundary lands on the navigate side: a slow X-close on a
		// busy machine that drifts past 100 ms is rare enough that
		// we'd rather not pull honest return-to-tab clicks across.
		expect(decideLeavePipAction({ videoPaused: true, msSincePauseEvent: 100 })).toBe('navigate');
	});
});
