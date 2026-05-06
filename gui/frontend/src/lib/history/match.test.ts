import { beforeEach, describe, expect, it, vi } from 'vitest';
import { resolveKitsuMatch } from './match';
import { resolveHistoryEntry } from './resolve';
import {
	kitsuAnimeBySlug,
	kitsuAnimeDetail,
	kitsuSearch,
	kitsuTitleMatchGet,
	kitsuTitleMatchPut,
	type HistoryEntry,
	type KitsuAnimeRef
} from '$lib/api';

// Mock the api module wholesale — `match.ts` is decoupled from the
// transport (was Tauri invoke, now HTTP fetch), and the assertions
// here are about which api functions get called with what args.
// Mocking the module itself lets these tests survive any future
// transport switch without churn.
vi.mock('$lib/api', () => ({
	kitsuAnimeBySlug: vi.fn(),
	kitsuAnimeDetail: vi.fn(),
	kitsuSearch: vi.fn(),
	kitsuTitleMatchGet: vi.fn(),
	kitsuTitleMatchPut: vi.fn()
}));

const mockedSlug = vi.mocked(kitsuAnimeBySlug);
const mockedDetail = vi.mocked(kitsuAnimeDetail);
const mockedSearch = vi.mocked(kitsuSearch);
const mockedGetMatch = vi.mocked(kitsuTitleMatchGet);
const mockedPutMatch = vi.mocked(kitsuTitleMatchPut);

