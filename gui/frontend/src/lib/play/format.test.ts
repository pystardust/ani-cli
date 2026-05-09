import { describe, expect, it } from 'vitest';
import { formatTime, progressLabel, skipLabel } from './format';

describe('progressLabel', () => {
	it('returns the banner text for banner events (used as the loading-overlay headline)', () => {
		expect(progressLabel({ kind: 'banner', text: 'Searching allmanga…' })).toBe(
			'Searching allmanga…'
		);
	});

	it('annotates the provider name with a checkmark for links_fetched', () => {
		// `links_fetched` fires once per provider as the scraper
		// makes progress; the ✓ tells the user "this provider
		// answered, moving on."
		expect(progressLabel({ kind: 'links_fetched', provider: 'wixmp' })).toBe('wixmp ✓');
	});

	it('passes through "other" event text verbatim', () => {
		expect(progressLabel({ kind: 'other', text: 'whatever' })).toBe('whatever');
	});
});

describe('skipLabel', () => {
	it('maps the three known skip types to user-facing copy', () => {
		expect(skipLabel('op')).toBe('Skip Opening');
		expect(skipLabel('ed')).toBe('Skip Ending');
		expect(skipLabel('recap')).toBe('Skip Recap');
	});

	it('falls back to a generic "Skip" for unknown / mixed types', () => {
		// aniskip surfaces "mixed-op" / "mixed-ed" too — those keep
		// the button usable but unbranded so we don't confidently
		// mislabel an OP as an outro.
		expect(skipLabel('mixed-op')).toBe('Skip');
		expect(skipLabel('mixed-ed')).toBe('Skip');
		expect(skipLabel('')).toBe('Skip');
	});
});

describe('formatTime', () => {
	it('formats seconds as M:SS for under-an-hour timestamps', () => {
		expect(formatTime(0)).toBe('0:00');
		expect(formatTime(7)).toBe('0:07');
		expect(formatTime(65)).toBe('1:05');
		expect(formatTime(599)).toBe('9:59');
	});

	it('formats seconds as H:MM:SS once the hour mark crosses', () => {
		// Pad the minutes to two digits so the column doesn't wobble
		// between sub-hour and hour-plus durations.
		expect(formatTime(3600)).toBe('1:00:00');
		expect(formatTime(3661)).toBe('1:01:01');
		expect(formatTime(3725)).toBe('1:02:05');
	});

	it('returns 0:00 for non-finite or negative inputs (handles uninitialized <video>.duration)', () => {
		// `<video>.duration` is NaN until metadata loads; the player
		// chrome would render "NaN:NaN" without this guard.
		expect(formatTime(NaN)).toBe('0:00');
		expect(formatTime(Infinity)).toBe('0:00');
		expect(formatTime(-1)).toBe('0:00');
	});

	it('floors fractional seconds (no rounding up across the next-second boundary)', () => {
		// `<video>.currentTime` is a float; consistently rounding
		// down keeps the displayed time monotonic with the timeline.
		expect(formatTime(59.9)).toBe('0:59');
		expect(formatTime(60.0001)).toBe('1:00');
	});
});
