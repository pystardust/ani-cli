import type { CreateSessionResponse } from '$lib/api';

/**
 * In-flight + result cache for `play()` calls. Backs the prefetch
 * path on the detail / player pages: when the page mounts we fire
 * a play() request for the likely-next episode and store the promise
 * here; when the user later clicks, we return the same promise (or
 * its resolved value) instead of starting a fresh ani-cli spawn.
 *
 * The cache lives only for the renderer process — backing storage
 * for the resolution itself is the backend's SessionTable, which has
 * its own 4 h TTL. We don't try to be clever about expiring entries
 * here; navigating away from a show calls `clearForShow(showId)` to
 * release them.
 */

/** Opaque cache key. Constructed via {@link makeKey}. */
export type CacheKey = string & { readonly __brand: 'PlayCacheKey' };

/**
 * Build a stable, deterministic key from the four axes that decide
 * whether two `play()` requests would resolve to the same session.
 * Mode + quality matter because the backend resolves a different
 * embed URL for each.
 */
export function makeKey(showId: string, episode: number, mode: string, quality: string): CacheKey {
	return `${showId}|${episode}|${mode}|${quality}` as CacheKey;
}

const cached = new Map<CacheKey, Promise<CreateSessionResponse>>();

/**
 * Look up an existing entry for `key` or fire a fresh one. Repeated
 * calls during the same in-flight resolution share the same promise;
 * once resolved, the value sticks around so a later call is instant.
 *
 * Failures drop the entry so a retry can fire fresh — a transient
 * upstream 403 / timeout shouldn't permanently mask the show.
 */
export function getOrFire(
	key: CacheKey,
	fire: () => Promise<CreateSessionResponse>
): Promise<CreateSessionResponse> {
	const existing = cached.get(key);
	if (existing) return existing;

	const promise = fire();
	cached.set(key, promise);
	promise.catch(() => {
		// Only drop if this is still the current promise — a race where
		// the same key was re-fired after rejection should leave the
		// newer attempt in place.
		if (cached.get(key) === promise) cached.delete(key);
	});
	return promise;
}

/**
 * Drop every cache entry for the given show. Called on detail-page /
 * player-page unmount so prefetched-but-unused sessions don't leak
 * into a future visit (the backend GCs them after 4 h regardless).
 */
export function clearForShow(showId: string): void {
	const prefix = `${showId}|`;
	for (const key of cached.keys()) {
		if (key.startsWith(prefix)) cached.delete(key);
	}
}

/** Test seam: wipe the whole cache between vitest cases. */
export function __resetPlayCacheForTests(): void {
	cached.clear();
}
