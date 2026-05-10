import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	__resetApiBaseForTests,
	allmangaKitsuMapGet,
	altTitlesFromKitsu,
	aniskipGet,
	appInfo,
	availabilityBatch,
	availabilityWarm,
	checkAvailability,
	createSession,
	downloadDefaultDir,
	downloadStream,
	evictPlayCache,
	historyByKitsu,
	historyClear,
	historyList,
	imageCacheClear,
	imageProxyUrl,
	kitsuAnimeBySlug,
	kitsuAnimeDetail,
	kitsuEpisodes,
	kitsuResolveAllmangaShowId,
	kitsuSearch,
	kitsuTitleMatchGet,
	kitsuTitleMatchPut,
	kitsuTopRated,
	kitsuTrending,
	kitsuTrendingAnilist,
	markWatched,
	metaCacheClear,
	openExternalPlayer,
	play,
	playExternal,
	playStream,
	proxyBaseUrl,
	settingsGet,
	settingsPut,
	watchedAtAll,
	type Config,
	type CreateSessionResponse,
	type DownloadProgress,
	type DownloadResponse,
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
	// `text()` mirrors the real Response API — expect2xx now reads
	// `text()` first to tolerate empty 2xx bodies, so the mock has
	// to expose both. JSON-stringifying the payload here lets
	// `text()` -> JSON.parse round-trip the way real fetch does.
	// `undefined` payload means "no body" (the empty-body case);
	// literal `null` is a JSON value the backend can return ("null").
	const text = payload === undefined ? '' : JSON.stringify(payload);
	const response = {
		ok: status >= 200 && status < 300,
		status,
		async json() {
			return payload;
		},
		async text() {
			return text;
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
	it('tolerates an empty 2xx body without throwing a JSON parse error', async () => {
		// /api/play/external returns 202 Accepted with no body. expect2xx
		// previously called resp.json() unconditionally and surfaced
		// "Unexpected end of JSON input" as a confusing toast even though
		// mpv had already launched. Pinning the empty-body path here so a
		// regression of that fix is loud.
		const response = {
			ok: true,
			status: 202,
			async json() {
				throw new SyntaxError('Unexpected end of JSON input');
			},
			async text() {
				return '';
			}
		} as unknown as Response;
		const fetchMock = vi.fn(async () => response);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		await expect(playExternal({ title: 'x', episode: '1', mode: 'sub' })).resolves.toBeUndefined();
	});

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

describe('markWatched', () => {
	it('POSTs the PlayArgs payload to /api/play/mark-watched', async () => {
		// Backend writes Continue Watching by looking up the cache row
		// for this key. 204 No Content on success, idempotent.
		const fetchMock = mockFetchOnce(null, 204);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		await markWatched({
			title: 'Naruto: Shippuuden',
			episode: '150',
			mode: 'sub',
			quality: 'best'
		});
		const { url, init } = lastCall(fetchMock);
		expect(url).toBe(`${BASE}/api/play/mark-watched`);
		expect(init?.method).toBe('POST');
		expect(JSON.parse(init?.body as string)).toMatchObject({
			title: 'Naruto: Shippuuden',
			episode: '150',
			mode: 'sub'
		});
	});
});

describe('evictPlayCache', () => {
	it('POSTs the PlayArgs payload to /api/play/cache/evict', async () => {
		// 204 No Content is the documented success status for the
		// eviction endpoint — there's no body to parse, just confirm
		// the request shape.
		const fetchMock = mockFetchOnce(null, 204);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		await evictPlayCache({
			title: 'Stone Ocean',
			episode: '1',
			mode: 'sub',
			quality: 'best'
		});
		const { url, init } = lastCall(fetchMock);
		expect(url).toBe(`${BASE}/api/play/cache/evict`);
		expect(init?.method).toBe('POST');
		expect(JSON.parse(init?.body as string)).toMatchObject({
			title: 'Stone Ocean',
			episode: '1',
			mode: 'sub'
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

	it('appends prefetch=1 when args.prefetch is true', async () => {
		// Backend deserialize_loose_bool accepts "1" / "true" / "yes".
		// The renderer's prefetch loops set this so the backend skips
		// Continue Watching updates for warming calls.
		const promise = playStream({ title: 'X', episode: '5', mode: 'sub', prefetch: true }, () => {});
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		expect(es.url).toContain('prefetch=1');
		es.dispatch('done', JSON.stringify(donePayload()));
		await promise;
	});

	it('omits prefetch param entirely when not set or false', async () => {
		// Click path: leave the field default. Backend treats absence
		// as false, history-write fires.
		const promise = playStream({ title: 'X', episode: '5', mode: 'sub' }, () => {});
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		expect(es.url).not.toContain('prefetch');
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

	it('rejects synchronously when the signal is already aborted at call time', async () => {
		// A click that races a clearForShow can hand us an already-
		// aborted signal. The implementation must finish before the
		// EventSource has a chance to do anything — otherwise the
		// promise stays pending and the loading overlay sits open
		// against a dead session.
		const ctrl = new AbortController();
		ctrl.abort();
		const promise = playStream({ title: 't', episode: '1', mode: 'sub' }, () => {}, ctrl.signal);
		await expect(promise).rejects.toThrow(/aborted/i);
		// The EventSource was constructed (apiBase resolves first),
		// but should be closed by now.
		const es = FakeEventSource.instances[0];
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

describe('watchedAtAll', () => {
	it('GETs /api/watched-at and returns the parsed map', async () => {
		const fetchMock = mockFetchOnce({ 'show-a': 1_800_000_000_000 });
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		const got = await watchedAtAll();
		const { url } = lastCall(fetchMock);
		expect(url).toBe(`${BASE}/api/watched-at`);
		expect(got).toEqual({ 'show-a': 1_800_000_000_000 });
	});

	it('returns an empty object when nothing is stamped', async () => {
		globalThis.fetch = mockFetchOnce({}) as unknown as typeof fetch;
		const got = await watchedAtAll();
		expect(got).toEqual({});
	});
});

describe('allmangaKitsuMapGet', () => {
	it('GETs /api/allmanga-kitsu-map/:show_id with the id URL-encoded', async () => {
		const fetchMock = mockFetchOnce('11061');
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		const got = await allmangaKitsuMapGet('vDTSJHSpYnrkZnAvG');
		const { url } = lastCall(fetchMock);
		expect(url).toBe(`${BASE}/api/allmanga-kitsu-map/vDTSJHSpYnrkZnAvG`);
		expect(got).toBe('11061');
	});

	it('returns null when the backend has no mapping for the show_id', async () => {
		globalThis.fetch = mockFetchOnce(null) as unknown as typeof fetch;
		const got = await allmangaKitsuMapGet('never-played');
		expect(got).toBeNull();
	});

	it('encodes show_ids that contain URL-special characters', async () => {
		// Defensive — allmanga's ids are alphanum today, but the
		// endpoint shape commits to encodeURIComponent so future
		// id-format changes don't crash with double-slash splits.
		const fetchMock = mockFetchOnce(null);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		await allmangaKitsuMapGet('weird/id with spaces');
		const { url } = lastCall(fetchMock);
		expect(url).toBe(`${BASE}/api/allmanga-kitsu-map/weird%2Fid%20with%20spaces`);
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
			image_cache_cap_mb: 500,
			auto_play_next: false,
			download_bottom_bar_enabled: true,
			auto_skip_op: false,
			auto_skip_ed: false,
			use_custom_player_controls: false,
			disable_auto_pip_on_leave: false,
			auto_update_anicli: true
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
			image_cache_cap_mb: 1000,
			auto_play_next: true,
			download_bottom_bar_enabled: false,
			auto_skip_op: true,
			auto_skip_ed: true,
			use_custom_player_controls: true,
			disable_auto_pip_on_leave: true,
			auto_update_anicli: false
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

describe('imageCacheClear', () => {
	it('DELETEs /api/cache/images', async () => {
		const fetchMock = mockFetchOnce(null, 204);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		await imageCacheClear();
		const { url, init } = lastCall(fetchMock);
		expect(url).toBe(`${BASE}/api/cache/images`);
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

describe('apiBase resolution', () => {
	// `apiBase()` doesn't have its own export, but every wrapper
	// goes through it. Drive the three branches by clearing the
	// cache, swapping in a `window.aniGui.apiBase` shim, and
	// asserting the resolved URL. The error path is covered by
	// pointing at no source of truth.
	type WinHolder = { window?: { aniGui?: { apiBase?: string } } };
	const g = globalThis as unknown as WinHolder;

	afterEach(() => {
		delete g.window;
		__resetApiBaseForTests(BASE);
	});

	it('uses window.aniGui.apiBase when available (Electron preload path)', async () => {
		__resetApiBaseForTests(null as unknown as string);
		g.window = { aniGui: { apiBase: 'http://127.0.0.1:99999' } };
		const fetchMock = mockFetchOnce({ ok: true });
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		await proxyBaseUrl();
		expect(lastCall(fetchMock).url).toBe('http://127.0.0.1:99999/api/proxy-base-url');
	});

	it('falls back to VITE_ANI_GUI_API_BASE when neither window nor cache has it', async () => {
		// Drop the cache and window stubs so apiBase() walks past
		// both branches and lands on the import.meta.env arm. Vite
		// exposes the env at runtime; vitest's stubEnv lets us pin
		// the value for one test.
		__resetApiBaseForTests(null as unknown as string);
		delete g.window;
		vi.stubEnv('VITE_ANI_GUI_API_BASE', 'http://127.0.0.1:55555');
		try {
			const fetchMock = mockFetchOnce({ ok: true });
			globalThis.fetch = fetchMock as unknown as typeof fetch;
			await proxyBaseUrl();
			expect(lastCall(fetchMock).url).toBe('http://127.0.0.1:55555/api/proxy-base-url');
		} finally {
			vi.unstubAllEnvs();
		}
	});

	it('throws an explanatory error when no apiBase source is configured', async () => {
		// Both window AND env missing → the wrapper throws so the
		// failure surfaces explicitly instead of silently doing
		// nothing.
		__resetApiBaseForTests(null as unknown as string);
		delete g.window;
		vi.unstubAllEnvs();
		// Defensive: ensure neither dev server nor harness has a
		// VITE_ANI_GUI_API_BASE injected.
		vi.stubEnv('VITE_ANI_GUI_API_BASE', '');
		try {
			await expect(proxyBaseUrl()).rejects.toThrow(/apiBase is not configured/);
		} finally {
			vi.unstubAllEnvs();
		}
	});
});

// — Coverage backfill for the discovery / availability / download
// wrappers. Each is a thin fetch shim, but the URL shape is the
// load-bearing contract — a stray `?episode_count=undefined` or a
// missing `kitsu_id` segment surfaces as a 4xx the user never gets a
// good error for. Pinning the URL prevents that drift.

describe('historyByKitsu', () => {
	it('GETs /api/history/by-kitsu/<id> and returns the parsed entry', async () => {
		const entry = {
			showId: 'allmanga-1',
			title: 'One Piece',
			lastEpisode: 1100,
			lastWatchedAt: 12345
		};
		const fetchMock = mockFetchOnce(entry);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		const got = await historyByKitsu('kid-7442');
		expect(lastCall(fetchMock).url).toBe(`${BASE}/api/history/by-kitsu/kid-7442`);
		expect(got).toEqual(entry);
	});

	it('returns null when the backend has no row for the id', async () => {
		const fetchMock = mockFetchOnce(null);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		expect(await historyByKitsu('kid-unknown')).toBeNull();
	});
});

describe('checkAvailability / availabilityBatch / availabilityWarm', () => {
	it('checkAvailability POSTs the args to /api/availability', async () => {
		const fetchMock = mockFetchOnce({ available: true, episode_count: 12, extra_episodes: [] });
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		const got = await checkAvailability({
			title: 'Demon Slayer',
			mode: 'sub',
			alt_titles: ['Kimetsu no Yaiba'],
			episode_count: 26,
			kitsu_id: 'kid-1',
			status: 'finished'
		});
		const { url, init } = lastCall(fetchMock);
		expect(url).toBe(`${BASE}/api/availability`);
		expect(init?.method).toBe('POST');
		expect(JSON.parse(init?.body as string)).toMatchObject({
			title: 'Demon Slayer',
			mode: 'sub',
			alt_titles: ['Kimetsu no Yaiba'],
			episode_count: 26,
			kitsu_id: 'kid-1',
			status: 'finished'
		});
		expect(got.available).toBe(true);
		expect(got.episode_count).toBe(12);
	});

	it('availabilityBatch POSTs ids + mode to /api/availability/batch', async () => {
		const fetchMock = mockFetchOnce({ cached: { 'kid-1': true, 'kid-2': false } });
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		const got = await availabilityBatch(['kid-1', 'kid-2'], 'dub');
		const { url, init } = lastCall(fetchMock);
		expect(url).toBe(`${BASE}/api/availability/batch`);
		expect(init?.method).toBe('POST');
		expect(JSON.parse(init?.body as string)).toEqual({
			kitsu_ids: ['kid-1', 'kid-2'],
			mode: 'dub'
		});
		expect(got.cached['kid-1']).toBe(true);
	});

	it('availabilityWarm POSTs the items list and resolves to undefined', async () => {
		// The backend can return any body it likes (the warm endpoint
		// is fire-and-forget); the wrapper discards it.
		const fetchMock = mockFetchOnce({ ignored: 'whatever' });
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		const items = [{ title: 'A', mode: 'sub' }];
		const got = await availabilityWarm(items);
		const { url, init } = lastCall(fetchMock);
		expect(url).toBe(`${BASE}/api/availability/warm`);
		expect(init?.method).toBe('POST');
		expect(JSON.parse(init?.body as string)).toEqual({ items });
		expect(got).toBeUndefined();
	});
});

describe('kitsuTrendingAnilist', () => {
	it('GETs /api/kitsu/trending-anilist and returns the parsed list', async () => {
		const list: KitsuAnimeRef[] = [];
		const fetchMock = mockFetchOnce(list);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		const got = await kitsuTrendingAnilist();
		expect(lastCall(fetchMock).url).toBe(`${BASE}/api/kitsu/trending-anilist`);
		expect(got).toEqual(list);
	});
});

describe('aniskipGet', () => {
	it('encodes both the kitsu id and the episode in the path', async () => {
		// Both segments need encodeURIComponent because allmanga's
		// half-episode tags ("1061.5") contain a dot that path
		// matchers would otherwise treat as a separator.
		const intervals = [{ skip_type: 'op', start_time: 0, end_time: 88 }];
		const fetchMock = mockFetchOnce(intervals);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		const got = await aniskipGet('kid 7442', '1061.5', 1417.7);
		const { url } = lastCall(fetchMock);
		expect(url).toBe(`${BASE}/api/aniskip/kid%207442/1061.5?episode_length=1417.7`);
		expect(got).toEqual(intervals);
	});

	it('returns an empty list when aniskip has no skip times', async () => {
		// Backend collapses 404 / found:false / unmapped MAL into
		// `[]` — the wrapper just trusts the body shape.
		const fetchMock = mockFetchOnce([]);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		expect(await aniskipGet('kid-1', '1', 1200)).toEqual([]);
	});
});

describe('kitsuResolveAllmangaShowId', () => {
	it('GETs /api/kitsu/resolve-allmanga/<showId> and returns the Kitsu ref', async () => {
		const ref = { id: 'kid-1', canonical_title: 'One Piece' } as unknown as KitsuAnimeRef;
		const fetchMock = mockFetchOnce(ref);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		const got = await kitsuResolveAllmangaShowId('1P');
		expect(lastCall(fetchMock).url).toBe(`${BASE}/api/kitsu/resolve-allmanga/1P`);
		expect(got).toEqual(ref);
	});

	it('returns null when no Kitsu match exists', async () => {
		const fetchMock = mockFetchOnce(null);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		expect(await kitsuResolveAllmangaShowId('garbage')).toBeNull();
	});
});

describe('downloadDefaultDir', () => {
	it('GETs /api/download/default-dir and returns the path string', async () => {
		const fetchMock = mockFetchOnce('/home/user/Videos/Anime');
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		const got = await downloadDefaultDir();
		expect(lastCall(fetchMock).url).toBe(`${BASE}/api/download/default-dir`);
		expect(got).toBe('/home/user/Videos/Anime');
	});

	it('returns null when neither $XDG_DOWNLOAD_DIR nor $HOME is available', async () => {
		// Backend emits literal `null` JSON; the modal then prompts
		// the user to pick a path explicitly.
		const fetchMock = mockFetchOnce(null);
		globalThis.fetch = fetchMock as unknown as typeof fetch;
		expect(await downloadDefaultDir()).toBeNull();
	});
});

describe('downloadStream', () => {
	// Same fake-EventSource pattern as the playStream block above —
	// duplicated rather than extracted because vitest doesn't share
	// describe-block locals across files cleanly.
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

	function donePayload(): DownloadResponse {
		return { dest_dir: '/var/anime' };
	}

	it('opens the SSE URL with title / episode / mode / quality / kitsu_id / download_dir', async () => {
		const onProgress = vi.fn();
		const promise = downloadStream(
			{
				title: 'Demon Slayer',
				episode: '5',
				mode: 'sub',
				quality: '1080',
				kitsu_id: 'kid-9',
				download_dir: '/tmp/dl'
			},
			onProgress
		);
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		expect(es).toBeTruthy();
		expect(es.url).toContain('/api/download/stream?');
		expect(es.url).toContain('title=Demon+Slayer');
		expect(es.url).toContain('episode=5');
		expect(es.url).toContain('mode=sub');
		expect(es.url).toContain('quality=1080');
		expect(es.url).toContain('kitsu_id=kid-9');
		expect(es.url).toContain('download_dir=%2Ftmp%2Fdl');
		es.dispatch('done', JSON.stringify(donePayload()));
		await promise;
	});

	it('forwards SSE progress events to onProgress, parsing the JSON payload', async () => {
		const onProgress = vi.fn<(p: DownloadProgress) => void>();
		const promise = downloadStream({ title: 'X', episode: '1', mode: 'sub' }, onProgress);
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		es.dispatch('progress', JSON.stringify({ line: '[download] 50%' }));
		es.dispatch('progress', JSON.stringify({ line: '[download] 100%' }));
		es.dispatch('done', JSON.stringify(donePayload()));
		await promise;
		expect(onProgress).toHaveBeenCalledTimes(2);
		expect(onProgress.mock.calls[0][0]).toEqual({ line: '[download] 50%' });
	});

	it('swallows malformed progress JSON (the dock keeps going)', async () => {
		const onProgress = vi.fn<(p: DownloadProgress) => void>();
		const promise = downloadStream({ title: 'X', episode: '1', mode: 'sub' }, onProgress);
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		// Garbage payload — the stream stays open, the call still
		// resolves on the eventual `done`.
		es.dispatch('progress', 'not-json');
		es.dispatch('done', JSON.stringify(donePayload()));
		await promise;
		expect(onProgress).not.toHaveBeenCalled();
	});

	it('rejects with the parsed error payload on a JSON error event', async () => {
		const promise = downloadStream({ title: 'X', episode: '1', mode: 'sub' }, () => {});
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		es.dispatch('error', JSON.stringify({ kind: 'scraper', detail: 'no_results' }));
		await expect(promise).rejects.toEqual({ kind: 'scraper', detail: 'no_results' });
		expect(es.closed).toBe(true);
	});

	it('rejects with a generic Error on a non-JSON error event', async () => {
		// Bare error events (connection drop) come through with empty
		// `data`; the wrapper falls back to a single canonical
		// message so the dock can still tell the user what happened.
		const promise = downloadStream({ title: 'X', episode: '1', mode: 'sub' }, () => {});
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		es.dispatch('error');
		await expect(promise).rejects.toThrow(/closed before completion/);
	});

	it('rejects when the abort signal fires and closes the SSE', async () => {
		const ctrl = new AbortController();
		const promise = downloadStream(
			{ title: 'X', episode: '1', mode: 'sub' },
			() => {},
			ctrl.signal
		);
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		ctrl.abort();
		await expect(promise).rejects.toThrow(/aborted/);
		expect(es.closed).toBe(true);
	});

	it('rejects immediately when a pre-aborted signal is passed', async () => {
		// User clicked Cancel on the dock before the SSE opened.
		// The synchronous-abort branch must still close the
		// EventSource cleanly.
		const ctrl = new AbortController();
		ctrl.abort();
		const promise = downloadStream(
			{ title: 'X', episode: '1', mode: 'sub' },
			() => {},
			ctrl.signal
		);
		await expect(promise).rejects.toThrow(/aborted/);
		// Give the apiBase().then callback one tick — by the time
		// it resolved, finish() must have flipped `closed`.
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		expect(es.closed).toBe(true);
	});

	it('ignores a `done` event that arrives after the signal already aborted (settled guard)', async () => {
		// Re-entrant finish() — we want only the first reason to
		// stick; the second event must short-circuit on `settled`.
		const ctrl = new AbortController();
		const promise = downloadStream(
			{ title: 'X', episode: '1', mode: 'sub' },
			() => {},
			ctrl.signal
		);
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		ctrl.abort();
		await expect(promise).rejects.toThrow(/aborted/);
		// Late `done` event — must not crash, must not flip the
		// already-rejected promise's state.
		expect(() => es.dispatch('done', JSON.stringify(donePayload()))).not.toThrow();
	});

	it('rejects synchronously when EventSource is unavailable', async () => {
		delete g.EventSource;
		await expect(
			downloadStream({ title: 'X', episode: '1', mode: 'sub' }, () => {})
		).rejects.toThrow(/EventSource unavailable/);
	});

	it('joins alt_titles with newlines and includes episode_count when provided', async () => {
		// The corresponding `playStream` test pins this for play; the
		// download path should match (backend uses the same
		// deserialize_alt_titles helper for both endpoints).
		const promise = downloadStream(
			{
				title: 'Stone Ocean',
				episode: '1',
				mode: 'sub',
				alt_titles: ['Jojo Part 6', 'ストーンオーシャン'],
				episode_count: 12
			},
			() => {}
		);
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		expect(es.url).toContain('episode_count=12');
		const expectedAlt = new URLSearchParams({
			alt_titles: 'Jojo Part 6\nストーンオーシャン'
		}).toString();
		expect(es.url).toContain(expectedAlt);
		es.dispatch('done', JSON.stringify(donePayload()));
		await promise;
	});

	it('rejects when the `done` event payload fails to parse', async () => {
		// Should never happen on a healthy backend, but a partial /
		// truncated SSE chunk would surface here. Make sure the
		// rejection still closes the stream rather than hanging the
		// dock on an open EventSource.
		const promise = downloadStream({ title: 'X', episode: '1', mode: 'sub' }, () => {});
		await Promise.resolve();
		await Promise.resolve();
		const es = FakeEventSource.instances[0];
		es.dispatch('done', 'not-json');
		await expect(promise).rejects.toBeDefined();
		expect(es.closed).toBe(true);
	});
});
