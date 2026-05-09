import { describe, expect, it } from 'vitest';
import { describeError, describePlayFailure } from './error-copy';

describe('describeError', () => {
	it('formats AniError envelopes as "<kind>: <detail>"', () => {
		expect(describeError({ kind: 'scraper', detail: 'no_results' })).toBe('scraper: no_results');
	});

	it('falls back to just the kind when detail is missing', () => {
		// The Rust backend's serializer omits `detail` for variants
		// that don't carry one (Timeout, Network, etc.). Make sure
		// those still render usefully.
		expect(describeError({ kind: 'timeout' })).toBe('timeout');
	});

	it('passes through other thrown values via String()', () => {
		expect(describeError(new Error('boom'))).toBe('Error: boom');
		expect(describeError('plain string')).toBe('plain string');
		expect(describeError(42)).toBe('42');
		expect(describeError(null)).toBe('null');
		expect(describeError(undefined)).toBe('undefined');
	});

	it('ignores non-string kind / detail fields (defensive — backends sometimes drift)', () => {
		// Numeric kind: not the AniError shape; fall through to
		// String(e) which gives `[object Object]`. The user never
		// sees this raw — describePlayFailure pattern-matches on
		// the lowercase output and lands on the generic message.
		expect(describeError({ kind: 1, detail: 'x' })).toBe('[object Object]');
	});
});

describe('describePlayFailure', () => {
	it('matches the no_results branch', () => {
		expect(describePlayFailure({ kind: 'scraper', detail: 'no_results' })).toMatch(
			/Couldn't find this title/
		);
	});

	it('matches the scraper branch when no_results is not present', () => {
		expect(describePlayFailure({ kind: 'scraper', detail: 'allmanga 503' })).toMatch(
			/streaming source looks unhappy/
		);
	});

	it('matches the timeout branch', () => {
		expect(describePlayFailure({ kind: 'timeout' })).toMatch(/took too long to respond/);
	});

	it('matches the network branch on either kind', () => {
		expect(describePlayFailure({ kind: 'network' })).toMatch(/Network trouble/);
		expect(describePlayFailure({ kind: 'upstream', detail: '503' })).toMatch(/Network trouble/);
	});

	it('falls back to a generic retry message for unrecognized errors', () => {
		// A plain Error or an unexpected shape lands here. The copy
		// stays optimistic — "try again" — because the most common
		// real-world cause is a transient hiccup the user hasn't
		// seen before.
		expect(describePlayFailure(new Error('something weird'))).toMatch(
			/Couldn't start this episode right now/
		);
		expect(describePlayFailure({ unexpected: true })).toMatch(
			/Couldn't start this episode right now/
		);
	});

	it('treats no_results case-insensitively (backend may shift casing)', () => {
		// describeError lowercases before matching, so an upstream
		// that emits "NO_RESULTS" still hits the catalogue-miss
		// branch.
		expect(describePlayFailure({ kind: 'NO_RESULTS' })).toMatch(/Couldn't find this title/);
	});
});
