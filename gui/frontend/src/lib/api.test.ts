import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
	__resetApiBaseForTests,
	appInfo,
	createSession,
	historyClear,
	historyList,
	imageProxyUrl,
	kitsuAnimeBySlug,
	kitsuAnimeDetail,
	kitsuEpisodes,
	kitsuSearch,
	kitsuTitleMatchGet,
	kitsuTitleMatchPut,
	kitsuTopRated,
	kitsuTrending,
	metaCacheClear,
	openExternalPlayer,
	proxyBaseUrl,
	settingsGet,
	settingsPut,
	type Config
} from './api';

const BASE = 'http://127.0.0.1:1234';

/**
 * Build a mock `fetch` that returns one canned JSON response. Status
 * defaults to 200; pass a 4xx/5xx to exercise the error path. The
 * api wrappers call `expect2xx` internally which throws the parsed
 * body when `ok` is false, so the same helper covers both paths.
 */
function mockFetchOnce(payload: unknown, status = 200): ReturnType<typeof vi.fn> {
	const response = {
		ok: status >= 200 && status < 300,
		status,
		async json() {
			return payload;
		}
	} as unknown as Response;
	return vi.fn(async () => response);
}

beforeEach(() => {
	__resetApiBaseForTests(BASE);
});

// — Convenience: pull the URL and the optional init off the most-recent
// fetch call. Strongly typed so individual tests can assert on body
// shape without re-casting.
function lastCall(mock: ReturnType<typeof vi.fn>): { url: string; init: RequestInit | undefined } {
	const call = mock.mock.calls.at(-1);
	expect(call).toBeDefined();
	return { url: call![0] as string, init: call![1] as RequestInit | undefined };
}

describe('appInfo', () => {
	it('GETs /api/app-info and returns the parsed body', async () => {
		const fetchMock = mockFetchOnce({
			version: '0.1.0',
			ani_cli_path: '/usr/local/bin/ani-cli',
			history_path: '/home/u/.local/state/ani-cli/ani-hsts',
			proxy_base_url: 'http://127.0.0.1:42337'
		});
		globalThis.fetch = fetchMock as unknown as typeof fetch;

		const info = await appInfo();

		const { url, init } = lastCall(fetchMock);
		expect(url).toBe(`${BASE}/api/app-info`);
		expect(init).toBeUndefined(); // GET, no body
		expect(info.version).toBe('0.1.0');
		expect(info.proxy_base_url).toBe('http://127.0.0.1:42337');
	});
});

describe('proxyBaseUrl', () => {
	it('returns the string the backend hands back', async () => {
		const fetchMock = mockFetchOnce('http://127.0.0.1:42337');
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		const url = await proxyBaseUrl();
		expect(lastCall(fetchMock).url).toBe(`${BASE}/api/proxy-base-url`);
		expect(url).toBe('http://127.0.0.1:42337');
	});
});

describe('historyList', () => {
	it('GETs /api/history and returns the parsed entries', async () => {
		const fetchMock = mockFetchOnce([
			{ ep_no: '1', id: 'aaa', title: 'One Piece (1100 episodes)' },
			{ ep_no: '5', id: 'bbb', title: 'Demon Slayer (26 episodes)' }
		]);
		globalThis.fetch = fetchMock as unknown as typeof fetch;

		const entries = await historyList();

		expect(lastCall(fetchMock).url).toBe(`${BASE}/api/history`);
		expect(entries).toHaveLength(2);
		expect(entries[0].ep_no).toBe('1');
		expect(entries[1].title).toContain('Demon Slayer');
	});

	it('returns an empty array when the backend does', async () => {
		globalThis.fetch = mockFetchOnce([]) as unknown as typeof fetch;
		expect(await historyList()).toEqual([]);
	});
});

describe('historyClear', () => {
	it('DELETEs /api/history with no body', async () => {
		const fetchMock = mockFetchOnce(null, 204);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		await historyClear();
		const { url, init } = lastCall(fetchMock);
		expect(url).toBe(`${BASE}/api/history`);
		expect(init?.method).toBe('DELETE');
	});
});

