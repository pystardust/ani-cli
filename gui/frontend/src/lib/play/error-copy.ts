/**
 * Maps backend errors to user-facing copy on the play page. Two
 * helpers, in order of specificity:
 *
 *   • `describeError` flattens the AniError envelope to a debug
 *     string (`"<kind>: <detail>"`); used for log lines and as the
 *     input to `describePlayFailure`'s pattern match.
 *   • `describePlayFailure` picks the right user-facing message for
 *     a play-call failure — "no episode," "scraper unhappy,"
 *     "network trouble," etc.
 *
 * Extracted from the play page so the four message branches can be
 * unit-tested instead of being threaded through Svelte effect
 * runtime.
 */

import { m } from '$lib/paraglide/messages';

/** Flatten an arbitrary thrown value into a stable debug string.
 *  Recognises the AniError envelope shape (`{ kind, detail }`) and
 *  falls back to `String(e)` for anything else. */
export function describeError(e: unknown): string {
	if (typeof e === 'object' && e !== null) {
		const obj = e as Record<string, unknown>;
		const kind = typeof obj.kind === 'string' ? obj.kind : null;
		const detail = typeof obj.detail === 'string' ? obj.detail : null;
		if (kind && detail) return `${kind}: ${detail}`;
		if (kind) return kind;
	}
	return String(e);
}

/** User-facing copy for a play-call failure. The message branches
 *  match (in order): no_results → catalogue miss; scraper → upstream
 *  unhappy; timeout → slow upstream; network / upstream → connection
 *  trouble; default → generic retry. */
export function describePlayFailure(e: unknown): string {
	const raw = describeError(e).toLowerCase();
	if (raw.includes('no_results')) {
		return m.play_play_failure_no_results();
	}
	if (raw.includes('scraper')) {
		return m.play_play_failure_scraper();
	}
	if (raw.includes('timeout')) {
		return m.play_play_failure_timeout();
	}
	if (raw.includes('network') || raw.includes('upstream')) {
		return m.play_play_failure_network();
	}
	return m.play_play_failure_generic();
}

/** User-facing copy for an "Open in external player" failure. The
 *  common case is a `player_spawn_failed` payload — the configured
 *  binary isn't on PATH or doesn't exist. Other resolve-step
 *  failures (scraper / timeout / network) reuse `describePlayFailure`
 *  so the user doesn't get a debug-y "External player failed:
 *  scraper" string.
 *
 *  Returns the body text only — the surrounding modal's headline
 *  comes from `play_error_external_headline` and is interpolated
 *  with the episode number by the caller. */
export function describeExternalLaunchFailure(e: unknown): string {
	const obj = typeof e === 'object' && e !== null ? (e as Record<string, unknown>) : null;
	if (
		obj &&
		obj.kind === 'player_spawn_failed' &&
		typeof obj.binary === 'string' &&
		obj.binary.length > 0
	) {
		return m.play_external_spawn_failed_named({ binary: obj.binary });
	}
	// Other failures (scraper / timeout / network on the resolve
	// step) reuse the embedded path's copy so the user sees a
	// polished message instead of a debug-y "External player
	// failed: <kind>".
	return describePlayFailure(e);
}
