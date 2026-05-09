/**
 * List-view availability gate. Reads the backend's cache via the
 * batch endpoint, drops cards we KNOW are unavailable, and fires a
 * background warm so the next visit's cache is fuller. The caller
 * never sees cards disappear mid-session — filtering is a snapshot
 * taken before render; warming runs concurrent and silent.
 */

import {
	altTitlesFromKitsu,
	availabilityBatch,
	availabilityWarm,
	checkAvailability
} from '$lib/api';
import type { KitsuAnimeRef } from '$lib/api';

/** Filter `items` against the availability cache, then warm uncached
 *  entries in the background. Returns the filtered list immediately;
 *  the warm Promise is intentionally swallowed (fire-and-forget). */
export async function filterAvailable<T extends KitsuAnimeRef>(
	items: T[],
	mode: 'sub' | 'dub'
): Promise<T[]> {
	if (items.length === 0) return items;
	const ids = items.map((i) => i.id);
	let cached: Record<string, boolean> = {};
	try {
		const r = await availabilityBatch(ids, mode);
		cached = r.cached;
	} catch {
		// Cache fetch failed — render everything; lazy click path
		// still surfaces real errors.
		return items;
	}
	const filtered = items.filter((i) => cached[i.id] !== false);

	// Fire-and-forget warm for any item not in the cache. Skipping
	// items whose availability is already known keeps the queue
	// short.
	const toWarm = items
		.filter((i) => !(i.id in cached))
		.map((i) => ({
			title: i.canonical_title,
			mode,
			alt_titles: altTitlesFromKitsu(i),
			episode_count: i.episode_count ?? undefined,
			kitsu_id: i.id
		}));
	if (toWarm.length > 0) {
		void availabilityWarm(toWarm).catch(() => {});
	}

	return filtered;
}

/** Strict variant: probes uncached items inline (parallel, capped
 *  concurrency) before returning. Use on surfaces where the user
 *  is actively waiting for results — e.g. search — and would
 *  rather wait a beat than see unavailable cards rendered. Home
 *  uses the fire-and-forget {@link filterAvailable} so cards
 *  don't disappear mid-session. */
export async function filterAvailableStrict<T extends KitsuAnimeRef>(
	items: T[],
	mode: 'sub' | 'dub',
	concurrency = 4
): Promise<T[]> {
	if (items.length === 0) return items;
	const ids = items.map((i) => i.id);
	let cached: Record<string, boolean> = {};
	try {
		const r = await availabilityBatch(ids, mode);
		cached = r.cached;
	} catch {
		return items;
	}

	const uncached = items.filter((i) => !(i.id in cached));
	if (uncached.length > 0) {
		const queue = uncached.slice();
		const workers = Array.from({ length: Math.min(concurrency, queue.length) }, async () => {
			while (queue.length > 0) {
				const item = queue.shift();
				if (!item) break;
				try {
					const r = await checkAvailability({
						title: item.canonical_title,
						mode,
						alt_titles: altTitlesFromKitsu(item),
						episode_count: item.episode_count ?? undefined,
						kitsu_id: item.id
					});
					cached[item.id] = r.available;
				} catch {
					// Probe failed — leave unset so we render the card
					// (lazy click path will surface the real error).
				}
			}
		});
		await Promise.all(workers);
	}

	return items.filter((i) => cached[i.id] !== false);
}
