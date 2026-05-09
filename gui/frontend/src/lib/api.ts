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
		aniGui?: {
			apiBase?: string;
			/** Open a native folder picker dialog. Returns the chosen
			 *  absolute path or null on cancel. */
			pickDirectory?: (options?: {
				title?: string;
				defaultPath?: string;
			}) => Promise<string | null>;
			/** Open the OS file manager at `dirPath`. Returns true on
			 *  success. */
			revealInFolder?: (dirPath: string) => Promise<boolean>;
		};
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
	// Tolerate any 2xx with an empty body (the external-player
	// endpoint returns 202 Accepted with no JSON to parse). `.json()`
	// throws "Unexpected end of JSON input" on an empty body, which
	// surfaced as a confusing toast even though the action succeeded.
	if (resp.status === 204) return undefined as T;
	const text = await resp.text();
	if (text.length === 0) return undefined as T;
	return JSON.parse(text) as T;
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
	auto_play_next: boolean;
	download_bottom_bar_enabled: boolean;
	auto_skip_op: boolean;
	auto_skip_ed: boolean;
	use_custom_player_controls: boolean;
	auto_pip_on_leave: boolean;
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

/** Find the most-recent history entry whose allmanga show_id maps to
 *  this Kitsu id, via the (allmanga show_id → kitsu_id) reverse cache
 *  the play path stamps on each successful resolve. Used by the detail
 *  page to swap "Play episode 1" for "Continue · Episode N+1" when
 *  the user has watched this show before. Returns `null` when there's
 *  no such mapped entry — including the case where the mapping cache
 *  is cold (CLI-only history rows the GUI hasn't played yet). */
