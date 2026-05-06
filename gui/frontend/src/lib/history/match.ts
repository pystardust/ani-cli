/**
 * Resolves a `ResumeTarget`'s kitsu match against the title-match
 * cache before falling back to a live `kitsuSearch` + `pickKitsuMatch`
 * round-trip. Cache hit → one IPC call (`kitsuAnimeDetail`, also
 * cached). Miss → search + pick + persist, so the next session
 * short-circuits.
 *
 * Errors at any layer fall through to the next strategy and ultimately
 * to `null` — the caller (Continue Watching cards) treats null as "no
 * Kitsu data, render the bare allmanga title as a card".
 */

import {
	kitsuAnimeBySlug,
	kitsuAnimeDetail,
	kitsuSearch,
	kitsuTitleMatchGet,
	kitsuTitleMatchPut,
	type KitsuAnimeRef
} from '$lib/api';
import { deriveSlug, pickKitsuMatch, type ResumeTarget } from './resolve';

export async function resolveKitsuMatch(preliminary: ResumeTarget): Promise<KitsuAnimeRef | null> {
	// 1) Cache lookup. If we've resolved this title→id before, fetch
	//    the (cached, 7d-TTL) detail and short-circuit.
	try {
		const cachedId = await kitsuTitleMatchGet(preliminary.searchTitle, preliminary.cour);
		if (cachedId) {
			try {
				return await kitsuAnimeDetail(cachedId);
			} catch {
				// Stale id (Kitsu removed the entry) — fall through to a
				// live search and re-cache.
			}
		}
	} catch {
		// Cache backend unavailable — degrade to live search.
	}

	let match: KitsuAnimeRef | null = null;

	// 2) Slug-first for multi-cour entries. Kitsu's `filter[text]`
	//    ranks the most-popular sibling and drops sequels with
	//    Japanese-romanized canonical titles entirely (Stone Ocean
	//    Part 2 is the canonical example: same franchise, different
	//    Kitsu entry, NOT in the text-search response). Our hsts
	//    title slugifies cleanly to Kitsu's URL pattern, so a direct
	//    slug lookup pinpoints the right entry. Single-cour entries
	//    skip this and go straight to the search path — slug-fetching
	//    every Continue Watching row would double the IPC volume on
	//    cold load.
	if (preliminary.cour > 1) {
		const slug = deriveSlug(preliminary.searchTitle);
		if (slug.length >= 4) {
			try {
				match = await kitsuAnimeBySlug(slug);
			} catch {
				// Slug-fetch failure is non-fatal; fall through to search.
			}
		}
	}

	// 3) Live search + pick. Either the slug fallback didn't apply
	//    (cour 1) or it didn't find an entry; let the picker work the
	//    text-search hits.
	if (!match) {
		try {
			const hits = await kitsuSearch(preliminary.searchTitle);
			match = pickKitsuMatch(hits, preliminary);
		} catch {
			return null;
		}
	}

	// 4) Persist on success so the next session bypasses the lookup.
	if (match) {
		try {
			await kitsuTitleMatchPut(preliminary.searchTitle, preliminary.cour, match.id);
		} catch {
			// Cache write failed — non-fatal, callers still get the match.
		}
	}

	return match;
}
