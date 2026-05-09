/**
 * Singleton `<video>` element that survives SvelteKit page
 * navigations so Picture-in-Picture stays alive when the user
 * leaves /play/[id].
 *
 * The Fullscreen API and PiP API both bind to a specific
 * HTMLVideoElement instance. If that element is removed from the
 * DOM, the PiP window closes. Svelte tears down the page's
 * `<video>` on navigation, which kills PiP. This module sidesteps
 * that by parking the video in a hidden host attached to
 * `document.body` — body lives outside Svelte's tree, so the
 * element survives any number of route changes.
 *
 * The play route is a "controller" for the singleton: on mount it
 * attaches the singleton into its player frame; on destroy it
 * detaches it back to the hidden host (so PiP carries on).
 *
 * Imperative-style API on purpose — Svelte's `bind:` directives
 * don't span ownership boundaries, so we manage the element by
 * hand. Pages listen to its events to keep their reactive state
 * in sync.
 */

import type { MediaKind } from '$lib/api';

/** Where the singleton lives between play-page mounts. Off-screen
 *  fixed-positioned div so the element can't be observed visually
 *  but stays in the DOM (and PiP keeps showing the picture). */
let hostEl: HTMLDivElement | null = null;
let videoEl: HTMLVideoElement | null = null;

/** Last known session — what the singleton's `src` is bound to.
 *  The layout's leavepictureinpicture handler reads this so it
 *  can navigate back to /play/[kitsu_id] when PiP closes off-route. */
export interface VideoSession {
	kitsu_id: string;
	episode: number;
	session_id: string;
	media_url: string;
	media_kind: MediaKind;
	subtitle_url: string | null;
}
let currentSession: VideoSession | null = null;

function ensureCreated(): HTMLVideoElement {
	if (videoEl) return videoEl;
	if (typeof document === 'undefined') {
		throw new Error('global-video accessed in a non-DOM environment');
	}
	hostEl = document.createElement('div');
	hostEl.id = 'ani-gui-video-host';
	// Off-screen so the element renders but isn't visible. PiP
	// keeps drawing because the element is still laid out — only
	// the on-screen pixels are off-canvas.
	hostEl.style.position = 'fixed';
	hostEl.style.inset = '0 0 auto auto';
	hostEl.style.inlineSize = '1px';
	hostEl.style.blockSize = '1px';
	hostEl.style.overflow = 'hidden';
	hostEl.style.opacity = '0';
	hostEl.style.pointerEvents = 'none';
	hostEl.style.zIndex = '-1';
	document.body.appendChild(hostEl);

	videoEl = document.createElement('video');
	videoEl.autoplay = true;
	videoEl.preload = 'auto';
	videoEl.style.inlineSize = '100%';
	videoEl.style.blockSize = '100%';
	videoEl.style.display = 'block';
	videoEl.style.background = '#000';
	hostEl.appendChild(videoEl);
	return videoEl;
}

/** Get (or lazily create) the singleton video. */
export function getGlobalVideo(): HTMLVideoElement {
	return ensureCreated();
}

/** Move the singleton into `parent`. Idempotent — calling with
 *  the same parent twice is a no-op. */
export function attachGlobalVideoTo(parent: HTMLElement): HTMLVideoElement {
	const v = ensureCreated();
	if (v.parentElement !== parent) {
		parent.appendChild(v);
	}
	return v;
}

/** Move the singleton back to the hidden host. Called from the
 *  play page's onDestroy so the element survives navigation. */
export function detachGlobalVideo(): void {
	if (!videoEl || !hostEl) return;
	if (videoEl.parentElement !== hostEl) {
		hostEl.appendChild(videoEl);
	}
}

/** Read the most recent session the singleton was loaded with.
 *  The layout's PiP-exit handler uses this to navigate back. */
export function getCurrentSession(): VideoSession | null {
	return currentSession;
}

export function setCurrentSession(s: VideoSession | null): void {
	currentSession = s;
}

/** Replace the subtitle track on the singleton. Removes any
 *  existing `<track>` children and adds a fresh one when `url` is
 *  non-null. The play page's effect calls this when the session's
 *  subtitle URL changes. */
export function setSubtitleTrack(url: string | null): void {
	const v = ensureCreated();
	for (const t of Array.from(v.querySelectorAll('track'))) t.remove();
	if (url) {
		const track = document.createElement('track');
		track.kind = 'subtitles';
		track.label = 'Subtitles';
		track.srclang = 'en';
		track.src = url;
		track.default = true;
		v.appendChild(track);
	}
}