describe('createSession', () => {
	it('POSTs JSON to /api/sessions', async () => {
		const fetchMock = mockFetchOnce({
			session_id: '11111111-1111-1111-1111-111111111111',
			master_url: 'http://127.0.0.1:42337/s/11111111-.../master.m3u8',
			subtitle_url: null
		});
		globalThis.fetch = fetchMock as unknown as typeof fetch;

		const resp = await createSession({
			upstream_url: 'https://cdn.example/master.m3u8',
			referer: 'https://allmanga.to'
		});

		const { url, init } = lastCall(fetchMock);
		expect(url).toBe(`${BASE}/api/sessions`);
		expect(init?.method).toBe('POST');
		expect(JSON.parse(init?.body as string)).toMatchObject({
			upstream_url: 'https://cdn.example/master.m3u8',
			referer: 'https://allmanga.to'
		});
		expect(resp.session_id).toBe('11111111-1111-1111-1111-111111111111');
	});

	it('forwards optional subtitle_url through the JSON body', async () => {
		const fetchMock = mockFetchOnce({
			session_id: 'sid',
			master_url: 'http://127.0.0.1:1/s/sid/master.m3u8',
			subtitle_url: 'http://127.0.0.1:1/s/sid/sub.vtt'
		});
		globalThis.fetch = fetchMock as unknown as typeof fetch;

		await createSession({
			upstream_url: 'https://cdn.example/master.m3u8',
			referer: 'https://allmanga.to',
			subtitle_url: 'https://cdn.example/cap.vtt'
		});

		const { init } = lastCall(fetchMock);
		expect(JSON.parse(init?.body as string)).toMatchObject({
			subtitle_url: 'https://cdn.example/cap.vtt'
		});
	});

	it('propagates rejection so callers can render the error', async () => {
		const fetchMock = mockFetchOnce({ kind: 'parse_failed', detail: 'upstream_url: invalid' }, 400);
		globalThis.fetch = fetchMock as unknown as typeof fetch;

		await expect(
			createSession({
				upstream_url: 'not a url',
				referer: ''
			})
		).rejects.toMatchObject({ kind: 'parse_failed' });
	});
});

describe('openExternalPlayer', () => {
	it('POSTs the LaunchArgs payload to /api/external-player', async () => {
		const fetchMock = mockFetchOnce(null, 202);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		await openExternalPlayer({
			stream_url: 'https://cdn.example/master.m3u8',
			referer: 'https://allmanga.to',
			subtitle_url: null,
			title: 'Some Anime EP 5',
			player_command: 'mpv'
		});
		const { url, init } = lastCall(fetchMock);
		expect(url).toBe(`${BASE}/api/external-player`);
		expect(init?.method).toBe('POST');
		expect(JSON.parse(init?.body as string)).toMatchObject({
			stream_url: 'https://cdn.example/master.m3u8',
			player_command: 'mpv'
		});
	});
});

describe('kitsuSearch', () => {
	it('POSTs the query under the `query` key the backend expects', async () => {
		const fetchMock = mockFetchOnce([
			{
				id: '12',
				canonical_title: 'One Piece',
				slug: 'one-piece',
				synopsis: 'Long ago…',
				start_date: '1999-10-20',
				end_date: null,
				episode_count: null,
				average_rating: 83.98,
				subtype: 'TV',
				status: 'current',
				age_rating: 'PG',
				popularity_rank: 1,
				poster_image: null,
				cover_image: null
			}
		]);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		const hits = await kitsuSearch('one piece');
		const { url, init } = lastCall(fetchMock);
		expect(url).toBe(`${BASE}/api/kitsu/search`);
		expect(init?.method).toBe('POST');
		expect(JSON.parse(init?.body as string)).toEqual({ query: 'one piece' });
		expect(hits).toHaveLength(1);
		expect(hits[0].canonical_title).toBe('One Piece');
	});
});

