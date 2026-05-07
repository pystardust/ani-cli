import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	__resetApiBaseForTests,
	altTitlesFromKitsu,
	appInfo,
	createSession,
	play,
	playExternal,
	playStream,
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
	type Config,
	type CreateSessionResponse,
	type KitsuAnimeRef,
	type PlayProgress
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
			media_url: 'http://127.0.0.1:42337/s/11111111-.../master.m3u8',
			media_kind: 'hls',
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
			media_url: 'http://127.0.0.1:1/s/sid/master.m3u8',
			media_kind: 'hls',
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

describe('play', () => {
	it('POSTs the PlayArgs payload to /api/play and returns the session response', async () => {
		const sessionResponse = {
			session_id: 'abc-123',
			media_url: 'http://127.0.0.1:42337/s/abc-123/master.m3u8',
			media_kind: 'hls' as const,
			subtitle_url: null
		};
		const fetchMock = mockFetchOnce(sessionResponse);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		const resp = await play({
			title: 'Cowboy Bebop',
			episode: '5',
			mode: 'sub',
			quality: 'best'
		});
		expect(resp).toEqual(sessionResponse);
		const { url, init } = lastCall(fetchMock);
		expect(url).toBe(`${BASE}/api/play`);
		expect(init?.method).toBe('POST');
		expect(JSON.parse(init?.body as string)).toEqual({
			title: 'Cowboy Bebop',
			episode: '5',
			mode: 'sub',
			quality: 'best'
		});
	});
});

describe('playExternal', () => {
	it('POSTs the PlayArgs payload to /api/play/external', async () => {
		const fetchMock = mockFetchOnce(null, 202);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		await playExternal({
			title: 'Cowboy Bebop',
			episode: '5',
			mode: 'dub'
		});
		const { url, init } = lastCall(fetchMock);
		expect(url).toBe(`${BASE}/api/play/external`);
		expect(init?.method).toBe('POST');
		expect(JSON.parse(init?.body as string)).toMatchObject({
			title: 'Cowboy Bebop',
			episode: '5',
			mode: 'dub'
		});
	});
});

