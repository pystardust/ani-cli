/**
 * Sort history entries for the home Continue Watching strip.
 *
 * The on-disk `ani-hsts` file orders rows by "first time played"
 * (ani-cli's `update_history` updates the `ep_no` in place rather
 * than moving the row), so file position is a poor signal of what
 * the user wants to resume. Backend records a per-show wall-clock
 * timestamp in SQLite on every GUI play through `mark-watched`;
 * this sorter joins that map against the file order.
 *
 * Two-tier sort:
 *   1. GUI-stamped rows on top, descending by timestamp (most
 *      recently watched first).
 *   2. CLI-only / un-stamped rows at the bottom, preserving their
 *      on-disk file order.
 *
 * CLI plays don't reach `mark-watched`, so users alternating
 * between the GUI and CLI see CLI plays demoted but still rendered.
 * No reconciliation needed — Continue Watching is a GUI-tracked
 * surface.
 */

import type { HistoryEntry } from '$lib/api';

export function sortByWatchedAt(
	entries: HistoryEntry[],
	watchedAt: Record<string, number>
): HistoryEntry[] {
	const stamped: HistoryEntry[] = [];
	const unstamped: HistoryEntry[] = [];
	for (const e of entries) {
		if (e.id in watchedAt) {
			stamped.push(e);
		} else {
			unstamped.push(e);
		}
	}
	// stamped[].id is always a key in watchedAt by construction above,
	// so direct lookup is safe — no `?? 0` fallback to leave a dead
	// branch behind.
	stamped.sort((a, b) => watchedAt[b.id] - watchedAt[a.id]);
	return [...stamped, ...unstamped];
}
