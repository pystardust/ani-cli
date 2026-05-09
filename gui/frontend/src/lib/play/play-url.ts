/**
 * Build the `?session=…&episode=…` query string callers append to the
 * `/play/[id]` route. Centralised so the home, detail, and prev/next
 * call sites all assemble the URL the same way — no field gets
 * silently dropped on one path while another adds it.
 *
 * The presence of `&sub=1` is the contract /play reads to know whether
 * to render a `<track kind="subtitles">` inside its `<video>`. The
 * subtitle URL itself isn't passed through the query string — the
 * proxy mounts `/s/<session>/sub.vtt` deterministically — only the
 * boolean hint that the backend resolution produced a subtitle.
 */
import type { CreateSessionResponse } from '$lib/api';

/**
 * Compose the `?…` portion of a `/play/[id]` URL from a session
 * resolution + episode number. Always includes `session`, `episode`,
 * `kind`. Conditionally includes `cache_hit=1` and `sub=1`.
 */
export function buildPlayQuery(session: CreateSessionResponse, episode: number): string {
	const parts: string[] = [
		`session=${encodeURIComponent(session.session_id)}`,
		`episode=${episode}`,
		`kind=${session.media_kind}`
	];
	if (session.cache_hit === true) {
		parts.push('cache_hit=1');
	}
	if (session.subtitle_url) {
		parts.push('sub=1');
	}
	return `?${parts.join('&')}`;
}
