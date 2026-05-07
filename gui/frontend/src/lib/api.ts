/**
 * Typed wrappers around the Rust backend's HTTP API.
 *
 * The Electron renderer talks to the Rust sidecar via `fetch()`
 * against its localhost axum server. Every backend command surfaces
 * here as a Promise-returning function.
 *
 * Field names mirror the Rust types byte-for-byte (snake_case)
 * because `AniError` and the DTOs serialize that way; changing
 * casing here would break round-tripping. The TypeScript style guide
 * accepts snake_case in API DTO interfaces only.
 */

/**
 * Resolve the base URL of the local backend (e.g.
 * `http://127.0.0.1:42337`). Cached after the first call.
 *
 * Detection order:
 *   1. `window.aniGui.apiBase` — Electron preload script injection.
 *   2. `import.meta.env.VITE_ANI_GUI_API_BASE` — browser-only dev
 *      (run `cargo run --bin ani-gui-backend` and `pnpm dev` with
 *      the env var set to the printed URL).
 *
 * Throws if neither is available — the renderer can't function
 * without a backend address.
 */
let apiBaseCache: string | null = null;

declare global {
	interface Window {
		aniGui?: { apiBase?: string };
	}
}

async function apiBase(): Promise<string> {
	if (apiBaseCache !== null) return apiBaseCache;
	if (typeof window !== 'undefined' && window.aniGui?.apiBase) {
		apiBaseCache = window.aniGui.apiBase;
		return apiBaseCache;
	}
	const envBase =
		typeof import.meta !== 'undefined' ? import.meta.env?.VITE_ANI_GUI_API_BASE : undefined;
	if (typeof envBase === 'string' && envBase.length > 0) {
		apiBaseCache = envBase;
		return apiBaseCache;
	}
	throw new Error(
		'ani-gui apiBase is not configured — Electron preload should set window.aniGui.apiBase, ' +
			'or set VITE_ANI_GUI_API_BASE for browser-only dev.'
	);
}

/**
 * Test seam: reset the cached base URL so unit tests can rebind it.
 * Not exported in production paths — only used by `api.test.ts`.
 */
export function __resetApiBaseForTests(next: string | null = null): void {
	apiBaseCache = next;
}

/**
 * Internal: build a URL by joining the cached base with a path. Path
 * is taken verbatim, so callers control any query-string encoding.
 */
async function url(path: string): Promise<string> {
	const base = await apiBase();
	return base.replace(/\/+$/, '') + path;
}

/**
 * Read the JSON body or throw the parsed error payload. The backend
 * serializes `AniError` as `{ kind, key?, detail?, status? }`; tests
 * downstream of api.ts inspect the same shape that Tauri's reject
 * payloads carried, so call-site error parsing is unchanged.
 */
async function expect2xx<T>(resp: Response): Promise<T> {
	if (!resp.ok) {
		let detail: unknown;
		try {
			detail = await resp.json();
		} catch {
			detail = { kind: 'http', status: resp.status };
		}
		throw detail;
	}
	if (resp.status === 204) return undefined as T;
	return (await resp.json()) as T;
}

async function getJson<T>(path: string): Promise<T> {
	const resp = await fetch(await url(path));
	return expect2xx<T>(resp);
}

async function postJson<T>(path: string, body: unknown): Promise<T> {
	const resp = await fetch(await url(path), {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify(body)
	});
	return expect2xx<T>(resp);
}

async function putJson<T>(path: string, body: unknown): Promise<T> {
	const resp = await fetch(await url(path), {
		method: 'PUT',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify(body)
	});
	return expect2xx<T>(resp);
}

async function deleteJson<T>(path: string): Promise<T> {
	const resp = await fetch(await url(path), { method: 'DELETE' });
	return expect2xx<T>(resp);
}

/** Output of `cmd_app_info`. */
export interface AppInfo {
	version: string;
	ani_cli_path: string;
	history_path: string;
	proxy_base_url: string;
}

/** One row from the shared `ani-hsts` file. */
export interface HistoryEntry {
	ep_no: string;
	id: string;
	title: string;
}

