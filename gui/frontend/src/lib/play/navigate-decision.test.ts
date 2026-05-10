import { describe, it, expect } from 'vitest';
import { decideNavigateAction, type NavigateDecisionInput } from './navigate-decision';

const base: NavigateDecisionInput = {
	targetRoute: '/',
	targetShowId: '',
	currentShowId: 'kid-42',
	videoPaused: false,
	alreadyInPip: false,
	disableAutoPip: false
};

describe('decideNavigateAction', () => {
	it('noops when navigating to a different episode of the same show', () => {
		expect(
			decideNavigateAction({
				...base,
				targetRoute: '/play/[id]',
				targetShowId: 'kid-42'
			})
		).toBe('noop');
	});

	it('does NOT noop when navigating to a different show on the play route', () => {
		// /play/[id] only counts as a same-show swap when the id
		// matches; a different id is treated like leaving the player
		// (this is the bug fix from "different-show click bypassed PiP").
		expect(
			decideNavigateAction({
				...base,
				targetRoute: '/play/[id]',
				targetShowId: 'kid-99'
			})
		).toBe('request-pip');
	});

	it('requests PiP even when the video is paused', () => {
		// Previously paused → noop, but that meant a paused video silently
		// disappeared on navigation. Now the user keeps the floating
		// thumbnail (still paused) and can resume from there.
		expect(decideNavigateAction({ ...base, videoPaused: true })).toBe('request-pip');
	});

	it('noops when the user is already in PiP (no duplicate request)', () => {
		expect(decideNavigateAction({ ...base, alreadyInPip: true })).toBe('noop');
	});

	it('pauses when auto-PiP is disabled in settings', () => {
		// Without this branch, disabling auto-PiP would leave the
		// off-screen singleton playing audio with no visible video.
		expect(decideNavigateAction({ ...base, disableAutoPip: true })).toBe('pause');
	});

	it('requests PiP by default when leaving to a non-play route', () => {
		expect(decideNavigateAction({ ...base, targetRoute: '/anime/[id]' })).toBe('request-pip');
	});

	it('requests PiP when leaving to a different show on the play route', () => {
		expect(
			decideNavigateAction({
				...base,
				targetRoute: '/play/[id]',
				targetShowId: 'kid-99'
			})
		).toBe('request-pip');
	});

	it('disable flag does not override the in-PiP guard', () => {
		// `pause` only kicks in when the video would otherwise be
		// PiP'd. If we're already in PiP, there's no duplicate request
		// to pause around.
		expect(decideNavigateAction({ ...base, alreadyInPip: true, disableAutoPip: true })).toBe(
			'noop'
		);
	});

	it('same-show swap takes precedence over every other guard', () => {
		// Episode-to-episode navigation is purely an in-place src swap;
		// nothing about the current playback state should change.
		expect(
			decideNavigateAction({
				...base,
				targetRoute: '/play/[id]',
				targetShowId: 'kid-42',
				videoPaused: true,
				alreadyInPip: true,
				disableAutoPip: true
			})
		).toBe('noop');
	});
});
