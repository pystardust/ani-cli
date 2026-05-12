/**
 * Pure helpers behind the play page's "Watch together" (Syncplay)
 * surface. Mirrors `external-toast.ts` shape for parity: a
 * success-toast builder + a failure-copy helper. Both stay pure so
 * the play page's hamburger handler can be a thin adapter
 * (AGENTS.md §2).
 */

import type { PushArgs } from '$lib/toasts/store.svelte';
import { m } from '$lib/paraglide/messages';
import { describePlayFailure } from './error-copy';

/** Build the `PushArgs` for the success toast that announces a
 *  Syncplay launch on `episode`. Same 4s duration as the external-
 *  player success toast — both events feel the same from the user's
 *  perspective ("the click did something, watch the next window
 *  open"). */
export function syncplayLaunchSuccessToast(args: { episode: number }): PushArgs {
	return {
		kind: 'success',
		message: m.play_syncplay_sent_toast({ episode: args.episode }),
		duration: 4000
	};
}

/** User-facing copy for a Syncplay launch failure. The common case
 *  is a `syncplay_spawn_failed` payload — the configured binary
 *  isn't on PATH or doesn't exist; the surrounding modal then links
 *  the user to syncplay.pl. Other resolve-step failures (scraper /
 *  timeout / network) reuse `describePlayFailure` so the user sees
 *  the same polished message as the embedded play path.
 *
 *  Returns the body text only — the modal's headline and action
 *  link live on the play page (i18n keys + the syncplay.pl href). */
export function describeSyncplayLaunchFailure(e: unknown): string {
	const obj = typeof e === 'object' && e !== null ? (e as Record<string, unknown>) : null;
	if (obj && obj.kind === 'syncplay_spawn_failed' && typeof obj.binary === 'string') {
		// Empty-string is the "user cleared the path in Settings"
		// case the backend explicitly classifies as a spawn failure;
		// surface dedicated copy that points at Settings rather than
		// naming an empty quoted string.
		return obj.binary.length > 0
			? m.play_syncplay_spawn_failed_named({ binary: obj.binary })
			: m.play_syncplay_spawn_failed_unnamed();
	}
	// Resolve-step failures (scraper / timeout / network) — reuse
	// the embedded play path's copy so the user sees a polished
	// message instead of a debug-y "Syncplay failed: <kind>".
	return describePlayFailure(e);
}

/** Predicate that gates the play page's "Get Syncplay" affordance.
 *  True only for well-formed `syncplay_spawn_failed` payloads
 *  carrying a non-empty `binary` string — the situations where
 *  installing Syncplay or fixing the binary path will actually
 *  recover the launch. Resolve-step errors (scraper / network /
 *  timeout / no_results) get false because installing Syncplay
 *  won't help — the upstream resolution failed before Syncplay
 *  ever ran. */
export function isSyncplaySpawnFailure(e: unknown): boolean {
	if (typeof e !== 'object' || e === null) return false;
	const obj = e as Record<string, unknown>;
	// Empty-string binary is the cleared-Settings case the backend
	// emits as a spawn failure — recovery (install / set path) is
	// the same as for a named binary, so keep the affordance visible.
	return obj.kind === 'syncplay_spawn_failed' && typeof obj.binary === 'string';
}
