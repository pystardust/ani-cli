import { describe, expect, it } from 'vitest';
import * as fc from 'fast-check';
import {
	RECENT_LIMIT,
	cycleSelectedIdx,
	decideEnterAction,
	mergeRecents,
	parseStoredRecents
} from './dropdown';

describe('cycleSelectedIdx', () => {
	it('returns -1 for an empty result list (no selection possible)', () => {
		expect(cycleSelectedIdx(-1, 1, 0)).toBe(-1);
		expect(cycleSelectedIdx(0, -1, 0)).toBe(-1);
	});

	it('forward-cycles within bounds', () => {
		expect(cycleSelectedIdx(-1, 1, 5)).toBe(0); // first ↓ from no-selection lands on idx 0
		expect(cycleSelectedIdx(0, 1, 5)).toBe(1);
		expect(cycleSelectedIdx(3, 1, 5)).toBe(4);
	});

	it('wraps forward past the last index back to 0', () => {
		expect(cycleSelectedIdx(4, 1, 5)).toBe(0);
	});

	it('backward-cycles within bounds', () => {
		expect(cycleSelectedIdx(2, -1, 5)).toBe(1);
		expect(cycleSelectedIdx(1, -1, 5)).toBe(0);
	});

	it('wraps backward past 0 to the last index', () => {
		expect(cycleSelectedIdx(0, -1, 5)).toBe(4);
		// First ↑ from no-selection lands on the last item.
		expect(cycleSelectedIdx(-1, -1, 5)).toBe(4);
	});
});

describe('mergeRecents', () => {
	it('prepends a brand-new query', () => {
		expect(mergeRecents(['demon slayer'], 'jojo')).toEqual(['jojo', 'demon slayer']);
	});

	it('promotes an existing query to the front (no duplicate)', () => {
		expect(mergeRecents(['naruto', 'demon slayer', 'jojo'], 'demon slayer')).toEqual([
			'demon slayer',
			'naruto',
			'jojo'
		]);
	});

	it('caps the result at `max`', () => {
		const existing = ['a', 'b', 'c', 'd', 'e'];
		expect(mergeRecents(existing, 'f', 5)).toEqual(['f', 'a', 'b', 'c', 'd']);
	});

	it('uses the public RECENT_LIMIT default when max is omitted', () => {
		// Document the default — if it changes, this test catches the
		// drift between code and any UI copy that mentions "last 5".
		const existing = Array.from({ length: 10 }, (_, i) => `q${i}`);
		const got = mergeRecents(existing, 'fresh');
		expect(got).toHaveLength(RECENT_LIMIT);
		expect(got[0]).toBe('fresh');
	});
});

describe('parseStoredRecents', () => {
	it('returns [] for null input (no key present)', () => {
		expect(parseStoredRecents(null)).toEqual([]);
	});

	it('returns [] for malformed JSON', () => {
		expect(parseStoredRecents('not-json')).toEqual([]);
	});

	it('returns [] when the payload is not an array', () => {
		expect(parseStoredRecents('{"foo":"bar"}')).toEqual([]);
	});

	it('drops non-string entries defensively', () => {
		expect(parseStoredRecents('["jojo", 42, null, "naruto"]')).toEqual(['jojo', 'naruto']);
	});

	it('caps at max', () => {
		const raw = JSON.stringify(['a', 'b', 'c', 'd', 'e', 'f']);
		expect(parseStoredRecents(raw, 3)).toEqual(['a', 'b', 'c']);
	});
});

describe('decideEnterAction', () => {
	it('navigates to the highlighted result when one is selected', () => {
		expect(decideEnterAction(2, 5, 'whatever')).toEqual({ type: 'navigate-to-hit', idx: 2 });
	});

	it('submits the query when nothing is highlighted but the input has text', () => {
		expect(decideEnterAction(-1, 5, 'jojo')).toEqual({ type: 'submit-query' });
		// A whitespace-only input should still no-op.
		expect(decideEnterAction(-1, 5, '   ')).toEqual({ type: 'noop' });
	});

	it('no-ops on empty input with no selection', () => {
		expect(decideEnterAction(-1, 0, '')).toEqual({ type: 'noop' });
	});

	it('treats out-of-range selectedIdx as no selection', () => {
		// Defensive — a stale selection past the new list length should
		// not navigate to nonsense.
		expect(decideEnterAction(7, 3, 'jojo')).toEqual({ type: 'submit-query' });
	});
});

