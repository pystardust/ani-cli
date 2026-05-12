/**
 * Pure helpers behind the play page's "Open in external" success
 * surface. The launch flow used to set an inline `<p
 * class="external-notice">` banner under the player header; that
 * landed in a spot easy to miss (the user often clicked the
 * button twice). The new flow pushes a toast through
 * `$lib/toasts/store.svelte` instead — bottom-right, dock-aware,
 * properly visible.
 *
 * The build-message logic stays pure so the play page can stay a
 * thin adapter (AGENTS.md §2 — extract testable logic out of
 * .svelte files).
 */

import type { ExternalPlayerKind } from '$lib/api';
import type { PushArgs } from '$lib/toasts/store.svelte';
import { m } from '$lib/paraglide/messages';

const BRAND_LABELS: Record<Exclude<ExternalPlayerKind, 'custom'>, string> = {
	mpv: 'mpv',
	vlc: 'VLC',
	iina: 'IINA'
};

/** Human-readable label for an external-player kind. Brand names
 *  stay literal (proper nouns); the `custom` fallback gets a
 *  localized "external player" phrase since there's no product
 *  name to surface. */
export function playerKindLabel(kind: ExternalPlayerKind): string {
	if (kind === 'custom') return m.play_external_player_custom_label();
	return BRAND_LABELS[kind];
}

/** Build the `PushArgs` for the success toast that announces an
 *  external player launched on `episode`. Mirrors the 4s duration
 *  of the legacy inline banner so the surface migrates without
 *  changing how long the message sits on screen. */
export function externalLaunchSuccessToast(args: {
	episode: number;
	kind: ExternalPlayerKind;
}): PushArgs {
	return {
		kind: 'success',
		message: m.play_external_sent_toast({
			episode: args.episode,
			player: playerKindLabel(args.kind)
		}),
		duration: 4000
	};
}
