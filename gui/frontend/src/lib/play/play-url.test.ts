import { describe, expect, it } from 'vitest';
import { buildPlayQuery } from './play-url';
import type { CreateSessionResponse } from '$lib/api';

const baseSession = (over: Partial<CreateSessionResponse> = {}): CreateSessionResponse => ({
	session_id: 's-123',
	media_url: 'http://localhost:9999/s/s-123/master.m3u8',
	media_kind: 'hls',
	subtitle_url: null,
	cache_hit: false,
	...over
});

describe('buildPlayQuery', () => {
	it('includes session, episode, and kind', () => {
		const q = buildPlayQuery(baseSession({ session_id: 'abc', media_kind: 'mp4' }), 7);
		expect(q).toContain('session=abc');
		expect(q).toContain('episode=7');
		expect(q).toContain('kind=mp4');
	});

	it('url-encodes the session id (it can contain reserved chars)', () => {
		const q = buildPlayQuery(baseSession({ session_id: 'a/b c' }), 1);
		expect(q).toContain('session=a%2Fb%20c');
	});

	it('appends sub=1 when the resolution returned a subtitle URL', () => {
		// F1.11: /play reads ?sub=1 to decide whether to render a
		// <track kind="subtitles"> inside its <video>. The subtitle URL
		// itself isn't shuttled through the query string — the proxy
		// hosts it at /s/<session>/sub.vtt — only the boolean hint
		// that the backend resolution produced one.
		const q = buildPlayQuery(baseSession({ subtitle_url: 'https://upstream/sub.vtt' }), 1);
		expect(q).toContain('sub=1');
	});

	it('omits sub when subtitle_url is null', () => {
		// Most allmanga sources don't ship a separate subtitle file
		// (subs are baked into the HLS manifest as a TextTrack). For
		// those, /play shouldn't render a <track> — the CC button on
		// Chromium's native controls would just be a dead toggle.
		const q = buildPlayQuery(baseSession({ subtitle_url: null }), 1);
		expect(q).not.toContain('sub=1');
		expect(q).not.toContain('sub=');
	});

	it('appends cache_hit=1 only when cache_hit is true', () => {
		expect(buildPlayQuery(baseSession({ cache_hit: true }), 1)).toContain('cache_hit=1');
		expect(buildPlayQuery(baseSession({ cache_hit: false }), 1)).not.toContain('cache_hit');
		// Older backend builds may omit the field entirely; treat absence
		// the same as false.
		const noField = baseSession();
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		delete (noField as any).cache_hit;
		expect(buildPlayQuery(noField, 1)).not.toContain('cache_hit');
	});

	it('combines all flags when both subtitle and cache_hit apply', () => {
		const q = buildPlayQuery(
			baseSession({
				session_id: 'sx',
				media_kind: 'hls',
				subtitle_url: 'https://x/y.vtt',
				cache_hit: true
			}),
			12
		);
		// Order isn't part of the contract; URLSearchParams round-trip
		// proves every key landed regardless of join order.
		const params = new URLSearchParams(q.replace(/^\?/, ''));
		expect(params.get('session')).toBe('sx');
		expect(params.get('episode')).toBe('12');
		expect(params.get('kind')).toBe('hls');
		expect(params.get('cache_hit')).toBe('1');
		expect(params.get('sub')).toBe('1');
	});

	it('starts with `?` so callers can append directly to the route base', () => {
		const q = buildPlayQuery(baseSession(), 1);
		expect(q.startsWith('?')).toBe(true);
	});
});