describe('playStream', () => {
	// Minimal EventSource shim: enough surface area for `playStream` to
	// register handlers and for tests to dispatch synthetic events. The
	// real browser type has many fields we don't touch; this fake
	// exposes only what the implementation actually reads
	// (addEventListener / close) plus a `dispatch` test seam.
	type EsHandler = (ev: MessageEvent) => void;
	class FakeEventSource {
		static instances: FakeEventSource[] = [];
		url: string;
		listeners: Record<string, EsHandler[]> = {};
		closed = false;
		constructor(url: string) {
			this.url = url;
			FakeEventSource.instances.push(this);
		}
		addEventListener(name: string, handler: EsHandler) {
			(this.listeners[name] ??= []).push(handler);
		}
		close() {
			this.closed = true;
		}
		dispatch(name: string, data?: string) {
			const ev = { data: data ?? '', type: name } as unknown as MessageEvent;
			for (const h of this.listeners[name] ?? []) h(ev);
		}
	}

	type GlobalLike = { EventSource?: typeof FakeEventSource };
	const g = globalThis as unknown as GlobalLike;

	beforeEach(() => {
		FakeEventSource.instances.length = 0;
		g.EventSource = FakeEventSource;
	});

	afterEach(() => {
		delete g.EventSource;
	});

	function donePayload(): CreateSessionResponse {
		return {
			session_id: 'sid',
			media_url: `${BASE}/s/sid/master.m3u8`,
			media_kind: 'hls',
			subtitle_url: null
		};
	}

	it('opens an SSE URL with title/episode/mode/quality/episode_count query params', async () => {
		const onProgress = vi.fn();
		const promise = playStream(
			{ title: 'One Piece', episode: '1', mode: 'sub', quality: 'best', episode_count: 1100 },
			onProgress
		);
		// `apiBase()` is async — give the promise chain one tick to construct
		// the EventSource before we inspect it.
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		expect(es).toBeTruthy();
		expect(es.url).toBe(
			`${BASE}/api/play/stream?title=One+Piece&episode=1&mode=sub&quality=best&episode_count=1100`
		);
		es.dispatch('done', JSON.stringify(donePayload()));
		await promise;
	});

	it('joins alt_titles with `\\n` for the SSE query (matches backend deserializer)', async () => {
		// Backend's deserialize_alt_titles splits on `\n`. URLSearchParams
		// percent-encodes the newline as %0A. Stone Ocean's full
		// candidate list is the realistic case for this test.
		const promise = playStream(
			{
				title: "JoJo's Bizarre Adventure: Stone Ocean",
				episode: '1',
				mode: 'sub',
				alt_titles: [
					'Jojo no Kimyou na Bouken Part 6: Stone Ocean',
					'ジョジョの奇妙な冒険 ストーンオーシャン'
				]
			},
			() => {}
		);
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		// URLSearchParams form-encodes spaces as `+` (vs encodeURIComponent's
		// `%20`), so build the expected encoding the same way to compare.
		const expectedEncoded = new URLSearchParams({
			alt_titles:
				'Jojo no Kimyou na Bouken Part 6: Stone Ocean\nジョジョの奇妙な冒険 ストーンオーシャン'
		}).toString();
		expect(es.url).toContain(expectedEncoded);
		es.dispatch('done', JSON.stringify(donePayload()));
		await promise;
	});

	it('omits alt_titles entirely when the array is empty', async () => {
		const promise = playStream({ title: 'X', episode: '1', mode: 'sub', alt_titles: [] }, () => {});
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		expect(es.url).not.toContain('alt_titles');
		es.dispatch('done', JSON.stringify(donePayload()));
		await promise;
	});

	it('omits quality and episode_count when not provided', async () => {
		const promise = playStream({ title: 'X', episode: '2', mode: 'dub' }, () => {});
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		expect(es.url).toBe(`${BASE}/api/play/stream?title=X&episode=2&mode=dub`);
		es.dispatch('done', JSON.stringify(donePayload()));
		await promise;
	});

	it('forwards parsed progress events to onProgress in arrival order', async () => {
		const seen: PlayProgress[] = [];
		const promise = playStream({ title: 't', episode: '1', mode: 'sub' }, (p) => seen.push(p));
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		es.dispatch('progress', JSON.stringify({ kind: 'links_fetched', provider: 'youtube' }));
		es.dispatch('progress', JSON.stringify({ kind: 'banner', text: 'hi' }));
		es.dispatch('done', JSON.stringify(donePayload()));
		await promise;
		expect(seen).toEqual([
			{ kind: 'links_fetched', provider: 'youtube' },
			{ kind: 'banner', text: 'hi' }
		]);
	});

	it('swallows malformed progress JSON without rejecting', async () => {
		const onProgress = vi.fn();
		const promise = playStream({ title: 't', episode: '1', mode: 'sub' }, onProgress);
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		es.dispatch('progress', '{ not json');
		expect(onProgress).not.toHaveBeenCalled();
		es.dispatch('done', JSON.stringify(donePayload()));
		await expect(promise).resolves.toMatchObject({ session_id: 'sid' });
	});

	it('resolves with the parsed done payload and closes the EventSource', async () => {
		const promise = playStream({ title: 't', episode: '1', mode: 'sub' }, () => {});
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		es.dispatch('done', JSON.stringify(donePayload()));
		const resp = await promise;
		expect(resp.session_id).toBe('sid');
		expect(es.closed).toBe(true);
	});

	it('rejects when done payload is malformed JSON', async () => {
		const promise = playStream({ title: 't', episode: '1', mode: 'sub' }, () => {});
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		es.dispatch('done', '{ broken');
		await expect(promise).rejects.toBeInstanceOf(SyntaxError);
		expect(es.closed).toBe(true);
	});

	it('rejects with the parsed error payload when an error event carries data', async () => {
		const promise = playStream({ title: 't', episode: '1', mode: 'sub' }, () => {});
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		es.dispatch('error', JSON.stringify({ kind: 'upstream_403', detail: 'blocked' }));
		await expect(promise).rejects.toMatchObject({ kind: 'upstream_403' });
		expect(es.closed).toBe(true);
	});

	it('rejects with a generic Error when an error event has no data', async () => {
		const promise = playStream({ title: 't', episode: '1', mode: 'sub' }, () => {});
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		es.dispatch('error');
		await expect(promise).rejects.toThrow(/Stream closed before resolution finished/);
	});

	it('rejects with a generic Error when error data is non-JSON garbage', async () => {
		// Regression guard. The previous implementation called
		// `finish(() => reject(JSON.parse(data)))` — `JSON.parse`
		// fired *inside* `finish`, which had already set `settled = true`
		// before the throw. The fall-through `finish(...generic Error)`
		// then short-circuited on `if (settled) return`, leaving the
		// promise pending forever. This test would time out under that
		// behaviour.
		const promise = playStream({ title: 't', episode: '1', mode: 'sub' }, () => {});
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		es.dispatch('error', '{ not json');
		await expect(promise).rejects.toThrow(/Stream closed before resolution finished/);
	});

	it('ignores events that arrive after settling (no double-resolve)', async () => {
		const seen: PlayProgress[] = [];
		const promise = playStream({ title: 't', episode: '1', mode: 'sub' }, (p) => seen.push(p));
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		es.dispatch('done', JSON.stringify(donePayload()));
		await promise;
		// Late events should be no-ops once settled. The `settled` guard
		// in playStream is what stops a second resolve/close cycle.
		es.dispatch('done', JSON.stringify(donePayload()));
		es.dispatch('error');
		// Progress still fires its handler (no settle guard there) — that's
		// fine: subscribers receive whatever the producer sends. We only
		// assert nothing throws and the resolved value stayed sid.
		expect(es.closed).toBe(true);
	});

	it('closes the EventSource and rejects when the signal is aborted', async () => {
		// Cancel-on-unmount path. The detail / player page calls
		// clearForShow(id) on onDestroy; play-cache aborts each entry's
		// controller; the abort propagates here. Without this, an
		// abandoned prefetch keeps streaming SSE events into the void
		// while the user is on a different page.
		const ctrl = new AbortController();
		const promise = playStream({ title: 't', episode: '1', mode: 'sub' }, () => {}, ctrl.signal);
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		expect(es.closed).toBe(false);
		ctrl.abort();
		await expect(promise).rejects.toThrow(/aborted|cancelled|abort/i);
		expect(es.closed).toBe(true);
	});

	it('falls back to play() POST when EventSource is unavailable', async () => {
		delete g.EventSource;
		const fetchMock = mockFetchOnce(donePayload());
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		const onProgress = vi.fn();
		const resp = await playStream({ title: 't', episode: '1', mode: 'sub' }, onProgress);
		const { url, init } = lastCall(fetchMock);
		expect(url).toBe(`${BASE}/api/play`);
		expect(init?.method).toBe('POST');
		expect(resp.session_id).toBe('sid');
		expect(onProgress).not.toHaveBeenCalled();
	});
});

