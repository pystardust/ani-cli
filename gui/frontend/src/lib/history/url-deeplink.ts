/**
 * Pure helpers for the `/anime/[id]` page's URL deep-link effects.
 * Continue Watching cards link to `/anime/[id]?page=N&ep=M` so the
 * detail page can land directly on the right episode page with the
 * right tile highlighted; this module owns the parsing + decision
 * rules so the `$effect` blocks in the component become thin glue.
 *
 * Companion to `lib/history/nav-depth.ts` — same extraction pattern.
 */

/** Coerce a `?page=` query param into a 1-based UI page. Falls back
 *  to `1` for missing / non-numeric / non-positive values. */
export function parsePageParam(searchParams: URLSearchParams): number {
	const raw = searchParams.get('page') ?? '';
	const n = parseInt(raw, 10);
	return Number.isFinite(n) && n > 0 ? n : 1;
}

/** Coerce a `?ep=` query param into an episode number. Returns
 *  `null` for missing / non-numeric / non-positive values — the
 *  caller treats null as "no highlight target". */
export function parseEpParam(searchParams: URLSearchParams): number | null {
	const raw = searchParams.get('ep') ?? '';
	const n = parseInt(raw, 10);
	return Number.isFinite(n) && n > 0 ? n : null;
}

/** Minimal interface: the helper only cares about the two episode
 *  number fields, which lets tests use plain object literals
 *  instead of full KitsuEpisode stubs. */
export interface EpisodeLike {
	number: number | null;
	relative_number: number | null;
}

/** Whether the current episodes set contains the target episode.
 *  Match by `number` first, then `relative_number` — Kitsu's split
 *  shows expose the cour-relative count via `relative_number` while
 *  parent shows put the global count in `number`. */
export function episodesContainEpisode<E extends EpisodeLike>(
	episodes: readonly E[] | null,
	target: number
): boolean {
	if (!episodes) return false;
	return episodes.some((e) => (e.number ?? e.relative_number) === target);
}

/** What the page-driving `$effect` should do given current state +
 *  the URL-derived target page. */
export type EpisodeFetchAction = 'fetch-initial' | 'fetch' | 'noop';

export function decideEpisodeFetchAction(opts: {
	episodes: readonly unknown[] | null;
	episodesPage: number;
	episodesLoading: boolean;
	targetPage: number;
}): EpisodeFetchAction {
	if (opts.episodes === null) return 'fetch-initial';
	if (opts.targetPage === opts.episodesPage) return 'noop';
	if (opts.episodesLoading) return 'noop';
	return 'fetch';
}

/** Whether the highlight `$effect` should fire for the given URL ep.
 *  Returns false when:
 *   - no `?ep=` was supplied (target is null)
 *   - we already consumed this exact ep value (avoids re-firing on
 *     unrelated reactive updates that touch the URL)
 *   - episodes haven't loaded yet, or don't contain the target — the
 *     effect re-runs when episodes arrive, so deferring is fine. */
export function shouldHighlight<E extends EpisodeLike>(opts: {
	target: number | null;
	consumed: number | null;
	episodes: readonly E[] | null;
}): boolean {
	if (opts.target === null) return false;
	if (opts.target === opts.consumed) return false;
	return episodesContainEpisode(opts.episodes, opts.target);
}
