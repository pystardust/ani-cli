/**
 * Fullscreen idle-hide for the player controls + cursor.
 *
 * Outside fullscreen the existing CSS `.player-frame:hover` rule
 * is enough — the user can move the cursor off the frame and the
 * controls fade out. In fullscreen the frame *is* the entire
 * screen, so `:hover` is always true and the controls never go
 * away. This helper drives a JS-side idle decision that the
 * component layer maps to a class on `.player-frame`, which the
 * CSS uses to override the `:hover` rule and to hide the cursor
 * (`cursor: none`) — the chrome the user expects from any other
 * fullscreen video player.
 */

import { describe, it, expect } from 'vitest';
import { shouldHideControlsInFullscreen, FULLSCREEN_IDLE_HIDE_MS } from './fullscreen-idle';

const base = {
	mouseIdle: true,
	paused: false,
	scrubberHover: false,
	focusWithin: false
} as const;

describe('shouldHideControlsInFullscreen', () => {
	it('hides when the mouse is idle and the video is playing', () => {
		// The canonical case — user has been still for more than the
		// idle threshold and there's no interactive UI keeping the
		// chrome live.
		expect(shouldHideControlsInFullscreen({ ...base })).toBe(true);
	});

	it('shows whenever the mouse is active', () => {
		// Active wins over every "still showing for some other
		// reason" rule — moving the mouse should always reveal the
		// controls so the user can interact.
		expect(shouldHideControlsInFullscreen({ ...base, mouseIdle: false })).toBe(false);
	});

	it('shows when the video is paused even if idle', () => {
		// Pausing typically means the user is reading the timeline
		// or comparing to subs — keeping controls visible matches
		// what every native player does on pause.
		expect(shouldHideControlsInFullscreen({ ...base, paused: true })).toBe(false);
	});

	it('shows while the scrubber is hovered', () => {
		// scrubber-hover is sticky for the whole drag interaction;
		// we don't want the bar to vanish mid-drag because the
		// pointer didn't generate a mousemove for half a second.
		expect(shouldHideControlsInFullscreen({ ...base, scrubberHover: true })).toBe(false);
	});

	it('shows when focus is inside the controls', () => {
		// Keyboard users who arrow-tabbed onto the play button
		// shouldn't lose their visual cue when they stop typing.
		expect(shouldHideControlsInFullscreen({ ...base, focusWithin: true })).toBe(false);
	});

	it('hides only when ALL keep-alive reasons are absent', () => {
		// Defensive: any keep-alive flag wins over idle. This
		// pinned matrix is the spec the component implements.
		const inputs = [
			{ ...base, mouseIdle: false, paused: false, scrubberHover: false, focusWithin: false },
			{ ...base, mouseIdle: true, paused: true, scrubberHover: false, focusWithin: false },
			{ ...base, mouseIdle: true, paused: false, scrubberHover: true, focusWithin: false },
			{ ...base, mouseIdle: true, paused: false, scrubberHover: false, focusWithin: true },
			{ ...base, mouseIdle: false, paused: true, scrubberHover: true, focusWithin: true }
		];
		for (const s of inputs) {
			expect(shouldHideControlsInFullscreen(s)).toBe(false);
		}
	});

	it('exports a 2500 ms idle threshold', () => {
		// 2.5 s is the value the component imports for its
		// setTimeout — pin it here so any future tweak surfaces
		// in this test file too. Native HTML5 <video controls>
		// uses ~3 s in fullscreen; YouTube uses ~3 s; we go a
		// touch tighter so the chrome stays out of the way.
		expect(FULLSCREEN_IDLE_HIDE_MS).toBe(2500);
	});
});