export function historyByKitsu(kitsuId: string): Promise<HistoryEntry | null> {
	return getJson<HistoryEntry | null>(`/api/history/by-kitsu/${encodeURIComponent(kitsuId)}`);
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
	/** Kitsu id of the anime the user is playing. Frontend passes the
	 *  id from `/anime/[kitsu_id]`'s URL so the backend can persist
	 *  an `(allmanga show_id → kitsu_id)` reverse mapping. The home
	 *  page's Continue Watching strip then looks Kitsu up by show_id
	 *  instead of fuzzy-text-searching the (sometimes typo'd)
	 *  allmanga title. Optional; missing on legacy click sites that
	 *  haven't been updated yet. */
	kitsu_id?: string;
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

/** Stamp Continue Watching for the just-clicked episode.
 *
 *  Why this exists separately from playStream: page-mount prefetches
 *  fire playStream with prefetch=true so the backend skips its
 *  history write (whichever prefetch resolves last would otherwise
 *  clobber the user's actual click). When the user then clicks an
 *  episode that's already cached, getOrFire returns the prefetch's
 *  in-flight promise — the backend never sees a prefetch=false call,
 *  so the history line never gets written.
 *
 *  Click handlers fire markWatched as a side-effect after getOrFire
 *  resolves. Best-effort: caller should `void markWatched(...).catch(()=>{})`. */
export function markWatched(args: PlayArgs): Promise<void> {
	return postJson<void>('/api/play/mark-watched', args);
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

/** Wire payload for {@link downloadStream}. Mirrors PlayArgs but with
 *  an explicit `download_dir` chosen by the user via the folder picker.
 *  When omitted, the backend falls back to `paths::download_dir()`. */
export interface DownloadArgs {
	title: string;
	episode: string;
	mode: string;
	quality?: string;
	episode_count?: number;
	alt_titles?: string[];
	kitsu_id?: string;
	download_dir?: string;
}

/** SSE progress event body — one raw stderr line from aria2c / yt-dlp /
 *  ffmpeg. The renderer stores the latest line and the dock surfaces it
 *  under the active row. */
export interface DownloadProgress {
	line: string;
}

/** SSE final event body — the directory the file landed in, suitable
 *  for the completion toast's "reveal in folder" intent. */
export interface DownloadResponse {
	dest_dir: string;
}

/** Streaming download. Same shape as {@link playStream}: opens an SSE
 *  connection to GET /api/download/stream, fires `onProgress` for every
 *  forwarded ani-cli stderr line, resolves with the destination dir on
 *  the `done` event, rejects on `error` or close-before-done. The
 *  `signal` lets callers cancel mid-download — closing the SSE drops
 *  the spawned ani-cli child via Tokio's `kill_on_drop(true)`. */
export function downloadStream(
	args: DownloadArgs,
	onProgress: (p: DownloadProgress) => void,
	signal?: AbortSignal
): Promise<DownloadResponse> {
	if (typeof EventSource === 'undefined') {
		return Promise.reject(new Error('downloadStream: EventSource unavailable'));
	}
	const params = new URLSearchParams();
	params.set('title', args.title);
	params.set('episode', args.episode);
	params.set('mode', args.mode);
	if (args.quality) params.set('quality', args.quality);
	if (typeof args.episode_count === 'number')
		params.set('episode_count', String(args.episode_count));
	if (args.alt_titles && args.alt_titles.length > 0)
		params.set('alt_titles', args.alt_titles.join('\n'));
	if (args.kitsu_id) params.set('kitsu_id', args.kitsu_id);
	if (args.download_dir) params.set('download_dir', args.download_dir);

	return apiBase().then(
		(base) =>
			new Promise<DownloadResponse>((resolve, reject) => {
				const url = `${base.replace(/\/+$/, '')}/api/download/stream?${params.toString()}`;
				const es = new EventSource(url);
				let settled = false;
				const finish = (fn: () => void) => {
					if (settled) return;
					settled = true;
					es.close();
					fn();
				};
				if (signal) {
					const onAbort = () => finish(() => reject(new Error('downloadStream: aborted')));
					if (signal.aborted) onAbort();
					else signal.addEventListener('abort', onAbort, { once: true });
				}
				es.addEventListener('progress', (ev) => {
					try {
						onProgress(JSON.parse((ev as MessageEvent).data) as DownloadProgress);
					} catch {
						// Malformed payloads only stall the dock, not the download.
					}
				});
				es.addEventListener('done', (ev) => {
					try {
						const resp = JSON.parse((ev as MessageEvent).data) as DownloadResponse;
						finish(() => resolve(resp));
					} catch (e) {
						finish(() => reject(e));
					}
				});
				es.addEventListener('error', (ev) => {
					const data = (ev as MessageEvent).data;
					let payload: unknown;
					let parsed = false;
					if (typeof data === 'string' && data.length > 0) {
						try {
							payload = JSON.parse(data);
							parsed = true;
						} catch {
							/* fall through */
						}
					}
					finish(() => {
						if (parsed) reject(payload);
						else reject(new Error('Download stream closed before completion.'));
					});
				});
			})
	);
}

/** Default destination directory the download confirm modal opens at.
 *  Returns null when neither $XDG_DOWNLOAD_DIR nor $HOME is available
 *  on the backend — the modal then asks the user to pick a path
 *  explicitly. */
export function downloadDefaultDir(): Promise<string | null> {
	return getJson<string | null>('/api/download/default-dir');
}

/** Wire payload for {@link checkAvailability}. Same title/mode/alt
 *  shape PlayArgs uses so the detail page can pass through what it
 *  already gathered. `kitsu_id` keys the cache; `episode_count`
 *  feeds the picker's disambiguation. */
export interface AvailabilityArgs {
	title: string;
	mode: string;
	alt_titles?: string[];
	episode_count?: number;
	kitsu_id?: string;
	/** Kitsu's airing status — one of "current", "finished",
	 *  "upcoming", "tba", "unreleased". Branches the positive cache
	 *  TTL: ongoing shows refresh in 24h, finished in 30d. Optional;
	 *  unknown status defaults to the short TTL. */
	status?: string;
}

/** Response from {@link checkAvailability}. `episode_count` is the
 *  highest INTEGER episode allmanga has streamable in the requested
 *  mode — authoritative cap for the resume CTA, Download All, and
 *  episode-strip pagination. `extra_episodes` is the list of
 *  non-integer tags allmanga lists (recap / special episodes —
 *  e.g. `["1061.5"]` for One Piece). The detail/play pages splice
 *  these into the episode strip at their numeric position so the
 *  user can navigate to them. */
export interface AvailabilityResponse {
	available: boolean;
	episode_count: number | null;
	extra_episodes: string[];
}

/** "Is this title in allmanga's catalog?" probe. The detail page hits
 *  this on mount so it can gate the Play + Download CTAs ahead of a
 *  click instead of letting the user discover the gap by clicking. */
export function checkAvailability(args: AvailabilityArgs): Promise<AvailabilityResponse> {
	return postJson<AvailabilityResponse>('/api/availability', args);
}

/** Cached-only batch lookup. List views (home / search) call this
 *  before render to filter out cards whose availability is already
 *  cached as `false`. Missing entries in the response are titles
 *  whose availability is unknown — the caller renders them as-is. */
export function availabilityBatch(
	kitsu_ids: string[],
	mode: string
): Promise<{ cached: Record<string, boolean> }> {
	return postJson<{ cached: Record<string, boolean> }>('/api/availability/batch', {
		kitsu_ids,
		mode
	});
}

/** Fire-and-forget cache warmer. Hands the backend a list of titles
 *  to probe in the background; the cache is populated for the next
 *  list-view visit. List views never wait for the warm to finish. */
export function availabilityWarm(items: AvailabilityArgs[]): Promise<void> {
	return postJson<unknown>('/api/availability/warm', { items }).then(() => undefined);
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

/** AniList-backed trending — recency-weighted (last few days of
 *  viewer activity). Backend bridges each AniList entry to a Kitsu
 *  ref via /mappings, so the response shape matches the regular
 *  trending endpoint. Falls back to {@link kitsuTrending} when
 *  AniList is unreachable; the page still loads either way. */
export function kitsuTrendingAnilist(): Promise<KitsuAnimeRef[]> {
	return getJson<KitsuAnimeRef[]>('/api/kitsu/trending-anilist');
}

/** One skip interval for the embedded player's Skip OP / Skip
 *  Outro button. Times are seconds (sub-second precision). */
export interface SkipInterval {
	/** `"op"` | `"ed"` | `"mixed-op"` | `"mixed-ed"` | `"recap"`. */
	skip_type: string;
	start_time: number;
	end_time: number;
}

/** Aniskip skip-times lookup. Player calls this on
 *  `loadedmetadata`, passing the video's duration in seconds.
 *  Returns an empty array when Kitsu has no MAL mapping for the
 *  show or aniskip has no skip times catalogued — both are
 *  normal, the player just doesn't render the button. */
export function aniskipGet(
	kitsuId: string,
	episode: string,
	episodeLength: number
): Promise<SkipInterval[]> {
	const qs = new URLSearchParams({ episode_length: String(episodeLength) });
	return getJson<SkipInterval[]>(
		`/api/aniskip/${encodeURIComponent(kitsuId)}/${encodeURIComponent(episode)}?${qs.toString()}`
	);
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

/**
 * Reverse-direction mapping: given an allmanga show_id (from
 * ani-hsts column 2), look up the kitsu_id the user previously
 * played it as. Returns `null` on miss. The mapping is recorded by
 * the backend during `mark-watched` whenever the frontend supplies
 * `kitsu_id` in PlayArgs — so any past click on `/anime/[id]/play`
 * has already populated it.
 *
 * Continue Watching's resolver hits this BEFORE the legacy title-
 * match path because allmanga's catalog has typos (e.g. "Nato:
 * Shippuuden" for Naruto Shippuuden) that Kitsu's text search
 * can't recover from. Show_id is unambiguous; the title isn't.
 */
export function allmangaKitsuMapGet(showId: string): Promise<string | null> {
	return getJson<string | null>(`/api/allmanga-kitsu-map/${encodeURIComponent(showId)}`);
}

/**
 * Resolve an allmanga show_id to its full Kitsu entry by walking
 * allmanga's `Show` GraphQL aliases (englishName / nativeName /
 * altNames) through Kitsu's text search. Returns null when no Kitsu
 * match is found OR when the upstream HTTP fails.
 *
 * Use this as the LAST step in the Continue Watching resolver, when
 * the reverse cache and the title-search both yield nothing — e.g.
 * fresh-from-cache-clear renders where allmanga's stub `name`
 * (`"1P"` for One Piece, `"Nato: Shippuuden"` for Naruto Shippuuden)
 * has no Kitsu text-search hit. The backend persists the resolved
 * mapping into the reverse cache so subsequent calls short-circuit.
 */
export function kitsuResolveAllmangaShowId(showId: string): Promise<KitsuAnimeRef | null> {
	return getJson<KitsuAnimeRef | null>(`/api/kitsu/resolve-allmanga/${encodeURIComponent(showId)}`);
}

/**
 * Per-show last-watched millis-since-epoch map. Populated by the
 * backend's `mark-watched` handler whenever the user clicks an
 * episode through the GUI; CLI plays bypass it. Drives the home
 * page Continue Watching sort: stamped rows on top (most recent
 * first), unstamped rows at the bottom in original file order.
 */
export function watchedAtAll(): Promise<Record<string, number>> {
	return getJson<Record<string, number>>('/api/watched-at');
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
 * Wipe the on-disk image byte cache (`~/.cache/ani-gui/images/`).
 * Does NOT touch the SQLite metadata cache. Useful when the byte
 * cache has accumulated stale images from a Kitsu re-cataloguing
 * or when the user wants to reclaim disk before the LRU prune
 * gets a chance.
 */
export function imageCacheClear(): Promise<void> {
	return deleteJson<void>('/api/cache/images');
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
