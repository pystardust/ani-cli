/**
 * History → Kitsu resolver.
 *
 * ani-cli (allmanga) sometimes splits one Kitsu anime across several
 * shows (Stone Ocean Part 1 / Part 2 / Part 3 in allmanga, where on
 * Kitsu the structure varies — sometimes one parent, sometimes three
 * separate entries). The Continue Watching cards for those entries
 * used to render identically; this resolver is the choke point that
 * keeps each card's display surface tied to the user's hsts entry.
 *
 * Today's implementation only uses entry.title for display
 * disambiguation and a direct episode mapping. We tried a cour-offset
 * heuristic ((cour-1) × courSize + ep) on the assumption that Kitsu
 * collapses multi-cour shows; it didn't survive contact with the
 * Stone Ocean test case (Kitsu's entry only has the first cour, so
 * "ep 16" routed to nothing and the page rendered placeholders). The
 * cour fields are still computed and stored on the target so the
 * UI can show "Part N" badges if desired, but kitsuEpisode is now a
 * direct passthrough of displayEpisode.
 *
 * TODO(post-v1): replace direct mapping with an offline-DB lookup.
 * `anime-offline-database` (manami-project) maps allmanga titles → an
 * anilist_id → kitsu_id with an explicit episode offset where the two
 * services disagree. AniList splits cours like allmanga does, so the
 * anilist_id resolution is exact; the kitsu offset is the only
 * inferred bit. Cache the DB locally, refresh weekly, fall back to
 * the title heuristic for entries the DB doesn't cover.
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
	/** Title to feed into Kitsu's text search. Verbatim copy of
	 *  displayTitle today — Kitsu often stores multi-cour shows as
	 *  separate entries (Part 1, Part 2, …), and stripping the suffix
	 *  collapsed Stone Ocean Part 2 onto Part 1's 12-episode page.
	 *  Kept distinct from displayTitle so a future implementation
	 *  (offline-DB lookup) can diverge them without churning call
	 *  sites. */
	searchTitle: string;
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
	 *  case we want to log misses or tag cards visually. */
	mappingNote: 'direct' | 'no-kitsu-match';
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
	// searchTitle keeps the cour suffix — see the comment on the
	// interface field for why.
	const searchTitle = stripped;

	const kitsuEpisode = kitsuMatch ? displayEpisode : null;
	const mappingNote: ResumeTarget['mappingNote'] = kitsuMatch ? 'direct' : 'no-kitsu-match';
	const uiPage = kitsuEpisode ? Math.max(1, Math.ceil(kitsuEpisode / EPISODES_UI_PAGE_SIZE)) : 1;

	return {
		displayTitle: stripped,
		searchTitle,
		displayEpisode,
		courSize,
		cour,
		kitsuId: kitsuMatch?.id ?? null,
		kitsuEpisode,
		uiPage,
		mappingNote
	};
}

/** Pick the best Kitsu hit for a multi-cour history entry. Kitsu's
 *  text search outranks the more-established Part 1 over its sequels
 *  even when the query carries the cour suffix, so the first hit is
 *  the wrong choice for Stone Ocean Part 2 / Part 3 etc. — it lands
 *  back on Part 1.
 *
 *  Picker walks four checks in priority order, falling through to
 *  the next when the previous yields no candidate:
 *    1. Exact slug match against a slug derived from searchTitle.
 *       Kitsu URLs are mechanical (e.g.
 *       `jojo-no-kimyou-na-bouken-part-6-stone-ocean-part-2`); when
 *       the slug we'd derive is one of the returned hits, we're
 *       sure that's the right entry.
 *    2. Cour token in slug — `…-part-N(-|$)` / `…-cour-N…` /
 *       `…-season-N…` — same anchor logic in slug form. Slug is
 *       always Latinscript, which dodges Japanese-titled hits.
 *    3. Cour token in canonical_title — `\b(part|cour|season)\s+N\b`
 *       with word-boundary anchoring so JoJo "Part 6" doesn't false-
 *       match cour 6 of an unrelated entry.
 *    4. First hit (the existing default).
 *  Single-cour entries skip 2-3 and use the slug-then-first-hit path. */
/** Mechanical slug derivation matching Kitsu's URL pattern: lowercase,
 *  non-alphanum runs collapsed to a single `-`, leading/trailing
 *  hyphens stripped. Exported because the title-match resolver also
 *  needs it for the slug-fallback IPC call. */
export function deriveSlug(s: string): string {
	return s
		.toLowerCase()
		.replace(/[^a-z0-9]+/g, '-')
		.replace(/^-+|-+$/g, '');
}

export function pickKitsuMatch(
	hits: KitsuAnimeRef[],
	preliminary: ResumeTarget
): KitsuAnimeRef | null {
	if (hits.length === 0) return null;

	const wantSlug = deriveSlug(preliminary.searchTitle);
	if (wantSlug.length >= 4) {
		const slugExact = hits.find((h) => h.slug === wantSlug);
		if (slugExact) return slugExact;
	}

	if (preliminary.cour <= 1) return hits[0];

	const slugRe = new RegExp(`(?:^|-)(?:part|cour|season)-${preliminary.cour}(?:-|$)`, 'i');
	const courInSlug = hits.find((h) => slugRe.test(h.slug ?? ''));
	if (courInSlug) return courInSlug;

	const titleRe = new RegExp(`\\b(?:part|cour|season)\\s+${preliminary.cour}\\b`, 'i');
	const courInTitle = hits.find((h) => titleRe.test(h.canonical_title ?? ''));
	if (courInTitle) return courInTitle;

	return hits[0];
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
