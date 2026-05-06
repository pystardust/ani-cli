import { describe, expect, it } from 'vitest';
import { nextHeroIndex, shouldRunHeroRotation } from './hero-rotation';

describe('nextHeroIndex', () => {
	it('advances within bounds', () => {
		expect(nextHeroIndex(0, 3)).toBe(1);
		expect(nextHeroIndex(1, 3)).toBe(2);
	});

	it('wraps at the end of the rotation', () => {
		expect(nextHeroIndex(2, 3)).toBe(0);
	});

	it('returns 0 for an empty rotation (defensive)', () => {
		// Caller might call us with total=0 if the trending row hasn't
		// loaded yet. Don't blow up; let the next render figure it out.
		expect(nextHeroIndex(0, 0)).toBe(0);
		expect(nextHeroIndex(7, 0)).toBe(0);
	});
});

describe('shouldRunHeroRotation', () => {
	const ok = {
		rotationLength: 3,
		paused: false,
		prefersReducedMotion: false
	};

	it('runs in the happy path', () => {
		expect(shouldRunHeroRotation(ok)).toBe(true);
	});

	it('does not run for a single-item rotation', () => {
		// Nothing to rotate to.
		expect(shouldRunHeroRotation({ ...ok, rotationLength: 1 })).toBe(false);
		expect(shouldRunHeroRotation({ ...ok, rotationLength: 0 })).toBe(false);
	});

	it('does not run while paused (hover / focus held)', () => {
		expect(shouldRunHeroRotation({ ...ok, paused: true })).toBe(false);
	});

	it('does not run when the user prefers reduced motion', () => {
		expect(shouldRunHeroRotation({ ...ok, prefersReducedMotion: true })).toBe(false);
	});
});
