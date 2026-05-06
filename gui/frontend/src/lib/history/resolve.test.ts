import { describe, expect, it } from 'vitest';
import { resolveHistoryEntry, resumeQueryString } from './resolve';
import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';

const stubKitsu = (id = '13'): KitsuAnimeRef => ({
	id,
	canonical_title: 'Stub',
	slug: null,
	synopsis: null,
	start_date: null,
	end_date: null,
	episode_count: null,
	average_rating: null,
	subtype: null,
	status: null,
	age_rating: null,
	popularity_rank: null,
	poster_image: null,
	cover_image: null
});

const entry = (title: string, ep_no = '1'): HistoryEntry => ({
	id: 'allmanga-id',
	ep_no,
	title
});

describe('resolveHistoryEntry — single-cour shows', () => {
	it('maps directly when there is no Part/Cour/Season suffix', () => {
		const r = resolveHistoryEntry(entry('Demon Slayer (26 episodes)', '5'), stubKitsu());
		expect(r.displayTitle).toBe('Demon Slayer');
		expect(r.displayEpisode).toBe(5);
		expect(r.cour).toBe(1);
		expect(r.kitsuEpisode).toBe(5);
		expect(r.uiPage).toBe(1);
		expect(r.mappingNote).toBe('direct');
	});

	it('lands on the right UI page when the episode is past page 1', () => {
		const r = resolveHistoryEntry(entry('One Piece (1100 episodes)', '54'), stubKitsu());
		expect(r.uiPage).toBe(Math.ceil(54 / 12));
		expect(r.kitsuEpisode).toBe(54);
	});

	it('does not treat mid-title "Part N" as a cour disambiguator', () => {
		// "JoJo … Part 6: Stone Ocean" — the "Part 6" refers to the
		// JoJo series, not the cour. Trailing-anchored regex skips it.
		const r = resolveHistoryEntry(
			entry('JoJo no Kimyou na Bouken Part 6: Stone Ocean (12 episodes)', '4'),
			stubKitsu()
		);
		expect(r.cour).toBe(1);
		expect(r.kitsuEpisode).toBe(4);
		expect(r.uiPage).toBe(1);
		expect(r.mappingNote).toBe('direct');
		// Single-cour shows: searchTitle equals displayTitle.
		expect(r.searchTitle).toBe(r.displayTitle);
	});
});

describe('resolveHistoryEntry — multi-cour shows split across allmanga entries', () => {
	it('offsets a Part-2 ep 4 to the right Kitsu episode (Stone Ocean case)', () => {
		const r = resolveHistoryEntry(
			entry('JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2 (12 episodes)', '4'),
			stubKitsu()
		);
		expect(r.cour).toBe(2);
		expect(r.courSize).toBe(12);
		expect(r.displayEpisode).toBe(4);
		expect(r.kitsuEpisode).toBe(16); // (2-1) * 12 + 4
		expect(r.uiPage).toBe(2); // ceil(16 / 12)
		expect(r.mappingNote).toBe('cour-offset-suffix');
		// displayTitle keeps Part 2 so the card disambiguates from the
		// other Stone Ocean row; searchTitle drops it so Kitsu's text
		// search hits the same parent anime as Stone Ocean (Part 1).
		expect(r.displayTitle).toBe('JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2');
		expect(r.searchTitle).toBe('JoJo no Kimyou na Bouken Part 6: Stone Ocean');
	});

	it('handles Part 3 the same way', () => {
		const r = resolveHistoryEntry(
			entry('JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 3 (12 episodes)', '4'),
			stubKitsu()
		);
		expect(r.kitsuEpisode).toBe(28); // (3-1) * 12 + 4
		expect(r.uiPage).toBe(3);
	});

	it('detects "Cour 2" the same as "Part 2"', () => {
		const r = resolveHistoryEntry(entry('Some Anime Cour 2 (12 episodes)', '3'), stubKitsu());
		expect(r.cour).toBe(2);
		expect(r.kitsuEpisode).toBe(15);
	});

	it('detects "Season 2" the same as "Part 2"', () => {
		const r = resolveHistoryEntry(entry('Some Anime Season 2 (12 episodes)', '3'), stubKitsu());
		expect(r.cour).toBe(2);
		expect(r.kitsuEpisode).toBe(15);
	});

	it('falls back to direct mapping when cour size is unknown', () => {
		// Older ani-cli formats may omit the "(N episodes)" tail.
		const r = resolveHistoryEntry(entry('Some Anime Part 2', '3'), stubKitsu());
		expect(r.cour).toBe(2);
		expect(r.courSize).toBeNull();
		expect(r.kitsuEpisode).toBe(3); // best-effort
		expect(r.mappingNote).toBe('no-cour-detected');
	});
});

describe('resolveHistoryEntry — no Kitsu match', () => {
	it('returns a usable target without kitsu fields', () => {
		const r = resolveHistoryEntry(entry('Obscure Anime (12 episodes)', '5'), null);
		expect(r.kitsuId).toBeNull();
		expect(r.kitsuEpisode).toBeNull();
		expect(r.uiPage).toBe(1);
		expect(r.displayTitle).toBe('Obscure Anime');
		expect(r.displayEpisode).toBe(5);
		expect(r.mappingNote).toBe('no-kitsu-match');
	});
});

describe('resolveHistoryEntry — episode-number quirks', () => {
	it('takes the head of a "1-12" range', () => {
		const r = resolveHistoryEntry(entry('Show (12 episodes)', '1-12'), stubKitsu());
		expect(r.displayEpisode).toBe(1);
	});

	it('falls back to 1 when ep_no is not numeric', () => {
		const r = resolveHistoryEntry(entry('Show (12 episodes)', 'foo'), stubKitsu());
		expect(r.displayEpisode).toBe(1);
	});
});

describe('resumeQueryString', () => {
	it('omits page when uiPage is 1', () => {
		const r = resolveHistoryEntry(entry('Show (12 episodes)', '4'), stubKitsu());
		expect(resumeQueryString(r)).toBe('?ep=4');
	});

	it('includes both page and ep when paginated', () => {
		const r = resolveHistoryEntry(entry('Show Part 2 (12 episodes)', '4'), stubKitsu());
		expect(resumeQueryString(r)).toBe('?page=2&ep=16');
	});

	it('returns empty string when neither page nor ep is meaningful', () => {
		const r = resolveHistoryEntry(entry('Show (12 episodes)', '0'), null);
		// kitsuEpisode null + uiPage 1 — nothing to deep-link.
		expect(resumeQueryString(r)).toBe('');
	});
});
