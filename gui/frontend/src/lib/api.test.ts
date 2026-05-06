import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import {
	appInfo,
	createSession,
	historyClear,
	historyList,
	openExternalPlayer,
	proxyBaseUrl
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
