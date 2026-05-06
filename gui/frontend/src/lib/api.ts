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

/** Output of `cmd_create_session` — proxy URLs the player should fetch. */
export interface CreateSessionResponse {
	session_id: string;
	master_url: string;
	subtitle_url: string | null;
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
