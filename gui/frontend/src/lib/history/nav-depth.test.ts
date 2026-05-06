import { describe, expect, it } from 'vitest';
import { nextDepth, shouldShowBackButton, type NavType } from './nav-depth';

const step = (type: NavType, prevDepth: number, stampedDepth: number | null = null) => ({
	type,
	prevDepth,
	stampedDepth
});

describe('nextDepth', () => {
	it("'enter' resets to 0 regardless of prev or stamped depth", () => {
		// Tauri's WebView sometimes preserves window.history.state across
		// app launches; a stamped 5 from yesterday must not bleed into
		// today's first paint.
		expect(nextDepth(step('enter', 5, 5))).toBe(0);
		expect(nextDepth(step('enter', 0, null))).toBe(0);
	});

	it("'link' / 'goto' / 'form' increment depth by one", () => {
		expect(nextDepth(step('link', 0))).toBe(1);
		expect(nextDepth(step('goto', 1))).toBe(2);
		expect(nextDepth(step('form', 2))).toBe(3);
	});

	it("'popstate' uses the new entry's stamped depth when present", () => {
		// Forward through SPA history works for free: each entry carries
		// its own depth, so popstate just reads it back.
		expect(nextDepth(step('popstate', 3, 1))).toBe(1);
		expect(nextDepth(step('popstate', 0, 2))).toBe(2);
	});

	it("'popstate' without a stamped depth decrements (back-press)", () => {
		// The original home entry of a session never gets stamped, so the
		// first back-press from depth 1 should land at 0.
		expect(nextDepth(step('popstate', 1, null))).toBe(0);
		expect(nextDepth(step('popstate', 3, null))).toBe(2);
	});

	it("'popstate' from depth 0 with no stamp clamps at 0", () => {
		// Defensive: never go negative.
		expect(nextDepth(step('popstate', 0, null))).toBe(0);
	});

	it("'leave' and 'replaceState' don't change depth", () => {
		expect(nextDepth(step('leave', 4))).toBe(4);
		expect(nextDepth(step('replaceState', 4))).toBe(4);
	});
});

describe('shouldShowBackButton', () => {
	it('hides at depth 0 (home / fresh launch)', () => {
		expect(shouldShowBackButton(0)).toBe(false);
	});

	it('shows whenever the user has navigated at least once', () => {
		expect(shouldShowBackButton(1)).toBe(true);
		expect(shouldShowBackButton(7)).toBe(true);
	});
});

describe('navigation traces — full sessions', () => {
	// End-to-end sequences modelling typical user flows. They run the
	// pure rules across a series of events and assert depth at each
	// point. These are the "if this regresses, the BackButton bug
	// returns" test cases.

	it('home → /search → back → home: button appears, then hides', () => {
		let depth = 0;
		expect(shouldShowBackButton(depth)).toBe(false);
		depth = nextDepth(step('enter', depth));
		expect(shouldShowBackButton(depth)).toBe(false);
		depth = nextDepth(step('link', depth)); // home → /search
		expect(shouldShowBackButton(depth)).toBe(true);
		depth = nextDepth(step('popstate', depth, 0)); // back → home (stamped 0)
		expect(shouldShowBackButton(depth)).toBe(false);
	});

	it('home → /search → /anime → back → back: stays visible until home', () => {
		let depth = 0;
		depth = nextDepth(step('enter', depth));
		depth = nextDepth(step('link', depth)); // depth 1, /search
		depth = nextDepth(step('link', depth)); // depth 2, /anime
		expect(shouldShowBackButton(depth)).toBe(true);
		depth = nextDepth(step('popstate', depth, 1)); // back to /search
		expect(shouldShowBackButton(depth)).toBe(true);
		depth = nextDepth(step('popstate', depth, null)); // back to home (un-stamped)
		expect(shouldShowBackButton(depth)).toBe(false);
	});
});
