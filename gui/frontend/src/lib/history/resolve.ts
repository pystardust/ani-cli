/**
 * History → Kitsu resolver.
 *
 * ani-cli (allmanga) sometimes splits one Kitsu anime across several
 * shows. Stone Ocean is the canonical example: allmanga has it as
 * Part 1 (eps 1-12), Part 2 (eps 1-12), Part 3 (eps 1-12); Kitsu has
 * it as a single 38-episode entry. A naive title-match collapses
 * both Continue Watching cards to the same Kitsu page on the same
 * episode, which is wrong.
 *
 * This module is the choke point for that resolution. Today's
 * implementation is the title heuristic ("...Part N" / "Cour N" /
 * "Season N" suffix → cour-offset). Tomorrow's implementation pulls
 * `(allmanga_id → anilist_id → kitsu_id, episode_offset)` from the
 * `anime-offline-database` weekly dump, cached locally. Either way,
 * call sites consume the same `ResumeTarget` shape — swap the
 * internals, leave the UI untouched.
 *
 * TODO(post-v1): replace `cour-offset-suffix` branch with a lookup
 * against the offline-DB. AniList splits cours like allmanga does, so
 * an allmanga title resolves to an anilist_id directly; the only
 * inferred bit is the offset to Kitsu (which collapses cours). Keep
 * the title heuristic as fallback for entries not in the DB.
 */

import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';

/** UI tile count per page in /anime/[id]'s episode grid. Must match
 *  the `UI_PAGE_SIZE` used in that route — the resolver computes
 *  which UI page contains the resumed episode, which only works if
 *  both ends agree. */
export const EPISODES_UI_PAGE_SIZE = 12;

/** Kitsu's `page[limit]` hard cap on the episodes endpoint. */
export const EPISODES_KITSU_PAGE_SIZE = 20;

export interface ResumeTarget {
	/** Title as it appears in ani-cli's history, with the trailing
	 *  "(N episodes)" parenthetical stripped. The source of truth for
	 *  what the user is actually watching — Kitsu's canonical title
	 *  collapses cours into one row and would render two distinct
	 *  Continue Watching cards as identical strings. */
	displayTitle: string;
	/** The episode number the user remembers (allmanga-relative). */
	displayEpisode: number;
	/** Cour size taken from the "(N episodes)" tail. Null when the
	 *  entry has no parenthetical (older ani-cli formats). */
	courSize: number | null;
	/** Detected cour index. 1 when no Part/Cour/Season suffix is
	 *  found at the end of the title, which is most shows. */
	cour: number;

	/** Kitsu match (or null). When null the card has nowhere
	 *  meaningful to navigate; caller should route to /search. */
	kitsuId: string | null;
	/** The episode number translated into Kitsu's numbering. Equals
	 *  `displayEpisode` for single-cour shows; offset by
	 *  `(cour − 1) × courSize` when a cour suffix was detected. */
	kitsuEpisode: number | null;
	/** 1-based UI page of /anime/[id] that contains kitsuEpisode. */
	uiPage: number;

	/** Diagnostic — which branch of the resolver fired. Surfaced in
	 *  case we want to tag cards visually (e.g. show a "best-effort"
	 *  badge when the cour heuristic kicks in) or log misses. */
	mappingNote: 'direct' | 'cour-offset-suffix' | 'no-cour-detected' | 'no-kitsu-match';
}

/** Matches a `Part N` / `Cour N` / `Season N` token at the *end* of
 *  the title. Anchored to end so "JoJo Part 6: Stone Ocean" doesn't
 *  trip on the mid-title "Part 6" — that "Part" refers to the parent
 *  series, not the cour. Only a trailing match is treated as a cour
 *  disambiguator. */
const COUR_SUFFIX_RE = /(?:^|[\s:])(?:Part|Cour|Season)\s+(\d+)\s*$/i;

/** Matches the "(N episodes)" parenthetical ani-cli appends. */
const EPISODE_TAIL_RE = /\s*\(\s*(\d+)\s+episodes?\s*\)\s*$/i;

export function resolveHistoryEntry(
	entry: HistoryEntry,
	kitsuMatch: KitsuAnimeRef | null
): ResumeTarget {
	// ep_no is sometimes a range like "1-12"; take the head.
	const epHead = (entry.ep_no.split(/[^0-9]+/)[0] ?? entry.ep_no) || entry.ep_no;
	const displayEpisode = parseInt(epHead, 10) || 1;

	const tailMatch = entry.title.match(EPISODE_TAIL_RE);
	const courSize = tailMatch ? parseInt(tailMatch[1], 10) : null;
	const stripped = entry.title.replace(EPISODE_TAIL_RE, '').trim();

	const courMatch = stripped.match(COUR_SUFFIX_RE);
	const cour = courMatch ? parseInt(courMatch[1], 10) : 1;

	let kitsuEpisode: number | null = null;
	let mappingNote: ResumeTarget['mappingNote'];

	if (!kitsuMatch) {
		mappingNote = 'no-kitsu-match';
	} else if (cour > 1 && courSize) {
		kitsuEpisode = (cour - 1) * courSize + displayEpisode;
		mappingNote = 'cour-offset-suffix';
	} else if (cour > 1) {
		// Suffix found but we don't know how many episodes per cour.
		// Punt to direct mapping; user will land on the wrong episode
		// but at least the right anime.
		kitsuEpisode = displayEpisode;
		mappingNote = 'no-cour-detected';
	} else {
		kitsuEpisode = displayEpisode;
		mappingNote = 'direct';
	}

	const uiPage = kitsuEpisode ? Math.max(1, Math.ceil(kitsuEpisode / EPISODES_UI_PAGE_SIZE)) : 1;

	return {
		displayTitle: stripped,
		displayEpisode,
		courSize,
		cour,
		kitsuId: kitsuMatch?.id ?? null,
		kitsuEpisode,
		uiPage,
		mappingNote
	};
}

/** Compose the query-string portion of a Resume URL — caller appends
 *  it to the route base built via SvelteKit's `resolve()`. Returns an
 *  empty string when there's nothing worth deep-linking (UI page 1,
 *  no episode target). */
export function resumeQueryString(target: ResumeTarget): string {
	const params = new URLSearchParams();
	if (target.uiPage > 1) params.set('page', String(target.uiPage));
	if (target.kitsuEpisode) params.set('ep', String(target.kitsuEpisode));
	const s = params.toString();
	return s ? `?${s}` : '';
}