describe('altTitlesFromKitsu', () => {
	const baseRef = (overrides: Partial<KitsuAnimeRef>): KitsuAnimeRef => ({
		id: '12',
		canonical_title: 'Stub',
		titles: undefined,
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
		cover_image: null,
		...overrides
	});

	it('returns empty list when titles is missing', () => {
		expect(altTitlesFromKitsu(baseRef({}))).toEqual([]);
	});

	it('returns empty list when ref is null', () => {
		expect(altTitlesFromKitsu(null)).toEqual([]);
		expect(altTitlesFromKitsu(undefined)).toEqual([]);
	});

	it('emits en_jp first (allmanga indexes under romanized JP)', () => {
		const ref = baseRef({
			canonical_title: "JoJo's Bizarre Adventure: Stone Ocean",
			titles: {
				en: "JoJo's Bizarre Adventure: Stone Ocean",
				en_jp: 'Jojo no Kimyou na Bouken Part 6: Stone Ocean',
				ja_jp: 'ジョジョの奇妙な冒険 ストーンオーシャン'
			}
		});
		expect(altTitlesFromKitsu(ref)[0]).toBe('Jojo no Kimyou na Bouken Part 6: Stone Ocean');
	});

	it('drops the canonical title from the alt list', () => {
		// One Piece's titles map duplicates the canonical across en/en_jp.
		// We don't want to send the same title twice.
		const ref = baseRef({
			canonical_title: 'One Piece',
			titles: { en: 'One Piece', en_jp: 'One Piece', ja_jp: 'ONE PIECE' }
		});
		expect(altTitlesFromKitsu(ref)).toEqual(['ONE PIECE']);
	});

	it('skips empty / undefined values', () => {
		const ref = baseRef({
			canonical_title: 'Show',
			titles: { en: 'Show', en_jp: '', ja_jp: 'ja' }
		});
		expect(altTitlesFromKitsu(ref)).toEqual(['ja']);
	});

	it('preserves priority order en_jp → ja_jp → en → en_us', () => {
		const ref = baseRef({
			canonical_title: 'Canon',
			titles: { en_us: 'us', en: 'en', ja_jp: 'ja', en_jp: 'enjp' }
		});
		expect(altTitlesFromKitsu(ref)).toEqual(['enjp', 'ja', 'en', 'us']);
	});
});

