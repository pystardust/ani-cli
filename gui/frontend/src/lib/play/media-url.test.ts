import { describe, expect, it } from 'vitest';
import { buildMediaUrl } from './media-url';

describe('buildMediaUrl', () => {
	it('returns the master.m3u8 endpoint for HLS sessions', () => {
		const url = buildMediaUrl('http://127.0.0.1:42337', 'abc-123', 'hls');
		expect(url).toBe('http://127.0.0.1:42337/s/abc-123/master.m3u8');
	});

	it('returns the file.mp4 endpoint for MP4 sessions', () => {
		const url = buildMediaUrl('http://127.0.0.1:42337', 'def-456', 'mp4');
		expect(url).toBe('http://127.0.0.1:42337/s/def-456/file.mp4');
	});

	it('encodes the session id so dots / slashes in a malformed id stay safe', () => {
		const url = buildMediaUrl('http://x', 'a/b', 'hls');
		expect(url).toBe('http://x/s/a%2Fb/master.m3u8');
	});

	it('strips a trailing slash on the api base so we never emit double slashes', () => {
		const url = buildMediaUrl('http://x/', 'sid', 'mp4');
		expect(url).toBe('http://x/s/sid/file.mp4');
	});
});
