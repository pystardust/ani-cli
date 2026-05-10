/**
 * Pure formatting helpers extracted from the /play/[id] page so the
 * branch-heavy bits can be unit-tested directly. Each one is small on
 * purpose — the page imports them by name and only the formatting
 * logic gets re-tested when copy or layout shifts.
 */

import type { PlayProgress } from '$lib/api';
import { m } from '$lib/paraglide/messages';

/** Human-readable copy for one SSE progress event. The play page
 *  feeds this into the loading overlay. */
export function progressLabel(p: PlayProgress): string {
	switch (p.kind) {
		case 'banner':
			return p.text;
		case 'links_fetched':
			return `${p.provider} ✓`;
		case 'other':
			return p.text;
	}
}

/** Label for the Skip OP / Skip Outro / Skip Recap button. Falls
 *  back to a generic "Skip" so the button stays usable when aniskip
 *  surfaces a skip-type the UI hasn't seen before. */
export function skipLabel(skipType: string): string {
	if (skipType === 'op') return m.play_skip_op();
	if (skipType === 'ed') return m.play_skip_ed();
	if (skipType === 'recap') return m.play_skip_recap();
	return m.play_skip_default();
}

/** Render a media timestamp as `M:SS` or `H:MM:SS`. Negative or
 *  non-finite inputs render as `0:00` rather than NaN — guards the
 *  player chrome against an `<video>` that hasn't loaded its
 *  duration yet. */
export function formatTime(seconds: number): string {
	if (!Number.isFinite(seconds) || seconds < 0) return '0:00';
	const total = Math.floor(seconds);
	const h = Math.floor(total / 3600);
	const m = Math.floor((total % 3600) / 60);
	const s = total % 60;
	const mm = h > 0 ? String(m).padStart(2, '0') : String(m);
	const ss = String(s).padStart(2, '0');
	return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
}