// — Properties ──────────────────────────────────────────────────────
//
// fast-check exercises the dropdown helpers across randomly-generated
// inputs and shrinks any failure to a minimal counter-example. The
// example-based tests above pin specific scenarios; these document
// invariants the helpers must hold for every input shape.

describe('cycleSelectedIdx (properties)', () => {
	it('result is always a valid index or -1 (never out of range)', () => {
		fc.assert(
			fc.property(
				fc.integer({ min: -1, max: 100 }),
				fc.constantFrom<1 | -1>(1, -1),
				fc.integer({ min: 0, max: 100 }),
				(current, dir, total) => {
					const r = cycleSelectedIdx(current, dir, total);
					return total === 0 ? r === -1 : r >= 0 && r < total;
				}
			)
		);
	});

	it('forward then backward from a valid selection returns to start', () => {
		fc.assert(
			fc.property(
				fc
					.integer({ min: 1, max: 100 })
					.chain((total) => fc.tuple(fc.integer({ min: 0, max: total - 1 }), fc.constant(total))),
				([start, total]) => cycleSelectedIdx(cycleSelectedIdx(start, 1, total), -1, total) === start
			)
		);
	});

	it('from no selection (-1), ↓ lands on 0 and ↑ lands on total-1', () => {
		fc.assert(
			fc.property(fc.integer({ min: 1, max: 100 }), (total) => {
				return (
					cycleSelectedIdx(-1, 1, total) === 0 && cycleSelectedIdx(-1, -1, total) === total - 1
				);
			})
		);
	});
});

describe('mergeRecents (properties)', () => {
	// `mergeRecents` is purely "prepend, dedupe, cap" — the component
	// owns trimming and skipping empty queries before calling. These
	// properties pin the contract at THIS layer, not the one above.
	const anyQuery = fc.string({ minLength: 1, maxLength: 20 });
	const anyExisting = fc.array(fc.string({ minLength: 1, maxLength: 20 }), { maxLength: 20 });

	it('result length never exceeds max', () => {
		fc.assert(
			fc.property(
				anyExisting,
				anyQuery,
				fc.integer({ min: 1, max: 10 }),
				(existing, query, max) => mergeRecents(existing, query, max).length <= max
			)
		);
	});

	it('the query is always the first element of the result', () => {
		fc.assert(
			fc.property(
				anyExisting,
				anyQuery,
				(existing, query) => mergeRecents(existing, query, RECENT_LIMIT)[0] === query
			)
		);
	});

	it('result has no duplicate entries (query is deduped against existing)', () => {
		fc.assert(
			fc.property(anyExisting, anyQuery, (existing, query) => {
				const out = mergeRecents(existing, query, RECENT_LIMIT);
				return new Set(out).size === out.length;
			})
		);
	});

	it('every element of the result either is the query or appeared in existing', () => {
		fc.assert(
			fc.property(anyExisting, anyQuery, (existing, query) => {
				const out = mergeRecents(existing, query, RECENT_LIMIT);
				const existingSet = new Set(existing);
				return out.every((x) => x === query || existingSet.has(x));
			})
		);
	});
});

describe('parseStoredRecents (properties)', () => {
	it('always returns a string array regardless of input shape', () => {
		fc.assert(
			fc.property(fc.anything(), (raw) => {
				const out = parseStoredRecents(JSON.stringify(raw));
				return Array.isArray(out) && out.every((s) => typeof s === 'string');
			})
		);
	});

	it('malformed JSON inputs return an empty array, not throw', () => {
		fc.assert(
			fc.property(fc.string({ maxLength: 50 }), (raw) => {
				const out = parseStoredRecents(raw);
				return Array.isArray(out);
			})
		);
	});
});