/** Input to `cmd_create_session`. */
export interface CreateSessionArgs {
	upstream_url: string;
	referer: string;
	subtitle_url?: string | null;
}

/** What kind of media the resolved upstream serves. The renderer uses
 *  this to pick hls.js vs. a plain `<video src>` for direct MP4 streams
 *  (wixmp / sharepoint / fast4speed) that don't have an HLS manifest. */
export type MediaKind = 'hls' | 'mp4';

/** Output of `cmd_create_session` — proxy URLs the player should fetch. */
export interface CreateSessionResponse {
	session_id: string;
	/** Full proxy URL the player should fetch. For HLS sessions this
	 *  points at `…/s/<id>/master.m3u8`; for MP4 sessions at
	 *  `…/s/<id>/file.mp4`. The frontend never composes the path itself. */
	media_url: string;
	/** Tells the renderer which player to mount around `media_url`. */
	media_kind: MediaKind;
	subtitle_url: string | null;
	/** True when this play resolution came from the long-term cache
	 *  (no fresh ani-cli spawn). The play page uses it to decide
	 *  whether a player error is silently retryable: cache hits can
	 *  be evicted + re-resolved; fresh fetches already exhausted the
	 *  resolve path so the user should see the error. Optional for
	 *  forward compat with older backends — treat absence as `false`. */
	cache_hit?: boolean;
}

/** Input to `cmd_open_external_player`. */
export interface LaunchExternalPlayerArgs {
	stream_url: string;
	referer?: string | null;
	subtitle_url?: string | null;
	title?: string | null;
	player_command: string;
}

/**
 * Shape of `AniError` once Tauri serializes it as the rejection value.
 * Frontend localizers look up `key` (when present) in the i18n catalog.
 */
export interface AniErrorPayload {
	kind: string;
	key?: string;
	detail?: string;
	status?: number;
}

/** Portrait poster URLs (5:7) at Kitsu's pre-rendered sizes. */
export interface KitsuPosterImage {
	tiny: string | null;
	small: string | null;
	medium: string | null;
	large: string | null;
	original: string | null;
}

/** Banner cover URLs (21:5). No `medium` key — Kitsu doesn't expose one. */
export interface KitsuCoverImage {
	tiny: string | null;
	small: string | null;
	large: string | null;
	original: string | null;
}

/** User-editable settings persisted to ~/.config/ani-gui/config.toml. */
export interface Config {
	locale: string;
	mode: string; // "sub" | "dub"
	quality: string; // "best" | "worst" | "1080" | "720" | "480"
	external_player: string;
	image_cache_cap_mb: number;
}

/** Single-size thumbnail Kitsu exposes for episodes (no tiny/small variants). */
export interface KitsuEpisodeThumbnail {
	original: string | null;
}

/** One episode in a Kitsu anime's episode list. */
export interface KitsuEpisode {
	id: string;
	canonical_title: string | null;
	season_number: number | null;
	number: number | null;
	relative_number: number | null;
	length: number | null;
	synopsis: string | null;
	airdate: string | null;
	thumbnail: KitsuEpisodeThumbnail | null;
}

/** Public Kitsu anime view returned by `cmd_kitsu_search` / `_anime_detail`. */
export interface KitsuAnimeRef {
	id: string;
	canonical_title: string;
	/** Localized title variants Kitsu serves under `attributes.titles`.
	 *  Common keys: `en`, `en_jp` (romanized JP), `en_us`, `ja_jp`
	 *  (kana). Missing keys are absent from the map entirely. The play
	 *  flow uses these to retry allanime lookups when the canonical
	 *  title (often English) doesn't match allmanga's index — see
	 *  {@link altTitlesFromKitsu}. May be missing on cached responses
	 *  produced before titles were surfaced; treat as `?:`. */
	titles?: Record<string, string>;
	slug: string | null;
	synopsis: string | null;
	start_date: string | null;
	end_date: string | null;
	episode_count: number | null;
	average_rating: number | null;
	subtype: string | null;
	status: string | null;
	age_rating: string | null;
	popularity_rank: number | null;
	poster_image: KitsuPosterImage | null;
	cover_image: KitsuCoverImage | null;
}

