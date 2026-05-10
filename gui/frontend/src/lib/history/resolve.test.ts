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
	it('prefers the Kitsu canonical title for displayTitle when match present', () => {
		// Continue Watching used to show the romanized history title
		// while the detail page rendered Kitsu's canonical (English for
		// some shows). The user clicked into Stone Ocean and saw two
		// different names for the same show. With a Kitsu match, the
		// resolver routes the canonical onto displayTitle so the two
		// surfaces agree. Cours stay distinct via Kitsu's per-cour
		// canonical (`Stone Ocean Part 2` is its own Kitsu entry) and
		// the EP number — no Part badge needed.
		const r = resolveHistoryEntry(
			entry('Jojo no Kimyou na Bouken Part 6: Stone Ocean (38 episodes)', '4'),
			{ ...stubKitsu(), canonical_title: "JoJo's Bizarre Adventure: Stone Ocean" }
		);
		expect(r.displayTitle).toBe("JoJo's Bizarre Adventure: Stone Ocean");
	});

	it('strips the trailing "(N episodes)" parenthetical when no Kitsu match', () => {
		const r = resolveHistoryEntry(entry('Demon Slayer (26 episodes)', '5'), null);
		// No Kitsu match → fall back to the stripped history title.
		expect(r.displayTitle).toBe('Demon Slayer');
		// searchTitle still derives from the history entry regardless of
		// whether a match is present — it's the input to a *future* Kitsu
		// search, not a label.
		expect(r.searchTitle).toBe('Demon Slayer');
	});

	it('keeps the cour suffix on searchTitle (still drives Kitsu search)', () => {
		// Empirically Kitsu often stores multi-cour shows as separate
		// entries (Part 1, Part 2, …). Stripping the suffix collapsed
		// Stone Ocean Part 2 onto Part 1's 12-episode page, breaking
		// navigation. Verbatim search lets Kitsu pick the matching
		// per-cour entry when one exists.
		const r = resolveHistoryEntry(
			entry('JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2 (12 episodes)', '4'),
			null
		);
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

	const titledCountedHit = (
		id: string,
		canonical_title: string,
		episode_count: number | null
	): KitsuAnimeRef => ({
		...stubKitsu(id),
		canonical_title,
		episode_count
	});

	it('rejects a fuzzy first hit whose episode count is wildly off', () => {
		// Burichi -, episode 2, 366-episode show. Kitsu's text search
		// for "Burichi -" returns Doraemon Movie 14: Nobita to Buriki
		// no Labyrinth (1 episode) as the first hit because it
		// fuzzy-matches "Buriki". Without the episode-count filter the
		// picker locks onto Doraemon, persists the title-match cache,
		// and Continue Watching renders the wrong show forever. With
		// the filter Doraemon is rejected (1 vs 366 — way outside
		// tolerance) and the picker falls through; the alias
		// enrichment path (englishName "Bleach") then resolves
		// correctly.
		const r = resolveHistoryEntry(entry('Burichi - (366 episodes)', '2'), null);
		const hits = [
			titledCountedHit('doraemon14', 'Doraemon Movie 14: Nobita to Buriki no Labyrinth', 1),
			titledCountedHit('chichibu', 'Chichibu de Buchichi', 1)
		];
		expect(pickKitsuMatch(hits, r)).toBeNull();
	});

	it('passes hits whose episode count is close to courSize', () => {
		// Off-by-one episode-count differences are common (Kitsu
		// counts a recap, allmanga doesn't). Accept those.
		const r = resolveHistoryEntry(entry('Bleach (366 episodes)', '2'), null);
		const hits = [
			titledCountedHit('right', 'BLEACH', 367),
			titledCountedHit('wrong', 'Doraemon Movie 14', 1)
		];
		expect(pickKitsuMatch(hits, r)?.id).toBe('right');
	});

	it('does not filter when courSize is unknown (legacy hsts row)', () => {
		// Older history entries have no "(N episodes)" parenthetical,
		// so courSize is null. We can't filter — preserve the existing
		// fall-back-to-first-hit behaviour.
		const r = resolveHistoryEntry(entry('Some Show', '1'), null);
		const hits = [
			titledCountedHit('first', 'Some Show', 1),
			titledCountedHit('second', 'Some Show: Movie', 1)
		];
		expect(pickKitsuMatch(hits, r)?.id).toBe('first');
	});

	it('does not filter when a hit has null episode_count', () => {
		// Kitsu's `episodeCount` is null for ongoing shows that have
		// not declared a final count. We can't reject those — they
		// might be the right match. Pass null-count hits through to
		// the existing heuristics.
		const r = resolveHistoryEntry(entry('Ongoing Show (12 episodes)', '1'), null);
		const hits = [titledCountedHit('ongoing', 'Ongoing Show', null)];
		expect(pickKitsuMatch(hits, r)?.id).toBe('ongoing');
	});

	it('accepts within ±25% tolerance for long-running shows', () => {
		// Long shows (Naruto, One Piece) have wide allmanga ↔ Kitsu
		// drift because Kitsu sometimes splits filler arcs into
		// separate entries. 220 vs 200 should still match.
		const r = resolveHistoryEntry(entry('Naruto (220 episodes)', '1'), null);
		const hits = [
			titledCountedHit('naruto', 'Naruto', 200),
			titledCountedHit('movie', 'Naruto Movie 1', 1)
		];
		expect(pickKitsuMatch(hits, r)?.id).toBe('naruto');
	});

	it('accepts within ±5 absolute tolerance for short shows', () => {
		// Single-cour drift: 12 vs 13 (one extra special) is common.
		// Tolerance must allow it without making the long-show
		// percentage too tight.
		const r = resolveHistoryEntry(entry('Tiny Show (12 episodes)', '1'), null);
		const hits = [
			titledCountedHit('thirteen', 'Tiny Show', 13),
			titledCountedHit('one', 'Tiny Show Movie', 1)
		];
		expect(pickKitsuMatch(hits, r)?.id).toBe('thirteen');
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
