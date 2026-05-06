import { describe, expect, it } from 'vitest';
import { pickKitsuMatch, resolveHistoryEntry, resumeQueryString } from './resolve';
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

describe('pickKitsuMatch', () => {
	const titledHit = (id: string, canonical_title: string): KitsuAnimeRef => ({
		...stubKitsu(id),
		canonical_title
	});

	it('returns null when there are no hits', () => {
		const r = resolveHistoryEntry(entry('Stone Ocean Part 2 (12 episodes)', '4'), null);
		expect(pickKitsuMatch([], r)).toBeNull();
	});

	it('returns the first hit for single-cour entries', () => {
		const r = resolveHistoryEntry(entry('Demon Slayer (26 episodes)', '5'), null);
		const hits = [titledHit('1', 'Demon Slayer'), titledHit('2', 'Demon Slayer: Mugen Train')];
		expect(pickKitsuMatch(hits, r)?.id).toBe('1');
	});

	it('prefers the hit whose title carries the same cour suffix', () => {
		// Real-world Stone Ocean: searching "…Stone Ocean Part 2"
		// returns Part 1 first because Part 1 is more popular. We
		// post-filter for the cour-matching entry.
		const r = resolveHistoryEntry(
			entry('JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2 (12 episodes)', '4'),
			null
		);
		const hits = [
			titledHit('part1', 'JoJo no Kimyou na Bouken Part 6: Stone Ocean'),
			titledHit('part2', 'JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2')
		];
		expect(pickKitsuMatch(hits, r)?.id).toBe('part2');
	});

	const slugHit = (id: string, slug: string, canonical_title = 'Localized'): KitsuAnimeRef => ({
		...stubKitsu(id),
		slug,
		canonical_title
	});

	it('matches by slug when the canonical_title is in a non-Latin script', () => {
		// Kitsu sometimes returns the Japanese title as canonical and
		// only the slug carries the romanized "part-2". Slug-based
		// matching is the reliable signal in that case.
		const r = resolveHistoryEntry(
			entry('JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2 (12 episodes)', '4'),
			null
		);
		const hits = [
			slugHit('p1', 'jojo-no-kimyou-na-bouken-part-6-stone-ocean', 'ストーンオーシャン'),
			slugHit(
				'p2',
				'jojo-no-kimyou-na-bouken-part-6-stone-ocean-part-2',
				'ストーンオーシャン パート2'
			)
		];
		expect(pickKitsuMatch(hits, r)?.id).toBe('p2');
	});

	it('exact-slug match wins over cour heuristics', () => {
		const r = resolveHistoryEntry(entry('JoJo Stone Ocean Part 2 (12 episodes)', '4'), null);
		const hits = [
			slugHit('mid', 'jojo-stone-ocean-part-2', 'something else'),
			slugHit('cour', 'unrelated', 'JoJo Stone Ocean Part 2 — alt')
		];
		// Slug-derived "jojo-stone-ocean-part-2" matches `mid` exactly.
		expect(pickKitsuMatch(hits, r)?.id).toBe('mid');
	});

	it('does not false-match the parent series number ("Part 6" in JoJo)', () => {
		// Searching for cour 2, the only "Part 6" in any title is
		// the JoJo series number — should not be picked.
		const r = resolveHistoryEntry(
			entry('JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2 (12 episodes)', '4'),
			null
		);
		const hits = [titledHit('s1', 'JoJo no Kimyou na Bouken Part 6: Stone Ocean')];
		expect(pickKitsuMatch(hits, r)?.id).toBe('s1'); // falls back to first hit
	});

	it('falls back to the first hit when no cour-tagged title is in the result set', () => {
		const r = resolveHistoryEntry(entry('Stone Ocean Part 3 (12 episodes)', '1'), null);
		const hits = [titledHit('p1', 'Stone Ocean'), titledHit('p2', 'Stone Ocean Part 2')];
		expect(pickKitsuMatch(hits, r)?.id).toBe('p1');
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
