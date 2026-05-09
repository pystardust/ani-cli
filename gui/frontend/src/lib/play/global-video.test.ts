// @vitest-environment happy-dom
//
// happy-dom gives us a real `document` so we can exercise the
// imperative DOM bits of global-video (the singleton creation,
// attach / detach lifecycle, subtitle track swap). The pure
// `canReuseSession` decision below doesn't need DOM but co-locates
// nicely with its DOM-bound siblings.
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
	attachGlobalVideoTo,
	canReuseSession,
	detachGlobalVideo,
	getCurrentSession,
	getGlobalVideo,
	reuseSessionIfMatching,
	setCurrentSession,
	setSubtitleTrack,
	type VideoSession,
	type VideoStateSnapshot
} from './global-video';

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

// The rest of the suite needs a real DOM. happy-dom (set via the
// header directive above) gives us `document.body` and friends; we
// reset state between cases by wiping the host element so the
// singleton can be re-created from scratch each time.
describe('singleton lifecycle (happy-dom)', () => {
	beforeEach(() => {
		document.body.innerHTML = '';
		setCurrentSession(null);
	});
	afterEach(() => {
		setCurrentSession(null);
		document.getElementById('ani-gui-video-host')?.remove();
	});

	it('lazily creates a hidden host with the singleton on first access', () => {
		const v = getGlobalVideo();
		expect(v).toBeInstanceOf(HTMLVideoElement);
		const host = document.getElementById('ani-gui-video-host');
		expect(host).not.toBeNull();
		expect(host?.contains(v)).toBe(true);
		// Off-screen styling so the parked element doesn't leak
		// pixels into the page when nothing is mounted.
		expect(host?.style.opacity).toBe('0');
		expect(host?.style.pointerEvents).toBe('none');
	});

	it('returns the same element on subsequent calls (singleton contract)', () => {
		const a = getGlobalVideo();
		const b = getGlobalVideo();
		expect(b).toBe(a);
	});

	it('attachGlobalVideoTo moves the singleton into the requested parent', () => {
		const frame = document.createElement('div');
		document.body.appendChild(frame);
		const v = attachGlobalVideoTo(frame);
		expect(v.parentElement).toBe(frame);
	});

	it('attachGlobalVideoTo is a no-op when the element is already in that parent', () => {
		const frame = document.createElement('div');
		document.body.appendChild(frame);
		attachGlobalVideoTo(frame);
		const before = frame.children.length;
		attachGlobalVideoTo(frame);
		expect(frame.children.length).toBe(before);
	});

	it('detachGlobalVideo moves the singleton back to the hidden host', () => {
		const frame = document.createElement('div');
		document.body.appendChild(frame);
		const v = attachGlobalVideoTo(frame);
		detachGlobalVideo();
		expect(v.parentElement?.id).toBe('ani-gui-video-host');
	});

	it('setCurrentSession / getCurrentSession round-trips the session pointer', () => {
		expect(getCurrentSession()).toBeNull();
		const s: VideoSession = {
			kitsu_id: 'kid-1',
			episode: 1,
			session_id: 'sess-x',
			media_url: 'http://localhost/proxy/1',
			media_kind: 'mp4',
			subtitle_url: null
		};
		setCurrentSession(s);
		expect(getCurrentSession()).toEqual(s);
		setCurrentSession(null);
		expect(getCurrentSession()).toBeNull();
	});

	it('reuseSessionIfMatching consults the live singleton state', () => {
		const v = getGlobalVideo();
		// Without a session pointer the wrapper short-circuits to
		// null even when the element looks healthy.
		expect(reuseSessionIfMatching('kid-1', 1)).toBeNull();
		setCurrentSession({
			kitsu_id: 'kid-1',
			episode: 1,
			session_id: 'sess-x',
			media_url: 'http://localhost/proxy/1',
			media_kind: 'mp4',
			subtitle_url: null
		});
		// With no src loaded the helper still returns null — same
		// "don't try to resume a fresh element" rule the pure
		// `canReuseSession` enforces.
		expect(reuseSessionIfMatching('kid-1', 1)).toBeNull();
		v.src = 'http://localhost/proxy/1';
		expect(reuseSessionIfMatching('kid-1', 1)?.session_id).toBe('sess-x');
		// And it still rejects mismatched ids.
		expect(reuseSessionIfMatching('kid-2', 1)).toBeNull();
		expect(reuseSessionIfMatching('kid-1', 2)).toBeNull();
	});

	it('setSubtitleTrack adds a default subtitle <track>', () => {
		setSubtitleTrack('http://localhost/proxy/x.vtt');
		const v = getGlobalVideo();
		const tracks = v.querySelectorAll('track');
		expect(tracks).toHaveLength(1);
		const t = tracks[0] as HTMLTrackElement;
		expect(t.kind).toBe('subtitles');
		expect(t.srclang).toBe('en');
		expect(t.default).toBe(true);
		expect(t.src).toContain('x.vtt');
	});

	it('setSubtitleTrack replaces an existing track on subsequent calls', () => {
		setSubtitleTrack('http://localhost/proxy/a.vtt');
		setSubtitleTrack('http://localhost/proxy/b.vtt');
		const v = getGlobalVideo();
		const tracks = v.querySelectorAll('track');
		// Replacement, not append — same contract the play page's
		// session-change effect needs to avoid stacking VTT files.
		expect(tracks).toHaveLength(1);
		expect((tracks[0] as HTMLTrackElement).src).toContain('b.vtt');
	});

	it('setSubtitleTrack(null) removes any existing track', () => {
		setSubtitleTrack('http://localhost/proxy/a.vtt');
		setSubtitleTrack(null);
		const v = getGlobalVideo();
		expect(v.querySelectorAll('track')).toHaveLength(0);
	});
});