const stubKitsu = (id: string, canonical_title = 'Stub'): KitsuAnimeRef => ({
	id,
	canonical_title,
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

beforeEach(() => {
	mockedSlug.mockReset();
	mockedDetail.mockReset();
	mockedSearch.mockReset();
	mockedGetMatch.mockReset();
	mockedPutMatch.mockReset();
	mockedPutMatch.mockResolvedValue(undefined);
});

describe('resolveKitsuMatch', () => {
	it('returns the cached anime detail when the title-match cache hits', async () => {
		const preliminary = resolveHistoryEntry(entry('Demon Slayer (26 episodes)', '5'), null);
		mockedGetMatch.mockResolvedValue('cached-id');
		mockedDetail.mockResolvedValue(stubKitsu('cached-id', 'Demon Slayer'));

		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('cached-id');
		expect(mockedGetMatch).toHaveBeenCalled();
		expect(mockedDetail).toHaveBeenCalledWith('cached-id');
		expect(mockedSearch).not.toHaveBeenCalled();
	});

	it('falls through to a live search + pick + put on cache miss', async () => {
		const preliminary = resolveHistoryEntry(entry('Demon Slayer (26 episodes)', '5'), null);
		mockedGetMatch.mockResolvedValue(null);
		mockedSearch.mockResolvedValue([stubKitsu('fresh-id', 'Demon Slayer')]);

		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('fresh-id');
		expect(mockedGetMatch).toHaveBeenCalled();
		expect(mockedSearch).toHaveBeenCalled();
		expect(mockedPutMatch).toHaveBeenCalledWith(
			preliminary.searchTitle,
			preliminary.cour,
			'fresh-id'
		);
	});

	it('cour > 1 with stale cache hit (slug mismatch) falls through to slug-fetch', async () => {
		// Pre-86e02d2 versions of the picker collapsed sequels onto
		// Part 1 and persisted "Part 2 → Part 1's id" into the cache.
		// On a cache hit, validate the anime's slug — if it doesn't
		// carry the cour suffix, the mapping is stale and we re-resolve.
		const preliminary = resolveHistoryEntry(
			entry('JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2 (12 episodes)', '4'),
			null
		);
		const stalePart1 = {
			...stubKitsu('part1-stale', 'Stone Ocean'),
			slug: 'jojo-s-bizarre-adventure-part-6-stone-ocean'
		};
		mockedGetMatch.mockResolvedValue('part1-stale');
		mockedDetail.mockResolvedValue(stalePart1);
		mockedSlug.mockResolvedValue(stubKitsu('part2-correct'));

		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('part2-correct');
	});

	it('cour > 1 with cache hit whose slug DOES match returns cached without re-fetch', async () => {
		const preliminary = resolveHistoryEntry(entry('Some Anime Part 2 (12 episodes)', '3'), null);
		const correctlyCached = {
			...stubKitsu('part2-cached', 'Some Anime Part 2'),
			slug: 'some-anime-part-2'
		};
		mockedGetMatch.mockResolvedValue('part2-cached');
		mockedDetail.mockResolvedValue(correctlyCached);

		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('part2-cached');
		expect(mockedSlug).not.toHaveBeenCalled();
		expect(mockedSearch).not.toHaveBeenCalled();
	});

	it('falls through to live search when kitsuAnimeDetail rejects (stale cached id)', async () => {
		const preliminary = resolveHistoryEntry(entry('Demon Slayer (26 episodes)', '5'), null);
		mockedGetMatch.mockResolvedValue('stale-id');
		mockedDetail.mockRejectedValue(new Error('404'));
		mockedSearch.mockResolvedValue([stubKitsu('rebuilt-id', 'Demon Slayer')]);

		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('rebuilt-id');
	});

	it('returns null when the live search itself fails', async () => {
		const preliminary = resolveHistoryEntry(entry('Obscure (12 episodes)', '1'), null);
		mockedGetMatch.mockResolvedValue(null);
		mockedSearch.mockRejectedValue(new Error('network down'));

		const got = await resolveKitsuMatch(preliminary);
		expect(got).toBeNull();
	});

	it('still returns the live match when the cache write fails (non-fatal)', async () => {
		const preliminary = resolveHistoryEntry(entry('Demon Slayer (26 episodes)', '5'), null);
		mockedGetMatch.mockResolvedValue(null);
		mockedSearch.mockResolvedValue([stubKitsu('id-1', 'Demon Slayer')]);
		mockedPutMatch.mockRejectedValue(new Error('disk full'));

		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('id-1');
	});

	it('passes searchTitle (cour-stripped if applicable) + cour to the cache key', async () => {
		const preliminary = resolveHistoryEntry(
			entry('JoJo Stone Ocean Part 2 (12 episodes)', '4'),
			null
		);
		mockedGetMatch.mockResolvedValue(null);
		mockedSlug.mockResolvedValue(null);
		mockedSearch.mockResolvedValue([]);

		await resolveKitsuMatch(preliminary);
		expect(mockedGetMatch).toHaveBeenCalledWith(preliminary.searchTitle, 2);
	});

	it('multi-cour entry: tries slug-fetch first and skips search when slug hits', async () => {
		// Stone Ocean Part 2: Kitsu's text-search drops it; the slug
		// lookup pinpoints it. resolveKitsuMatch should NOT fall through
		// to a search call once the slug returns a hit.
		const preliminary = resolveHistoryEntry(
			entry('JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2 (12 episodes)', '4'),
			null
		);
		mockedGetMatch.mockResolvedValue(null);
		mockedSlug.mockResolvedValue(stubKitsu('part2-id', 'JoJo Stone Ocean Part 2'));

		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('part2-id');
		expect(mockedSlug).toHaveBeenCalledWith('jojo-no-kimyou-na-bouken-part-6-stone-ocean-part-2');
		expect(mockedSearch).not.toHaveBeenCalled();
	});

	it('multi-cour entry: falls through to search + pick when slug miss', async () => {
		const preliminary = resolveHistoryEntry(entry('Some Anime Part 2 (12 episodes)', '3'), null);
		mockedGetMatch.mockResolvedValue(null);
		mockedSlug.mockResolvedValue(null);
		mockedSearch.mockResolvedValue([stubKitsu('searched-id', 'Some Anime Part 2')]);

		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('searched-id');
		expect(mockedSlug).toHaveBeenCalled();
		expect(mockedSearch).toHaveBeenCalled();
	});

	it('single-cour entry: skips slug-fetch and goes straight to search', async () => {
		// We don't want to double the IPC volume on cold load; slug
		// fetch is opt-in for cour > 1.
		const preliminary = resolveHistoryEntry(entry('Demon Slayer (26 episodes)', '5'), null);
		mockedGetMatch.mockResolvedValue(null);
		mockedSearch.mockResolvedValue([stubKitsu('id-1', 'Demon Slayer')]);

		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('id-1');
		expect(mockedSlug).not.toHaveBeenCalled();
	});
});
