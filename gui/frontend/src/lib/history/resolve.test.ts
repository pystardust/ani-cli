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

describe('resolveHistoryEntry — display vs search title', () => {
	it('strips the trailing "(N episodes)" parenthetical from displayTitle', () => {
		const r = resolveHistoryEntry(entry('Demon Slayer (26 episodes)', '5'), stubKitsu());
		expect(r.displayTitle).toBe('Demon Slayer');
		// Title-search query is the same — no cour suffix to strip.
		expect(r.searchTitle).toBe('Demon Slayer');
	});

	it('keeps the cour suffix on both display and search', () => {
		// Empirically Kitsu often stores multi-cour shows as separate
		// entries (Part 1, Part 2, …). Stripping the suffix collapsed
		// Stone Ocean Part 2 onto Part 1's 12-episode page, breaking
		// navigation. Verbatim search lets Kitsu pick the matching
		// per-cour entry when one exists; when it doesn't, both cours
		// land on the same Kitsu page and the cards stay distinct via
		// displayTitle alone.
		const r = resolveHistoryEntry(
			entry('JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2 (12 episodes)', '4'),
			stubKitsu()
		);
		expect(r.displayTitle).toBe('JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2');
		expect(r.searchTitle).toBe('JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2');
	});
});

describe('resolveHistoryEntry — direct episode mapping', () => {
	it('maps episode number directly through to Kitsu', () => {
		const r = resolveHistoryEntry(entry('Demon Slayer (26 episodes)', '5'), stubKitsu());
		expect(r.displayEpisode).toBe(5);
		expect(r.kitsuEpisode).toBe(5);
		expect(r.uiPage).toBe(1);
		expect(r.mappingNote).toBe('direct');
	});

	it('lands on the right UI page when the episode is past page 1', () => {
		const r = resolveHistoryEntry(entry('One Piece (1100 episodes)', '54'), stubKitsu());
		expect(r.kitsuEpisode).toBe(54);
		expect(r.uiPage).toBe(Math.ceil(54 / 12));
	});

	it('still records cour metadata for callers that want to surface it', () => {
		const r = resolveHistoryEntry(
			entry('JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2 (12 episodes)', '4'),
			stubKitsu()
		);
		// kitsuEpisode is direct (4), but the resolver still reports
		// cour=2 + courSize=12 so future UI can show a "Part 2" badge.
		expect(r.cour).toBe(2);
		expect(r.courSize).toBe(12);
		expect(r.kitsuEpisode).toBe(4);
		expect(r.uiPage).toBe(1);
	});

	it('does not treat mid-title "Part N" as a cour disambiguator', () => {
		const r = resolveHistoryEntry(
			entry('JoJo no Kimyou na Bouken Part 6: Stone Ocean (12 episodes)', '4'),
			stubKitsu()
		);
		expect(r.cour).toBe(1);
		expect(r.kitsuEpisode).toBe(4);
		expect(r.uiPage).toBe(1);
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
	it('emits ep when a kitsu episode is set', () => {
		const r = resolveHistoryEntry(entry('Show (12 episodes)', '4'), stubKitsu());
		expect(resumeQueryString(r)).toBe('?ep=4');
	});

	it('emits page when the target ep is past UI page 1', () => {
		const r = resolveHistoryEntry(entry('Show (200 episodes)', '54'), stubKitsu());
		expect(resumeQueryString(r)).toBe(`?page=${Math.ceil(54 / 12)}&ep=54`);
	});

	it('returns empty string when neither page nor ep is meaningful', () => {
		const r = resolveHistoryEntry(entry('Show (12 episodes)', '0'), null);
		expect(resumeQueryString(r)).toBe('');
	});
});
