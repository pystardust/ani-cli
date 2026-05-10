/**
 * Window logic for the Skip OP / Skip Outro button. The button
 * was sticking around for the entire OP/ED interval, which
 * cluttered the chrome long after the user had a fair chance to
 * see and click it. Spec: visible for the first 5 seconds of the
 * interval, hidden after.
 *
 * Note this is *display only* — the auto-skip effect on the
 * player page still binds to the broader `pickActiveSkip` so a
 * user with auto-skip enabled jumps past the interval the moment
 * they enter it, regardless of where the 5-second display window
 * lands.
 */

import { describe, it, expect } from 'vitest';
import type { SkipInterval } from '$lib/api';
import { shouldShowSkipButton, SKIP_BUTTON_WINDOW_SEC } from './skip-button-window';

const op: SkipInterval = {
	skip_type: 'op',
	start_time: 60,
	end_time: 150
};

describe('shouldShowSkipButton', () => {
	it('returns false when no interval is active', () => {
		expect(shouldShowSkipButton(null, 42)).toBe(false);
	});

	it('returns true at the start of the interval', () => {
		// Inclusive at start so the button doesn't blink off for
		// one frame the moment pickActiveSkip flips it on.
		expect(shouldShowSkipButton(op, 60)).toBe(true);
	});

	it('returns true within the 5-second window', () => {
		expect(shouldShowSkipButton(op, 61)).toBe(true);
		expect(shouldShowSkipButton(op, 64)).toBe(true);
		expect(shouldShowSkipButton(op, 64.999)).toBe(true);
	});

	it('returns false right at the 5-second boundary', () => {
		// Exclusive at the window edge — at exactly 5s in, hide. The
		// alternative (inclusive) feels indecisive: the button's
		// fade-out animation overlaps the user clicking it.
		expect(shouldShowSkipButton(op, 65)).toBe(false);
	});

	it('returns false long after the window has passed but the interval is still active', () => {
		// pickActiveSkip would still return the interval here (we're
		// inside it; auto-skip would still trigger on entry); only
		// the *button* hides.
		expect(shouldShowSkipButton(op, 90)).toBe(false);
		expect(shouldShowSkipButton(op, 149)).toBe(false);
	});

	it('returns false when the playhead is somehow before the interval', () => {
		// pickActiveSkip's contract guarantees this can't happen —
		// it only returns intervals where currentTime >= start_time.
		// But the helper takes both inputs separately and a future
		// caller might pass them out of sync (e.g. during a seek
		// that happens between $derived recomputations); fail closed.
		expect(shouldShowSkipButton(op, 59)).toBe(false);
		expect(shouldShowSkipButton(op, 0)).toBe(false);
	});

	it('honours a custom window length', () => {
		// The default is 5s but exposing it as a parameter keeps the
		// helper composable for future tweaks (per-show windows,
		// user-configurable, etc) without forking it.
		expect(shouldShowSkipButton(op, 62, 3)).toBe(true);
		expect(shouldShowSkipButton(op, 63, 3)).toBe(false);
	});

	it('exports the canonical 5-second window as a named constant', () => {
		// The component imports this constant for any UI that needs
		// to render the timer ("Skip in 5… 4…" hypothetically). Pin
		// the value so a future tweak surfaces here too.
		expect(SKIP_BUTTON_WINDOW_SEC).toBe(5);
	});
});