/**
 * Build the fallback-title list for a play call from a Kitsu ref.
 * Returns the localized variants (`en_jp`, `ja_jp`, `en`, `en_us`)
 * that aren't already the canonical, in priority order: romanized
 * Japanese first because allmanga indexes shows under that form, then
 * raw kana, then English alternates.
 *
 * Empty / null-ish titles are dropped. The output is deduped so the
 * backend never makes redundant allanime queries.
 */
export function altTitlesFromKitsu(ref: KitsuAnimeRef | null | undefined): string[] {
	if (!ref?.titles) return [];
	const seen = new Set<string>([ref.canonical_title]);
	const out: string[] = [];
	for (const key of ['en_jp', 'ja_jp', 'en', 'en_us']) {
		const v = ref.titles[key];
		if (typeof v === 'string' && v.length > 0 && !seen.has(v)) {
			seen.add(v);
			out.push(v);
		}
	}
	return out;
}

export function appInfo(): Promise<AppInfo> {
	return getJson<AppInfo>('/api/app-info');
}

export function proxyBaseUrl(): Promise<string> {
	return getJson<string>('/api/proxy-base-url');
}

export function historyList(): Promise<HistoryEntry[]> {
	return getJson<HistoryEntry[]>('/api/history');
}

export function historyClear(): Promise<void> {
	return deleteJson<void>('/api/history');
}

export function createSession(args: CreateSessionArgs): Promise<CreateSessionResponse> {
	return postJson<CreateSessionResponse>('/api/sessions', args);
}

export function openExternalPlayer(args: LaunchExternalPlayerArgs): Promise<void> {
	return postJson<void>('/api/external-player', args);
}

/** Body for the play endpoints — same shape on both `/api/play` and
 *  `/api/play/external`. The backend takes the canonical title (from
 *  Kitsu metadata), resolves it through ani-cli, and either wraps the
 *  result in a session or hands it to the OS player. */
export interface PlayArgs {
	title: string;
	episode: string;
	mode: 'sub' | 'dub';
	quality?: string;
	/** Kitsu's authoritative episode count. The backend uses this to
	 *  disambiguate allanime candidates that share a title — e.g.
	 *  picking the 500-ep "Naruto: Shippuden" main show over the
	 *  1-ep side story. Optional: if missing, the backend falls back
	 *  to allanime's first match. */
	episode_count?: number | null;
	/** Fallback titles to try when the canonical title returns no
	 *  allanime hits. Build with {@link altTitlesFromKitsu}. The
	 *  backend walks them in order and stops at the first non-empty
	 *  search result. Used to recover Stone Ocean Part 6 and similar
	 *  shows whose Kitsu canonical disagrees with allmanga's index. */
	alt_titles?: string[];
	/** `true` when the call is a background prefetch (warming the
	 *  cache for an episode the user hasn't clicked yet). The backend
	 *  uses it to skip Continue Watching updates — prefetches resolve
	 *  in arbitrary order, so the last one to finish would otherwise
	 *  overwrite the user's actual click. Click handlers leave this
	 *  unset (defaults to false on the wire). */
	prefetch?: boolean;
}

/** Play an episode in the embedded player. Returns the session URLs
 *  the renderer feeds to hls.js after navigating to /play. */
export function play(args: PlayArgs): Promise<CreateSessionResponse> {
	return postJson<CreateSessionResponse>('/api/play', args);
}

/** Play an episode in the user's external media player (default mpv).
 *  No session is registered — the player streams from the upstream
 *  directly with the resolved Referer. */
export function playExternal(args: PlayArgs): Promise<void> {
	return postJson<void>('/api/play/external', args);
}

/** Drop the cached play resolution for `args` so the next play call
 *  cache-misses and re-runs ani-cli. Used by the player error path:
 *  if the cached upstream URL 4xx'd (rotated *after* our HEAD said
 *  it was alive), the renderer evicts and silently retries. */
export function evictPlayCache(args: PlayArgs): Promise<void> {
	return postJson<void>('/api/play/cache/evict', args);
}

