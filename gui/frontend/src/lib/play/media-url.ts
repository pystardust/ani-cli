import type { MediaKind } from '$lib/api';

/**
 * Compose the proxy URL the player should fetch given a session id and
 * its media kind. Mirrors the backend's choice in
 * `commands/session.rs > create_session`:
 *
 *   - HLS sessions live at `…/s/<id>/master.m3u8` and feed hls.js.
 *   - MP4 sessions live at `…/s/<id>/file.mp4` and feed `<video src>`.
 *
 * The renderer can compose the URL purely from the session id + kind
 * carried in URL search params, so navigating between episodes (which
 * mints fresh sessions) doesn't need a backend round trip just to
 * learn the URL.
 */
export function buildMediaUrl(apiBase: string, sessionId: string, kind: MediaKind): string {
	const base = apiBase.replace(/\/+$/, '');
	const path = kind === 'mp4' ? 'file.mp4' : 'master.m3u8';
	return `${base}/s/${encodeURIComponent(sessionId)}/${path}`;
}
