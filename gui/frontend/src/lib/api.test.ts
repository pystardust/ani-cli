import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import {
	appInfo,
	createSession,
	historyClear,
	historyList,
	imageProxyUrl,
	kitsuAnimeDetail,
	kitsuSearch,
	kitsuTopRated,
	kitsuTrending,
	openExternalPlayer,
	proxyBaseUrl,
	settingsGet,
	settingsPut,
	type Config
} from './api';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn()
}));

const mockedInvoke = vi.mocked(invoke);

beforeEach(() => {
	mockedInvoke.mockReset();
});

describe('appInfo', () => {
	it('invokes cmd_app_info with no args', async () => {
		mockedInvoke.mockResolvedValue({
			version: '0.1.0',
			ani_cli_path: '/usr/local/bin/ani-cli',
			history_path: '/home/u/.local/state/ani-cli/ani-hsts',
			proxy_base_url: 'http://127.0.0.1:42337'
		});

		const info = await appInfo();

		expect(mockedInvoke).toHaveBeenCalledWith('cmd_app_info');
		expect(info.version).toBe('0.1.0');
		expect(info.proxy_base_url).toBe('http://127.0.0.1:42337');
	});
});

describe('proxyBaseUrl', () => {
	it('returns the string the backend hands back', async () => {
		mockedInvoke.mockResolvedValue('http://127.0.0.1:42337');
		const url = await proxyBaseUrl();
		expect(mockedInvoke).toHaveBeenCalledWith('cmd_proxy_base_url');
		expect(url).toBe('http://127.0.0.1:42337');
	});
});

describe('historyList', () => {
	it('returns the entries the backend hands back', async () => {
		mockedInvoke.mockResolvedValue([
			{ ep_no: '1', id: 'aaa', title: 'One Piece (1100 episodes)' },
			{ ep_no: '5', id: 'bbb', title: 'Demon Slayer (26 episodes)' }
		]);

		const entries = await historyList();

		expect(mockedInvoke).toHaveBeenCalledWith('cmd_history_list');
		expect(entries).toHaveLength(2);
		expect(entries[0].ep_no).toBe('1');
		expect(entries[1].title).toContain('Demon Slayer');
	});

	it('returns an empty array when the backend does', async () => {
		mockedInvoke.mockResolvedValue([]);
		expect(await historyList()).toEqual([]);
	});
});

describe('historyClear', () => {
	it('invokes cmd_history_clear with no args', async () => {
		mockedInvoke.mockResolvedValue(undefined);
		await historyClear();
		expect(mockedInvoke).toHaveBeenCalledWith('cmd_history_clear');
	});
});

describe('createSession', () => {
	it('passes args under the args key the Rust handler expects', async () => {
		mockedInvoke.mockResolvedValue({
			session_id: '11111111-1111-1111-1111-111111111111',
			master_url: 'http://127.0.0.1:42337/s/11111111-.../master.m3u8',
			subtitle_url: null
		});

		const resp = await createSession({
			upstream_url: 'https://cdn.example/master.m3u8',
			referer: 'https://allmanga.to'
		});

		expect(mockedInvoke).toHaveBeenCalledWith('cmd_create_session', {
			args: {
				upstream_url: 'https://cdn.example/master.m3u8',
				referer: 'https://allmanga.to'
			}
		});
		expect(resp.session_id).toBe('11111111-1111-1111-1111-111111111111');
		expect(resp.subtitle_url).toBeNull();
	});

	it('forwards optional subtitle_url through the args wrapper', async () => {
		mockedInvoke.mockResolvedValue({
			session_id: 'sid',
			master_url: 'http://127.0.0.1:1/s/sid/master.m3u8',
			subtitle_url: 'http://127.0.0.1:1/s/sid/sub.vtt'
		});

		await createSession({
			upstream_url: 'https://cdn.example/master.m3u8',
			referer: 'https://allmanga.to',
			subtitle_url: 'https://cdn.example/cap.vtt'
		});

		expect(mockedInvoke).toHaveBeenCalledWith('cmd_create_session', {
			args: {
				upstream_url: 'https://cdn.example/master.m3u8',
				referer: 'https://allmanga.to',
				subtitle_url: 'https://cdn.example/cap.vtt'
			}
		});
	});

	it('propagates rejection so callers can render the error', async () => {
		mockedInvoke.mockRejectedValue({ kind: 'parse_failed', detail: 'upstream_url: invalid' });

		await expect(
			createSession({
				upstream_url: 'not a url',
				referer: ''
			})
		).rejects.toMatchObject({ kind: 'parse_failed' });
	});
});

describe('openExternalPlayer', () => {
	it('passes the LaunchArgs payload under the args key', async () => {
		mockedInvoke.mockResolvedValue(undefined);
		await openExternalPlayer({
			stream_url: 'https://cdn.example/master.m3u8',
			referer: 'https://allmanga.to',
			subtitle_url: null,
			title: 'Some Anime EP 5',
			player_command: 'mpv'
		});
		expect(mockedInvoke).toHaveBeenCalledWith('cmd_open_external_player', {
			args: {
				stream_url: 'https://cdn.example/master.m3u8',
				referer: 'https://allmanga.to',
				subtitle_url: null,
				title: 'Some Anime EP 5',
				player_command: 'mpv'
			}
		});
	});
});

describe('kitsuSearch', () => {
	it('passes query under the query key the Rust handler expects', async () => {
		mockedInvoke.mockResolvedValue([
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
		const hits = await kitsuSearch('one piece');
		expect(mockedInvoke).toHaveBeenCalledWith('cmd_kitsu_search', { query: 'one piece' });
		expect(hits).toHaveLength(1);
		expect(hits[0].canonical_title).toBe('One Piece');
	});
});

describe('kitsuAnimeDetail', () => {
	it('passes id under the id key', async () => {
		mockedInvoke.mockResolvedValue({
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
		const detail = await kitsuAnimeDetail('12');
		expect(mockedInvoke).toHaveBeenCalledWith('cmd_kitsu_anime_detail', { id: '12' });
		expect(detail.id).toBe('12');
	});
});

describe('kitsuTrending', () => {
	it('invokes cmd_kitsu_trending with no args', async () => {
		mockedInvoke.mockResolvedValue([]);
		await kitsuTrending();
		expect(mockedInvoke).toHaveBeenCalledWith('cmd_kitsu_trending');
	});
});

describe('kitsuTopRated', () => {
	it('invokes cmd_kitsu_top_rated with no args', async () => {
		mockedInvoke.mockResolvedValue([]);
		await kitsuTopRated();
		expect(mockedInvoke).toHaveBeenCalledWith('cmd_kitsu_top_rated');
	});
});

describe('settingsGet', () => {
	it('invokes cmd_settings_get with no args and returns Config', async () => {
		const cfg: Config = {
			locale: 'en',
			mode: 'sub',
			quality: 'best',
			external_player: 'mpv',
			image_cache_cap_mb: 500
		};
		mockedInvoke.mockResolvedValue(cfg);
		const got = await settingsGet();
		expect(mockedInvoke).toHaveBeenCalledWith('cmd_settings_get');
		expect(got.mode).toBe('sub');
	});
});

describe('settingsPut', () => {
	it('passes cfg under the cfg key the Rust handler expects', async () => {
		const cfg: Config = {
			locale: 'pt-BR',
			mode: 'dub',
			quality: '1080',
			external_player: 'vlc',
			image_cache_cap_mb: 1000
		};
		mockedInvoke.mockResolvedValue(undefined);
		await settingsPut(cfg);
		expect(mockedInvoke).toHaveBeenCalledWith('cmd_settings_put', { cfg });
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
