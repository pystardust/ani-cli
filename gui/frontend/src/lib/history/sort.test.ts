import { describe, expect, it } from 'vitest';
import { sortByWatchedAt } from './sort';
import type { HistoryEntry } from '$lib/api';

const entry = (id: string, ep_no = '1', title = `${id} title`): HistoryEntry => ({
	id,
	ep_no,
	title
});

describe('sortByWatchedAt', () => {
	it('returns the input order when no entries are stamped', () => {
		const a = entry('a');
		const b = entry('b');
		const c = entry('c');
		const got = sortByWatchedAt([a, b, c], {});
		expect(got).toEqual([a, b, c]);
	});

	it('sorts stamped entries descending by timestamp on top', () => {
		const a = entry('a');
		const b = entry('b');
		const c = entry('c');
		const got = sortByWatchedAt([a, b, c], { a: 100, b: 300, c: 200 });
		expect(got.map((e) => e.id)).toEqual(['b', 'c', 'a']);
	});

	it('puts unstamped entries after stamped, preserving their file order', () => {
		// Mixed: stamped (sorted by ts desc) on top, unstamped after
		// in input order. CLI plays don't reach mark-watched, so they
		// are visible but demoted.
		const stale = entry('cli-only-1');
		const recent = entry('gui-recent');
		const older = entry('gui-older');
		const stale2 = entry('cli-only-2');
		const got = sortByWatchedAt([stale, recent, older, stale2], {
			'gui-recent': 1_800_000_000_000,
			'gui-older': 1_700_000_000_000
		});
		expect(got.map((e) => e.id)).toEqual(['gui-recent', 'gui-older', 'cli-only-1', 'cli-only-2']);
	});

	it('treats explicit zero stamps as still-stamped (sort, not filter)', () => {
		// Defensive — a clock-failure backend stamp of `0` shouldn't
		// drop the entry to the unstamped tier; it should sit above
		// any missing entries (since they have no key) and below any
		// real stamps.
		const a = entry('a');
		const b = entry('b');
		const got = sortByWatchedAt([a, b], { a: 0 });
		expect(got.map((e) => e.id)).toEqual(['a', 'b']);
	});
});
