import { describe, expect, it } from 'vitest';
import { ACCENT_PALETTE, accentFor } from './accent';

describe('accentFor', () => {
	it('returns a value from the palette for a non-empty id', () => {
		const a = accentFor('kitsu-12');
		expect(ACCENT_PALETTE).toContain(a);
	});

	it('is deterministic for the same id (stable across calls)', () => {
		expect(accentFor('one-piece-12')).toBe(accentFor('one-piece-12'));
	});

	it('produces different colors for different ids (hash spread sanity check)', () => {
		// djb2 over 32 distinct ids should hit at least 2 of 8 palette
		// slots — anything tighter would mean a hash collision storm.
		const seen = new Set<string>();
		for (let i = 0; i < 32; i++) seen.add(accentFor(`id-${i}`));
		expect(seen.size).toBeGreaterThan(1);
	});

	it('falls back to a fixed palette slot for the empty id', () => {
		// The empty-id branch is hit when a card renders before its
		// metadata is populated (e.g. skeleton loading). The picker
		// returns a deterministic accent so the placeholder is stable.
		expect(accentFor('')).toBe(ACCENT_PALETTE[1]);
	});
});
