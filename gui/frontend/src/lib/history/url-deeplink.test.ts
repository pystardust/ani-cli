import { describe, expect, it } from 'vitest';
import * as fc from 'fast-check';
import {
	decideEpisodeFetchAction,
	episodesContainEpisode,
	parseEpParam,
	parsePageParam,
	shouldHighlight,
	type EpisodeLike
} from './url-deeplink';

const params = (qs: string) => new URLSearchParams(qs);
const ep = (n: number | null, rel: number | null = null): EpisodeLike => ({
	number: n,
	relative_number: rel
});

describe('parsePageParam', () => {
	it('returns 1 when ?page= is absent', () => {
		expect(parsePageParam(params(''))).toBe(1);
	});

	it('returns the parsed integer for valid ?page=', () => {
		expect(parsePageParam(params('page=2'))).toBe(2);
		expect(parsePageParam(params('page=42'))).toBe(42);
	});

	it('falls back to 1 for non-numeric or non-positive values', () => {
		expect(parsePageParam(params('page=foo'))).toBe(1);
		expect(parsePageParam(params('page=0'))).toBe(1);
		expect(parsePageParam(params('page=-3'))).toBe(1);
	});
});

describe('parseEpParam', () => {
	it('returns null when ?ep= is absent', () => {
		expect(parseEpParam(params(''))).toBeNull();
	});

	it('returns the parsed integer for valid ?ep=', () => {
		expect(parseEpParam(params('ep=4'))).toBe(4);
	});

	it('returns null for non-numeric / zero / negative', () => {
		expect(parseEpParam(params('ep=foo'))).toBeNull();
		expect(parseEpParam(params('ep=0'))).toBeNull();
		expect(parseEpParam(params('ep=-1'))).toBeNull();
	});
});

describe('episodesContainEpisode', () => {
	it('returns false for null episode list', () => {
		expect(episodesContainEpisode(null, 4)).toBe(false);
	});

	it('matches by `number` when present', () => {
		expect(episodesContainEpisode([ep(1), ep(4), ep(5)], 4)).toBe(true);
		expect(episodesContainEpisode([ep(1), ep(2)], 99)).toBe(false);
	});

	it('falls back to `relative_number` when `number` is null', () => {
		// Kitsu split shows: cour-relative numbering lives on
		// relative_number when the parent's `number` is null.
		expect(episodesContainEpisode([ep(null, 4)], 4)).toBe(true);
	});

	it('prefers `number` over `relative_number` when both are set', () => {
		// Edge: an episode with number=10, relative_number=4 should
		// match target=10, not target=4.
		expect(episodesContainEpisode([ep(10, 4)], 10)).toBe(true);
		expect(episodesContainEpisode([ep(10, 4)], 4)).toBe(false);
	});
});

describe('decideEpisodeFetchAction', () => {
	it("returns 'fetch-initial' on the first load (episodes null)", () => {
		const got = decideEpisodeFetchAction({
			episodes: null,
			episodesPage: 1,
			episodesLoading: false,
			targetPage: 1
		});
		expect(got).toBe('fetch-initial');
	});

	it("returns 'fetch-initial' even when targetPage is past 1", () => {
		// e.g. user deep-linked to ?page=2 — we still need a fresh
		// initial fetch, just with a different page number.
		const got = decideEpisodeFetchAction({
			episodes: null,
			episodesPage: 1,
			episodesLoading: false,
			targetPage: 2
		});
		expect(got).toBe('fetch-initial');
	});

	it("returns 'noop' when the target matches the current page", () => {
		const got = decideEpisodeFetchAction({
			episodes: [],
			episodesPage: 2,
			episodesLoading: false,
			targetPage: 2
		});
		expect(got).toBe('noop');
	});

	it("returns 'noop' while a fetch is already in flight", () => {
		// Defends against the effect double-firing on its own state
		// writes (episodesPage transitioning, etc.).
		const got = decideEpisodeFetchAction({
			episodes: [],
			episodesPage: 1,
			episodesLoading: true,
			targetPage: 2
		});
		expect(got).toBe('noop');
	});

	it("returns 'fetch' when the URL target diverges from the current page", () => {
		const got = decideEpisodeFetchAction({
			episodes: [],
			episodesPage: 1,
			episodesLoading: false,
			targetPage: 2
		});
		expect(got).toBe('fetch');
	});
});