/** One incremental progress event surfaced by `playStream`. Mirrors the
 *  Rust `ProgressLine` enum's tagged JSON shape. */
export type PlayProgress =
	| { kind: 'banner'; text: string }
	| { kind: 'links_fetched'; provider: string }
	| { kind: 'other'; text: string };

/** Streaming variant of {@link play}: opens an SSE connection so the
 *  caller hears `<provider> Links Fetched` events as ani-cli emits
 *  them. Resolves with the same `CreateSessionResponse` `play()`
 *  returns; rejects on server-side errors or when the stream closes
 *  without a `done` event.
 *
 *  Falls back to a plain `play()` POST when `EventSource` isn't
 *  available — e.g. server-side rendering or older webviews. The
 *  `onProgress` callback simply never fires in that case.
 *
 *  When `signal` is provided and aborted, the EventSource is closed
 *  and the promise rejects with an "aborted" error. Used by
 *  play-cache's `clearForShow` to cancel abandoned prefetches when
 *  the user navigates away from a show. */
export function playStream(
	args: PlayArgs,
	onProgress: (p: PlayProgress) => void,
	signal?: AbortSignal
): Promise<CreateSessionResponse> {
	if (typeof EventSource === 'undefined') {
		return play(args);
	}
	const params = new URLSearchParams();
	params.set('title', args.title);
	params.set('episode', args.episode);
	params.set('mode', args.mode);
	if (args.quality) params.set('quality', args.quality);
	if (typeof args.episode_count === 'number')
		params.set('episode_count', String(args.episode_count));
	// alt_titles is a Vec<String> on the backend. serde_urlencoded can't
	// decode that from repeated keys, so we join with `\n` and the
	// backend's custom deserializer splits on the same separator.
	// Newline is unlikely in any real title and survives URL encoding.
	if (args.alt_titles && args.alt_titles.length > 0)
		params.set('alt_titles', args.alt_titles.join('\n'));
	if (args.prefetch === true) params.set('prefetch', '1');

	return apiBase().then(
		(base) =>
			new Promise<CreateSessionResponse>((resolve, reject) => {
				const url = `${base.replace(/\/+$/, '')}/api/play/stream?${params.toString()}`;
				const es = new EventSource(url);
				let settled = false;
				const finish = (fn: () => void) => {
					if (settled) return;
					settled = true;
					es.close();
					fn();
				};
				// Cancellation: abort during a streaming resolve closes
				// the EventSource and rejects. If the signal is already
				// aborted, finish synchronously before any listeners
				// attach. Listening on the signal once captures both
				// cases without leaking — `finish` is idempotent.
				if (signal) {
					const onAbort = () => finish(() => reject(new Error('playStream: aborted')));
					if (signal.aborted) onAbort();
					else signal.addEventListener('abort', onAbort, { once: true });
				}
				es.addEventListener('progress', (ev) => {
					try {
						onProgress(JSON.parse((ev as MessageEvent).data) as PlayProgress);
					} catch {
						// Ignore malformed progress payloads — overlay just
						// stops updating; the resolution still completes.
					}
				});
				es.addEventListener('done', (ev) => {
					try {
						const resp = JSON.parse((ev as MessageEvent).data) as CreateSessionResponse;
						finish(() => resolve(resp));
					} catch (e) {
						finish(() => reject(e));
					}
				});
				es.addEventListener('error', (ev) => {
					// Both `error` events emitted by the server (with data)
					// and EventSource transport errors land here. Carry the
					// parsed payload when present; otherwise surface a
					// generic error. Parsing happens outside `finish` so
					// a malformed payload doesn't strand the promise — the
					// throw used to fire after `settled = true` was set,
					// making the fall-through `finish()` a no-op.
					const data = (ev as MessageEvent).data;
					let payload: unknown;
					let parsed = false;
					if (typeof data === 'string' && data.length > 0) {
						try {
							payload = JSON.parse(data);
							parsed = true;
						} catch {
							/* fall through to generic error */
						}
					}
					finish(() => {
						if (parsed) reject(payload);
						else reject(new Error('Stream closed before resolution finished.'));
					});
				});
			})
	);
}

