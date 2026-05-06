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
});