describe('apiBase configuration', () => {
	type WinHolder = { window?: { aniGui?: { apiBase?: string } } };
	const g = globalThis as unknown as WinHolder;

	afterEach(() => {
		delete g.window;
		__resetApiBaseForTests(BASE); // restore for other tests
	});

	it('throws a configuration error when neither window.aniGui nor env is set', async () => {
		__resetApiBaseForTests(null);
		// No window stub, and vitest's import.meta.env.VITE_ANI_GUI_API_BASE
		// is not set in this run. Any call that needs the base should throw.
		await expect(appInfo()).rejects.toThrow(/apiBase is not configured/);
	});

	it('uses window.aniGui.apiBase when present (Electron preload path)', async () => {
		__resetApiBaseForTests(null);
		g.window = { aniGui: { apiBase: 'http://127.0.0.1:9999' } };
		const fetchMock = mockFetchOnce({
			version: '0.0.0',
			ani_cli_path: '/x',
			history_path: '/y',
			proxy_base_url: 'http://127.0.0.1:1'
		});
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		await appInfo();
		expect(lastCall(fetchMock).url).toBe('http://127.0.0.1:9999/api/app-info');
	});
});

describe('expect2xx error fallback', () => {
	it('synthesizes {kind:"http",status} when the backend returns a non-JSON error body', async () => {
		const response = {
			ok: false,
			status: 502,
			async json(): Promise<unknown> {
				throw new SyntaxError('not json');
			}
		} as unknown as Response;
		globalThis.fetch = vi.fn(async () => response) as unknown as typeof fetch;
		await expect(appInfo()).rejects.toMatchObject({ kind: 'http', status: 502 });
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
	// vitest runs in `environment: 'node'` (vite.config.ts), so `window`
	// isn't defined by default. Stub it on `globalThis` per test to
	// exercise the Electron path (preload-injected `aniGui.apiBase`).
	type WinHolder = { window?: { aniGui?: { apiBase?: string } } };
	const g = globalThis as unknown as WinHolder;

	afterEach(() => {
		delete g.window;
	});

	it('rewrites https URLs to /api/image when apiBase is exposed', () => {
		g.window = { aniGui: { apiBase: 'http://127.0.0.1:42337' } };
		expect(imageProxyUrl('https://media.kitsu.app/anime/12/poster.jpg')).toBe(
			'http://127.0.0.1:42337/api/image?url=' +
				encodeURIComponent('https://media.kitsu.app/anime/12/poster.jpg')
		);
	});

	it('returns null when apiBase is unavailable', () => {
		// No `window` stub → `typeof window === 'undefined'` → null. The
		// legacy `image://` Tauri-protocol fallback was removed in M-E5;
		// callers render the placeholder instead.
		expect(imageProxyUrl('https://media.kitsu.app/anime/12/poster.jpg')).toBeNull();
	});

	it('returns null for null/undefined/empty/non-https input', () => {
		expect(imageProxyUrl(null)).toBeNull();
		expect(imageProxyUrl(undefined)).toBeNull();
		expect(imageProxyUrl('')).toBeNull();
		expect(imageProxyUrl('http://insecure.example/x.jpg')).toBeNull();
		expect(imageProxyUrl('data:image/png;base64,…')).toBeNull();
	});
});