export function kitsuSearch(query: string): Promise<KitsuAnimeRef[]> {
	return postJson<KitsuAnimeRef[]>('/api/kitsu/search', { query });
}

export function kitsuAnimeDetail(id: string): Promise<KitsuAnimeRef> {
	return getJson<KitsuAnimeRef>(`/api/kitsu/anime/${encodeURIComponent(id)}`);
}

/**
 * Look up an anime by its Kitsu slug. Returns `null` when no entry
 * matches. The picker uses this as a last-resort fallback when text
 * search misses a sequel — Kitsu's `filter[text]` drops Stone Ocean
 * Part 2 from its results entirely, but `filter[slug]` finds it.
 */
export function kitsuAnimeBySlug(slug: string): Promise<KitsuAnimeRef | null> {
	return getJson<KitsuAnimeRef | null>(`/api/kitsu/anime-by-slug/${encodeURIComponent(slug)}`);
}

export function kitsuTrending(): Promise<KitsuAnimeRef[]> {
	return getJson<KitsuAnimeRef[]>('/api/kitsu/trending');
}

export function kitsuTopRated(): Promise<KitsuAnimeRef[]> {
	return getJson<KitsuAnimeRef[]>('/api/kitsu/top-rated');
}

export function kitsuEpisodes(animeId: string, page: number = 1): Promise<KitsuEpisode[]> {
	const qs = new URLSearchParams({ page: String(page) });
	return getJson<KitsuEpisode[]>(
		`/api/kitsu/episodes/${encodeURIComponent(animeId)}?${qs.toString()}`
	);
}

/**
 * Read the cached `(title, cour) → kitsu_id` mapping for a Continue
 * Watching row. Returns `null` on miss; caller should fall through to
 * a fresh `kitsuSearch` + `pickKitsuMatch` and `kitsuTitleMatchPut`
 * the resolved id back. The backend stores this under TITLE_MATCH_TTL
 * (30 days) — the title→id mapping rarely changes.
 */
export function kitsuTitleMatchGet(title: string, cour: number): Promise<string | null> {
	const qs = new URLSearchParams({ title, cour: String(cour) });
	return getJson<string | null>(`/api/title-match?${qs.toString()}`);
}

/**
 * Persist a `(title, cour) → kitsu_id` mapping resolved by the
 * frontend picker. Idempotent — re-puts overwrite any prior value.
 */
export function kitsuTitleMatchPut(title: string, cour: number, kitsuId: string): Promise<void> {
	return putJson<void>('/api/title-match', { title, cour, kitsu_id: kitsuId });
}

export function settingsGet(): Promise<Config> {
	return getJson<Config>('/api/settings');
}

export function settingsPut(cfg: Config): Promise<void> {
	return putJson<void>('/api/settings', cfg);
}

/**
 * Wipe the SQLite metadata cache (Kitsu responses + title-match
 * mappings). Does NOT touch the ani-cli history file or the on-disk
 * image cache. Used by the diagnostics "Clear metadata cache" button
 * when cached data goes stale.
 */
export function metaCacheClear(): Promise<void> {
	return deleteJson<void>('/api/cache');
}

/**
 * Convert an `https://…` upstream image URL into a URL the renderer
 * can drop into `<img src=>`.
 *
 * The backend serves `/api/image?url=…` over HTTP; we need the
 * apiBase synchronously (Svelte renders the `src` attribute eagerly),
 * so we read it directly off `window.aniGui.apiBase` set by the
 * Electron preload before any component mounts.
 *
 * Returns `null` for non-https input or when the apiBase isn't
 * available yet — components show a placeholder in that case.
 */
export function imageProxyUrl(httpsUrl: string | null | undefined): string | null {
	if (!httpsUrl || !httpsUrl.startsWith('https://')) return null;
	if (typeof window === 'undefined' || !window.aniGui?.apiBase) return null;
	return `${window.aniGui.apiBase}/api/image?url=${encodeURIComponent(httpsUrl)}`;
}