describe('shouldHighlight', () => {
	it('returns false when no target ep is set', () => {
		expect(shouldHighlight({ target: null, consumed: null, episodes: [ep(1)] })).toBe(false);
	});

	it('returns false when the target was already consumed', () => {
		// The component flips `consumed = target` after firing once;
		// subsequent re-runs of the effect must no-op.
		expect(shouldHighlight({ target: 4, consumed: 4, episodes: [ep(4)] })).toBe(false);
	});

	it('returns false when the target ep is not in the loaded set', () => {
		// The fetch effect will populate `episodes` shortly; this
		// effect re-runs and fires then.
		expect(shouldHighlight({ target: 16, consumed: null, episodes: [ep(1), ep(2)] })).toBe(false);
	});

	it('returns true when target is set, unconsumed, and present', () => {
		expect(shouldHighlight({ target: 4, consumed: null, episodes: [ep(4)] })).toBe(true);
	});
});

// — Properties ──────────────────────────────────────────────────────
//
// Both URL parsers must coerce *any* user-controlled query-string
// value into the type the downstream effect expects, never throw,
// and never return out-of-range numbers. fast-check exercises the
// space of inputs (numbers, garbage, exotic unicode) and shrinks any
// failure to a minimal counter-example.

describe('parsePageParam (properties)', () => {
	it('returns >= 1 for any input', () => {
		fc.assert(
			fc.property(fc.string({ maxLength: 50 }), (raw) => {
				const sp = new URLSearchParams();
				sp.set('page', raw);
				return parsePageParam(sp) >= 1;
			})
		);
	});

	it('returns the integer value for any positive integer string', () => {
		fc.assert(
			fc.property(fc.integer({ min: 1, max: 1_000_000 }), (n) => {
				const sp = new URLSearchParams();
				sp.set('page', String(n));
				return parsePageParam(sp) === n;
			})
		);
	});

	it('returns 1 for any non-positive integer or non-numeric input', () => {
		fc.assert(
			fc.property(
				fc.oneof(
					fc.integer({ max: 0 }).map(String),
					// Non-numeric strings: anything that fails parseInt to a
					// finite positive number. Bias toward letter-only.
					fc.string({ minLength: 1, maxLength: 10 }).filter((s) => {
						const n = parseInt(s, 10);
						return !(Number.isFinite(n) && n > 0);
					})
				),
				(raw) => {
					const sp = new URLSearchParams();
					sp.set('page', raw);
					return parsePageParam(sp) === 1;
				}
			)
		);
	});
});

describe('parseEpParam (properties)', () => {
	it('returns null or a positive integer — never NaN, zero, or negative', () => {
		fc.assert(
			fc.property(fc.string({ maxLength: 50 }), (raw) => {
				const sp = new URLSearchParams();
				sp.set('ep', raw);
				const r = parseEpParam(sp);
				return r === null || (Number.isFinite(r) && r > 0);
			})
		);
	});

	it('returns the integer value for any positive integer string', () => {
		fc.assert(
			fc.property(fc.integer({ min: 1, max: 100_000 }), (n) => {
				const sp = new URLSearchParams();
				sp.set('ep', String(n));
				return parseEpParam(sp) === n;
			})
		);
	});
});

describe('decideEpisodeFetchAction (properties)', () => {
	it("returns 'fetch-initial' iff episodes is null", () => {
		fc.assert(
			fc.property(
				fc.boolean(),
				fc.integer({ min: 1, max: 10 }),
				fc.boolean(),
				fc.integer({ min: 1, max: 10 }),
				(episodesNull, episodesPage, episodesLoading, targetPage) => {
					const action = decideEpisodeFetchAction({
						episodes: episodesNull ? null : [],
						episodesPage,
						episodesLoading,
						targetPage
					});
					return episodesNull ? action === 'fetch-initial' : action !== 'fetch-initial';
				}
			)
		);
	});

	it("never returns 'fetch' while a fetch is already in flight", () => {
		fc.assert(
			fc.property(
				fc.integer({ min: 1, max: 10 }),
				fc.integer({ min: 1, max: 10 }),
				(episodesPage, targetPage) => {
					const action = decideEpisodeFetchAction({
						episodes: [],
						episodesPage,
						episodesLoading: true,
						targetPage
					});
					return action === 'noop';
				}
			)
		);
	});
});
