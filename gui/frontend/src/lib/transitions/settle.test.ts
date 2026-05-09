import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { settle, settleOut } from './settle';

// `window` doesn't exist under vitest's `node` env. Stub a minimal
// shape so reducedMotion()'s `typeof window !== 'undefined'` branch
// can be exercised both ways.
function withReducedMotion<T>(matches: boolean, fn: () => T): T {
	const original = (globalThis as unknown as { window?: unknown }).window;
	(globalThis as unknown as { window: unknown }).window = {
		matchMedia: () => ({ matches })
	};
	try {
		return fn();
	} finally {
		if (original === undefined) {
			delete (globalThis as unknown as { window?: unknown }).window;
		} else {
			(globalThis as unknown as { window: unknown }).window = original;
		}
	}
}

const dummyNode = {} as Element;

describe('settle (entrance transition)', () => {
	beforeEach(() => {
		// Tear down the stub between tests so each starts from the
		// real env state.
		delete (globalThis as unknown as { window?: unknown }).window;
	});
	afterEach(() => {
		delete (globalThis as unknown as { window?: unknown }).window;
	});

	it('uses default delay 0 + duration 620 when no opts are given', () => {
		const r = withReducedMotion(false, () => settle(dummyNode));
		expect(r.delay).toBe(0);
		expect(r.duration).toBe(620);
	});

	it('honours delay and duration overrides', () => {
		const r = withReducedMotion(false, () => settle(dummyNode, { delay: 80, duration: 400 }));
		expect(r.delay).toBe(80);
		expect(r.duration).toBe(400);
	});

	it('starts at opacity 0 + scale 0.9 + 28px down + 8px blur (t=0)', () => {
		// css(t=0, u=1) is the entrance starting frame. Asserting the
		// boundary values pins the gesture's intent in case anyone
		// tweaks the curve and accidentally drops the blur or shifts
		// the starting offset.
		const r = withReducedMotion(false, () => settle(dummyNode));
		const css = r.css(0, 1);
		expect(css).toContain('opacity: 0');
		expect(css).toContain('translateY(28px)');
		expect(css).toContain('scale(0.9)');
		expect(css).toContain('blur(8px)');
	});

	it('rests at opacity 1 + scale 1 + 0px + 0 blur (t=1)', () => {
		const r = withReducedMotion(false, () => settle(dummyNode));
		const css = r.css(1, 0);
		expect(css).toContain('opacity: 1');
		expect(css).toContain('translateY(0px)');
		// 0.9 + 1*0.1 = 1
		expect(css).toContain('scale(1)');
		expect(css).toContain('blur(0px)');
	});

	it('drops the transform / blur and zeros the duration under reduced motion', () => {
		// Prefers-reduced-motion users skip the parallax + blur and
		// land on a plain opacity fade with duration 0 (instant).
		const r = withReducedMotion(true, () => settle(dummyNode));
		expect(r.duration).toBe(0);
		const css = r.css(0.5, 0.5);
		expect(css).toBe('opacity: 0.5;');
		expect(css).not.toContain('transform');
		expect(css).not.toContain('blur');
	});

	it('handles missing window object (SSR boot) by treating as full motion', () => {
		// reducedMotion() returns false when `window` is undefined,
		// so the full curve is used.
		const r = settle(dummyNode);
		expect(r.duration).toBe(620);
		expect(r.css(0, 1)).toContain('translateY(28px)');
	});
});

describe('settleOut (exit transition)', () => {
	afterEach(() => {
		delete (globalThis as unknown as { window?: unknown }).window;
	});

	it('uses default delay 0 + duration 320', () => {
		// Shorter than settle's 620 — old tiles clear before new
		// ones finish landing.
		const r = withReducedMotion(false, () => settleOut(dummyNode));
		expect(r.delay).toBe(0);
		expect(r.duration).toBe(320);
	});

	it('drops upward (negative translateY) and uses milder blur than settle', () => {
		// settleOut mirrors the curve but shifts by -16px and 4px
		// blur — distinct from settle's +28 / 8 so the exit feels
		// like a different gesture.
		const r = withReducedMotion(false, () => settleOut(dummyNode));
		const css = r.css(0, 1);
		expect(css).toContain('translateY(-16px)');
		expect(css).toContain('blur(4px)');
	});

	it('rests at full opacity / scale 1 / no offset / no blur (t=1)', () => {
		const r = withReducedMotion(false, () => settleOut(dummyNode));
		const css = r.css(1, 0);
		expect(css).toContain('opacity: 1');
		// `${0 * -16}px` stringifies to `0px`, not `-0px`.
		expect(css).toContain('translateY(0px)');
		// 0.94 + 1*0.06 = 1
		expect(css).toContain('scale(1)');
		expect(css).toContain('blur(0px)');
	});

	it('flat fade under reduced motion', () => {
		const r = withReducedMotion(true, () => settleOut(dummyNode));
		expect(r.duration).toBe(0);
		const css = r.css(0.3, 0.7);
		expect(css).toBe('opacity: 0.3;');
	});
});
