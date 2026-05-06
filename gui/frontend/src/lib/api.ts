/**
 * Typed wrappers around Tauri IPC commands. Every backend command surfaces
 * here as a Promise-returning function so the rest of the frontend never
 * touches `invoke()` directly.
 *
 * Field names mirror the Rust types byte-for-byte (snake_case) — the
 * `AniError` enum already serializes that way and changing the casing here
 * would break round-tripping. The TypeScript style guide for the project
 * accepts snake_case in IPC DTO interfaces only.
 */

import { invoke } from '@tauri-apps/api/core';

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
	return invoke<AppInfo>('cmd_app_info');
}

export function proxyBaseUrl(): Promise<string> {
	return invoke<string>('cmd_proxy_base_url');
}

export function historyList(): Promise<HistoryEntry[]> {
	return invoke<HistoryEntry[]>('cmd_history_list');
}

export function historyClear(): Promise<void> {
	return invoke<void>('cmd_history_clear');
}

export function createSession(args: CreateSessionArgs): Promise<CreateSessionResponse> {
	return invoke<CreateSessionResponse>('cmd_create_session', { args });
}

export function openExternalPlayer(args: LaunchExternalPlayerArgs): Promise<void> {
	return invoke<void>('cmd_open_external_player', { args });
}

export function kitsuSearch(query: string): Promise<KitsuAnimeRef[]> {
	return invoke<KitsuAnimeRef[]>('cmd_kitsu_search', { query });
}

export function kitsuAnimeDetail(id: string): Promise<KitsuAnimeRef> {
	return invoke<KitsuAnimeRef>('cmd_kitsu_anime_detail', { id });
}

export function kitsuTrending(): Promise<KitsuAnimeRef[]> {
	return invoke<KitsuAnimeRef[]>('cmd_kitsu_trending');
}

export function kitsuTopRated(): Promise<KitsuAnimeRef[]> {
	return invoke<KitsuAnimeRef[]>('cmd_kitsu_top_rated');
}

export function settingsGet(): Promise<Config> {
	return invoke<Config>('cmd_settings_get');
}

export function settingsPut(cfg: Config): Promise<void> {
	return invoke<void>('cmd_settings_put', { cfg });
}

/**
 * Convert an `https://media.kitsu.app/…` URL into the equivalent
 * `image://…` URL the Tauri custom protocol handles. Returns `null` if
 * the input isn't an https URL we can rewrite.
 */
export function imageProxyUrl(httpsUrl: string | null | undefined): string | null {
	if (!httpsUrl) return null;
	if (httpsUrl.startsWith('https://')) {
		return 'image://' + httpsUrl.slice('https://'.length);
	}
	return null;
}
