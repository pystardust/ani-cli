import { describe, it, expect } from 'vitest';
import { canReuseSession, type VideoSession, type VideoStateSnapshot } from './global-video';

const session: VideoSession = {
	kitsu_id: 'kid-42',
	episode: 5,
	session_id: 'sess-abc',
	media_url: 'http://localhost:8765/play/sess-abc/master.m3u8',
	media_kind: 'hls',
	subtitle_url: null
};

const livePlayback: VideoStateSnapshot = { hasSource: true, ended: false };

describe('canReuseSession', () => {
	it('returns the session when ids, episode, and state all line up', () => {
		expect(canReuseSession(session, livePlayback, 'kid-42', 5)).toBe(session);
	});

	it('returns null when there is no current session', () => {
		// A fresh app boot, or after a teardown — nothing to reuse.
		expect(canReuseSession(null, livePlayback, 'kid-42', 5)).toBeNull();
	});

	it('returns null when the kitsu_id differs', () => {
		expect(canReuseSession(session, livePlayback, 'kid-99', 5)).toBeNull();
	});

	it('returns null when the episode differs (next-episode click)', () => {
		// Different episode of the same show — the resume short-circuit
		// shouldn't fire; the load effect needs to swap the src.
		expect(canReuseSession(session, livePlayback, 'kid-42', 6)).toBeNull();
	});

	it('returns null when no <video> exists (state snapshot null)', () => {
		// The singleton is created lazily — a click before its first
		// mount would land here.
		expect(canReuseSession(session, null, 'kid-42', 5)).toBeNull();
	});

	it('returns null when the element has no source loaded', () => {
		// Session pointer outlives video state for a brief window
		// during teardown / hls.js detach. Don't try to resume an
		// empty <video>.
		expect(canReuseSession(session, { hasSource: false, ended: false }, 'kid-42', 5)).toBeNull();
	});

	it('returns null when the video has reached its end', () => {
		// Reusing an ended session would flash the last frame before
		// the new URL takes over. Force a fresh respawn instead.
		expect(canReuseSession(session, { hasSource: true, ended: true }, 'kid-42', 5)).toBeNull();
	});
});
