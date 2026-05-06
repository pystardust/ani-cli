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
