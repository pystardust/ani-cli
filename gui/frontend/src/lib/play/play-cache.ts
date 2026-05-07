import type { CreateSessionResponse, PlayProgress } from '$lib/api';

/**
 * In-flight + result cache for `play()` calls. Backs the prefetch
 * path on the detail / player pages: when the page mounts we fire
 * a play() request for the likely-next episode and store the promise
 * here; when the user later clicks, we return the same promise (or
 * its resolved value) instead of starting a fresh ani-cli spawn.
 *
 * The cache also tracks **progress subscribers**. The prefetch passes
 * `playStream`'s SSE events into the entry; a later click that races
 * an in-flight prefetch can subscribe via `getOrFire`'s `onProgress`
 * argument and receive the remaining events as ani-cli runs. The
 * latest event is replayed to new subscribers so they see what's
 * already happened (e.g. `youtube ✓`) and not just future events.
 *
 * The cache lives only for the renderer process — backing storage
 * for the resolution itself is the backend's SessionTable, which has
 * its own 4 h TTL. Navigating away from a show calls `clearForShow`
 * to release entries.
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

interface CacheEntry {
	promise: Promise<CreateSessionResponse>;
	/** Most recent progress event received from `fire`'s emit. Replayed
	 *  to new subscribers so a late `getOrFire` call sees `youtube ✓`
	 *  even if it joined after that event already streamed. */
	latestProgress: PlayProgress | null;
	/** Active progress callbacks. The producer's emit fan-outs to all
	 *  members. Subscribers add themselves on getOrFire and stay
	 *  attached until the entry is evicted. */
	subscribers: Set<(p: PlayProgress) => void>;
}

const cached = new Map<CacheKey, CacheEntry>();

/**
 * Cap on the number of `fire` calls running at once. Twelve concurrent
 * ani-cli spawns from a single page mount overloads the backend (CPU
 * contention + allanime rate-limit risk) and slows the user's own
 * click; queueing past the cap keeps the active set small while still
 * eventually warming every visible episode.
 *
 * Tunable: bump if backend SCRAPER_CONCURRENCY grows; lower if the
 * ratio of prefetched-but-unused entries becomes wasteful.
 */
const PREFETCH_CONCURRENCY = 2;
let activeFires = 0;
const fireQueue: Array<() => void> = [];

async function withSlot<T>(fn: () => Promise<T>): Promise<T> {
	if (activeFires >= PREFETCH_CONCURRENCY) {
		await new Promise<void>((resolve) => fireQueue.push(resolve));
	}
	activeFires += 1;
	try {
		return await fn();
	} finally {
		activeFires -= 1;
		const next = fireQueue.shift();
		if (next) next();
	}
}

/**
 * Look up an existing entry for `key` or fire a fresh one. Repeated
 * calls share the in-flight promise; once resolved, the value sticks
 * so a later call is instant.
 *
 * Progress events from the underlying stream are broadcast to every
 * caller that passed an `onProgress` callback for this key, in arrival
 * order. Late subscribers are replayed the most recent event so the
 * UI shows current state (`youtube ✓`) instead of waiting for the
 * next one.
 *
 * Failures drop the entry so a retry can fire fresh — a transient
 * upstream 403 / timeout shouldn't permanently mask the show.
 *
 * @param fire   Producer. Called only on the first hit for `key`.
 *               Receives an `emit` function that should be invoked
 *               with each progress event the resolution surfaces.
 * @param onProgress Optional consumer. Subscribes to progress
 *               events from the underlying stream — including a
 *               replay of the most recent event when this call
 *               attaches mid-flight.
 */
export function getOrFire(
	key: CacheKey,
	fire: (emit: (p: PlayProgress) => void) => Promise<CreateSessionResponse>,
	onProgress?: (p: PlayProgress) => void
): Promise<CreateSessionResponse> {
	let entry = cached.get(key);
	if (!entry) {
		const subscribers = new Set<(p: PlayProgress) => void>();
		const newEntry: CacheEntry = {
			promise: Promise.resolve({} as CreateSessionResponse), // placeholder, replaced below
			latestProgress: null,
			subscribers
		};
		const emit = (p: PlayProgress) => {
			newEntry.latestProgress = p;
			for (const s of subscribers) s(p);
		};
		newEntry.promise = withSlot(() => fire(emit));
		cached.set(key, newEntry);
		newEntry.promise.catch(() => {
			// Only drop if this is still the current entry — a race where
			// the same key was re-fired after rejection should leave the
			// newer attempt in place.
			if (cached.get(key) === newEntry) cached.delete(key);
		});
		entry = newEntry;
	}
	if (onProgress) {
		entry.subscribers.add(onProgress);
		// Replay the most recent event so a late subscriber sees the
		// state the rest of the band is already in.
		if (entry.latestProgress) onProgress(entry.latestProgress);
	}
	return entry.promise;
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