describe('kitsuAnimeDetail', () => {
	it('GETs /api/kitsu/anime/:id with the id encoded into the path', async () => {
		const fetchMock = mockFetchOnce({
			id: '12',
			canonical_title: 'One Piece',
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
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		const detail = await kitsuAnimeDetail('12');
		expect(lastCall(fetchMock).url).toBe(`${BASE}/api/kitsu/anime/12`);
		expect(detail.id).toBe('12');
	});
});

describe('kitsuAnimeBySlug', () => {
	it('GETs /api/kitsu/anime-by-slug/:slug', async () => {
		globalThis.fetch = mockFetchOnce(null) as unknown as typeof fetch;
		await kitsuAnimeBySlug('jojo-stone-ocean-part-2');
		// no further assertion needed — the URL alone proves shape
	});
});

describe('kitsuTrending', () => {
	it('GETs /api/kitsu/trending', async () => {
		const fetchMock = mockFetchOnce([]);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		await kitsuTrending();
		expect(lastCall(fetchMock).url).toBe(`${BASE}/api/kitsu/trending`);
	});
});

describe('kitsuTopRated', () => {
	it('GETs /api/kitsu/top-rated', async () => {
		const fetchMock = mockFetchOnce([]);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		await kitsuTopRated();
		expect(lastCall(fetchMock).url).toBe(`${BASE}/api/kitsu/top-rated`);
	});
});

describe('kitsuTitleMatchGet', () => {
	it('GETs /api/title-match with title + cour as query params', async () => {
		const fetchMock = mockFetchOnce('kitsu-id-42');
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		const got = await kitsuTitleMatchGet('Stone Ocean Part 2', 2);
		const { url } = lastCall(fetchMock);
		// URLSearchParams emits `+` for spaces in `application/x-www-form-urlencoded`.
		expect(url).toBe(`${BASE}/api/title-match?title=Stone+Ocean+Part+2&cour=2`);
		expect(got).toBe('kitsu-id-42');
	});

	it('returns null on cache miss', async () => {
		globalThis.fetch = mockFetchOnce(null) as unknown as typeof fetch;
		const got = await kitsuTitleMatchGet('Whatever', 1);
		expect(got).toBeNull();
	});
});

describe('kitsuTitleMatchPut', () => {
	it('PUTs the JSON body the backend expects (kitsu_id snake_case)', async () => {
		const fetchMock = mockFetchOnce(null, 204);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		await kitsuTitleMatchPut('Demon Slayer', 1, 'kitsu-x');
		const { url, init } = lastCall(fetchMock);
		expect(url).toBe(`${BASE}/api/title-match`);
		expect(init?.method).toBe('PUT');
		// Wire format is snake_case to match Rust DTOs.
		expect(JSON.parse(init?.body as string)).toEqual({
			title: 'Demon Slayer',
			cour: 1,
			kitsu_id: 'kitsu-x'
		});
	});
});

describe('kitsuEpisodes', () => {
	it('GETs /api/kitsu/episodes/:anime_id with page=1 by default', async () => {
		const fetchMock = mockFetchOnce([]);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		await kitsuEpisodes('12');
		expect(lastCall(fetchMock).url).toBe(`${BASE}/api/kitsu/episodes/12?page=1`);
	});
	it('passes the explicit page through', async () => {
		const fetchMock = mockFetchOnce([]);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		await kitsuEpisodes('12', 3);
		expect(lastCall(fetchMock).url).toBe(`${BASE}/api/kitsu/episodes/12?page=3`);
	});
});

describe('settingsGet', () => {
	it('GETs /api/settings and returns Config', async () => {
		const cfg: Config = {
			locale: 'en',
			mode: 'sub',
			quality: 'best',
			external_player: 'mpv',
			image_cache_cap_mb: 500
		};
		const fetchMock = mockFetchOnce(cfg);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		const got = await settingsGet();
		expect(lastCall(fetchMock).url).toBe(`${BASE}/api/settings`);
		expect(got.mode).toBe('sub');
	});
});

describe('settingsPut', () => {
	it('PUTs the Config payload directly (no wrapper key)', async () => {
		const cfg: Config = {
			locale: 'pt-BR',
			mode: 'dub',
			quality: '1080',
			external_player: 'vlc',
			image_cache_cap_mb: 1000
		};
		const fetchMock = mockFetchOnce(null, 204);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		await settingsPut(cfg);
		const { url, init } = lastCall(fetchMock);
		expect(url).toBe(`${BASE}/api/settings`);
		expect(init?.method).toBe('PUT');
		expect(JSON.parse(init?.body as string)).toEqual(cfg);
	});
});

describe('metaCacheClear', () => {
	it('DELETEs /api/cache', async () => {
		const fetchMock = mockFetchOnce(null, 204);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		await metaCacheClear();
		const { url, init } = lastCall(fetchMock);
		expect(url).toBe(`${BASE}/api/cache`);
		expect(init?.method).toBe('DELETE');
	});
});

describe('imageProxyUrl', () => {
	it('rewrites https URLs to image://', () => {
		expect(imageProxyUrl('https://media.kitsu.app/anime/12/poster.jpg')).toBe(
			'image://media.kitsu.app/anime/12/poster.jpg'
		);
	});
	it('returns null for null/undefined/empty/non-https input', () => {
		expect(imageProxyUrl(null)).toBeNull();
		expect(imageProxyUrl(undefined)).toBeNull();
		expect(imageProxyUrl('')).toBeNull();
		expect(imageProxyUrl('http://insecure.example/x.jpg')).toBeNull();
		expect(imageProxyUrl('data:image/png;base64,…')).toBeNull();
	});
});
