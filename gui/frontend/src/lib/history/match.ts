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
	kitsuAnimeDetail,
	kitsuSearch,
	kitsuTitleMatchGet,
	kitsuTitleMatchPut,
	type KitsuAnimeRef
} from '$lib/api';
import { pickKitsuMatch, type ResumeTarget } from './resolve';

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

	// 2) Live search + pick. Mirrors the home page's previous logic
	//    inline; the picker handles cour disambiguation.
	let match: KitsuAnimeRef | null;
	try {
		const hits = await kitsuSearch(preliminary.searchTitle);
		match = pickKitsuMatch(hits, preliminary);
	} catch {
		return null;
	}

	// 3) Persist on success so the next session bypasses the search.
	if (match) {
		try {
			await kitsuTitleMatchPut(preliminary.searchTitle, preliminary.cour, match.id);
		} catch {
			// Cache write failed — non-fatal, callers still get the match.
		}
	}

	return match;
}
