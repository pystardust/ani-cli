import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { resolveKitsuMatch } from './match';
import { resolveHistoryEntry } from './resolve';
import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn()
}));

const mockedInvoke = vi.mocked(invoke);

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
	mockedInvoke.mockReset();
});

describe('resolveKitsuMatch', () => {
	it('returns the cached anime detail when the title-match cache hits', async () => {
		const preliminary = resolveHistoryEntry(entry('Demon Slayer (26 episodes)', '5'), null);
		mockedInvoke.mockImplementation(async (cmd) => {
			if (cmd === 'cmd_title_match_get') return 'cached-id';
			if (cmd === 'cmd_kitsu_anime_detail') return stubKitsu('cached-id', 'Demon Slayer');
			throw new Error('unexpected ' + cmd);
		});
		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('cached-id');
		// Cache + detail; should NOT have hit kitsu_search.
		const calls = mockedInvoke.mock.calls.map((c) => c[0]);
		expect(calls).toContain('cmd_title_match_get');
		expect(calls).toContain('cmd_kitsu_anime_detail');
		expect(calls).not.toContain('cmd_kitsu_search');
	});

	it('falls through to a live search + pick + put on cache miss', async () => {
		const preliminary = resolveHistoryEntry(entry('Demon Slayer (26 episodes)', '5'), null);
		mockedInvoke.mockImplementation(async (cmd) => {
			if (cmd === 'cmd_title_match_get') return null;
			if (cmd === 'cmd_kitsu_search') return [stubKitsu('fresh-id', 'Demon Slayer')];
			if (cmd === 'cmd_title_match_put') return undefined;
			throw new Error('unexpected ' + cmd);
		});
		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('fresh-id');
		const calls = mockedInvoke.mock.calls.map((c) => c[0]);
		expect(calls).toContain('cmd_title_match_get');
		expect(calls).toContain('cmd_kitsu_search');
		expect(calls).toContain('cmd_title_match_put');
		// Verify the put carried the resolved id.
		const putCall = mockedInvoke.mock.calls.find((c) => c[0] === 'cmd_title_match_put');
		expect(putCall?.[1]).toMatchObject({ kitsuId: 'fresh-id' });
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
		mockedInvoke.mockImplementation(async (cmd) => {
			if (cmd === 'cmd_title_match_get') return 'part1-stale';
			if (cmd === 'cmd_kitsu_anime_detail') return stalePart1;
			if (cmd === 'cmd_kitsu_anime_by_slug') return stubKitsu('part2-correct');
			if (cmd === 'cmd_title_match_put') return undefined;
			throw new Error('unexpected ' + cmd);
		});
		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('part2-correct');
	});

	it('cour > 1 with cache hit whose slug DOES match returns cached without re-fetch', async () => {
		const preliminary = resolveHistoryEntry(entry('Some Anime Part 2 (12 episodes)', '3'), null);
		const correctlyCached = {
			...stubKitsu('part2-cached', 'Some Anime Part 2'),
			slug: 'some-anime-part-2'
		};
		mockedInvoke.mockImplementation(async (cmd) => {
			if (cmd === 'cmd_title_match_get') return 'part2-cached';
			if (cmd === 'cmd_kitsu_anime_detail') return correctlyCached;
			throw new Error('unexpected ' + cmd);
		});
		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('part2-cached');
		const calls = mockedInvoke.mock.calls.map((c) => c[0]);
		expect(calls).not.toContain('cmd_kitsu_anime_by_slug');
		expect(calls).not.toContain('cmd_kitsu_search');
	});

	it('falls through to live search when kitsuAnimeDetail rejects (stale cached id)', async () => {
		const preliminary = resolveHistoryEntry(entry('Demon Slayer (26 episodes)', '5'), null);
		mockedInvoke.mockImplementation(async (cmd) => {
			if (cmd === 'cmd_title_match_get') return 'stale-id';
			if (cmd === 'cmd_kitsu_anime_detail') throw new Error('404');
			if (cmd === 'cmd_kitsu_search') return [stubKitsu('rebuilt-id', 'Demon Slayer')];
			if (cmd === 'cmd_title_match_put') return undefined;
			throw new Error('unexpected ' + cmd);
		});
		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('rebuilt-id');
	});

	it('returns null when the live search itself fails', async () => {
		const preliminary = resolveHistoryEntry(entry('Obscure (12 episodes)', '1'), null);
		mockedInvoke.mockImplementation(async (cmd) => {
			if (cmd === 'cmd_title_match_get') return null;
			if (cmd === 'cmd_kitsu_search') throw new Error('network down');
			throw new Error('unexpected ' + cmd);
		});
		const got = await resolveKitsuMatch(preliminary);
		expect(got).toBeNull();
	});

	it('still returns the live match when the cache write fails (non-fatal)', async () => {
		const preliminary = resolveHistoryEntry(entry('Demon Slayer (26 episodes)', '5'), null);
		mockedInvoke.mockImplementation(async (cmd) => {
			if (cmd === 'cmd_title_match_get') return null;
			if (cmd === 'cmd_kitsu_search') return [stubKitsu('id-1', 'Demon Slayer')];
			if (cmd === 'cmd_title_match_put') throw new Error('disk full');
			throw new Error('unexpected ' + cmd);
		});
		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('id-1');
	});

	it('passes searchTitle (cour-stripped if applicable) + cour to the cache key', async () => {
		const preliminary = resolveHistoryEntry(
			entry('JoJo Stone Ocean Part 2 (12 episodes)', '4'),
			null
		);
		mockedInvoke.mockImplementation(async (cmd) => {
			if (cmd === 'cmd_title_match_get') return null;
			if (cmd === 'cmd_kitsu_anime_by_slug') return null;
			if (cmd === 'cmd_kitsu_search') return [];
			throw new Error('unexpected ' + cmd);
		});
		await resolveKitsuMatch(preliminary);
		const getCall = mockedInvoke.mock.calls.find((c) => c[0] === 'cmd_title_match_get');
		expect(getCall?.[1]).toMatchObject({
			title: preliminary.searchTitle,
			cour: 2
		});
	});

	it('multi-cour entry: tries slug-fetch first and skips search when slug hits', async () => {
		// Stone Ocean Part 2: Kitsu's text-search drops it; the slug
		// lookup pinpoints it. resolveKitsuMatch should NOT fall through
		// to a search call once the slug returns a hit.
		const preliminary = resolveHistoryEntry(
			entry('JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2 (12 episodes)', '4'),
			null
		);
		mockedInvoke.mockImplementation(async (cmd, args) => {
			if (cmd === 'cmd_title_match_get') return null;
			if (cmd === 'cmd_kitsu_anime_by_slug') {
				// Verify the derived slug matches Kitsu's URL pattern.
				expect((args as { slug?: string } | undefined)?.slug).toBe(
					'jojo-no-kimyou-na-bouken-part-6-stone-ocean-part-2'
				);
				return stubKitsu('part2-id', 'JoJo Stone Ocean Part 2');
			}
			if (cmd === 'cmd_title_match_put') return undefined;
			throw new Error('unexpected ' + cmd);
		});
		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('part2-id');
		const calls = mockedInvoke.mock.calls.map((c) => c[0]);
		expect(calls).toContain('cmd_kitsu_anime_by_slug');
		expect(calls).not.toContain('cmd_kitsu_search');
	});

	it('multi-cour entry: falls through to search + pick when slug miss', async () => {
		const preliminary = resolveHistoryEntry(entry('Some Anime Part 2 (12 episodes)', '3'), null);
		mockedInvoke.mockImplementation(async (cmd) => {
			if (cmd === 'cmd_title_match_get') return null;
			if (cmd === 'cmd_kitsu_anime_by_slug') return null; // slug miss
			if (cmd === 'cmd_kitsu_search') return [stubKitsu('searched-id', 'Some Anime Part 2')];
			if (cmd === 'cmd_title_match_put') return undefined;
			throw new Error('unexpected ' + cmd);
		});
		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('searched-id');
		const calls = mockedInvoke.mock.calls.map((c) => c[0]);
		expect(calls).toContain('cmd_kitsu_anime_by_slug');
		expect(calls).toContain('cmd_kitsu_search');
	});

	it('single-cour entry: skips slug-fetch and goes straight to search', async () => {
		// We don't want to double the IPC volume on cold load; slug
		// fetch is opt-in for cour > 1.
		const preliminary = resolveHistoryEntry(entry('Demon Slayer (26 episodes)', '5'), null);
		mockedInvoke.mockImplementation(async (cmd) => {
			if (cmd === 'cmd_title_match_get') return null;
			if (cmd === 'cmd_kitsu_search') return [stubKitsu('id-1', 'Demon Slayer')];
			if (cmd === 'cmd_title_match_put') return undefined;
			throw new Error('unexpected ' + cmd);
		});
		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('id-1');
		const calls = mockedInvoke.mock.calls.map((c) => c[0]);
		expect(calls).not.toContain('cmd_kitsu_anime_by_slug');
	});
});
