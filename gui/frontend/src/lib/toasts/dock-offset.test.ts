import { describe, expect, it } from 'vitest';
import { computeToastBottomOffset, dockHeightForRows } from './dock-offset';

describe('computeToastBottomOffset', () => {
	// Pure helper: maps (dock visibility, layout constants) → toast
	// `inset-block-end` so the toast rides above the DownloadBar
	// without overlapping when downloads are in flight.
	const layout = { baseRem: 0.75, dockHeightRem: 3.5, gapRem: 0.75 };

	it('returns the base offset when the dock is hidden', () => {
		const got = computeToastBottomOffset({ dockVisible: false, ...layout });
		expect(got).toBe(0.75);
	});

	it('clears the dock when visible: base + dockHeight + gap', () => {
		const got = computeToastBottomOffset({ dockVisible: true, ...layout });
		expect(got).toBe(0.75 + 3.5 + 0.75);
	});

	it('ignores the dock height when dockVisible is false even if non-zero', () => {
		// Defensive: a stale dockHeightRem reading shouldn't push the
		// toast up when the dock isn't actually on-screen.
		const got = computeToastBottomOffset({
			dockVisible: false,
			baseRem: 0.75,
			dockHeightRem: 12,
			gapRem: 1
		});
		expect(got).toBe(0.75);
	});
});

describe('dockHeightForRows', () => {
	// DownloadBar stacks one .dl-bar-row per active download with
	// `gap: var(--space-2)` between rows. Toast offset has to scale
	// with the actual row count or it only clears the first row when
	// a user runs multiple downloads concurrently.
	const layout = { rowRem: 2.6, interRowGapRem: 0.5 };

	it('returns 0 for zero rows (nothing to clear)', () => {
		expect(dockHeightForRows({ rows: 0, ...layout })).toBe(0);
	});

	it('returns the row height for one row (no inter-row gap)', () => {
		expect(dockHeightForRows({ rows: 1, ...layout })).toBe(2.6);
	});

	it('adds one inter-row gap between two rows', () => {
		// 2 rows × 2.6rem + 1 gap × 0.5rem = 5.7rem
		expect(dockHeightForRows({ rows: 2, ...layout })).toBeCloseTo(5.7, 5);
	});

	it('scales linearly with row count', () => {
		// 3 rows: 3 × 2.6 + 2 × 0.5 = 8.8rem
		expect(dockHeightForRows({ rows: 3, ...layout })).toBeCloseTo(8.8, 5);
		// 5 rows: 5 × 2.6 + 4 × 0.5 = 15.0rem
		expect(dockHeightForRows({ rows: 5, ...layout })).toBeCloseTo(15.0, 5);
	});

	it('clamps negative rows to 0 (defensive against stale store reads)', () => {
		// downloadStore.active should never go negative, but a
		// future race-on-startup shouldn't push the toast off-screen.
		expect(dockHeightForRows({ rows: -2, ...layout })).toBe(0);
	});
});
