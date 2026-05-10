import { describe, it, expect, beforeEach, vi } from 'vitest';
import {
	__resetDeferredCancelsForTests,
	decideOnDestroyPrefetch,
	decideOnPipCloseOrphanedPrefetch,
	fireDeferredCancelsExcept,
	registerDeferredCancel,
	unregisterDeferredCancel
} from './prefetch-lifecycle';

describe('decideOnDestroyPrefetch', () => {
	it('clears immediately when no PiP is active', () => {
		// Plain navigation away — no floating thumbnail keeping the
		// user engaged with this show. Cancel the prefetch like before.
		expect(decideOnDestroyPrefetch({ videoIsInPip: false })).toBe('clear-now');
	});

	it('defers when the singleton video is in PiP', () => {
		// User navigated away but the floating thumbnail is still on
		// screen. Keeping prefetches alive means the next episode is
		// warm if they return-to-tab or if auto-play-next fires.
		expect(decideOnDestroyPrefetch({ videoIsInPip: true })).toBe('defer');
	});
});

describe('decideOnPipCloseOrphanedPrefetch', () => {
	it('clears when the user is no longer on /play/[id] for this show', () => {
		// PiP closed and the user is somewhere else — they truly
		// disengaged. Cancel the prefetch.
		expect(
			decideOnPipCloseOrphanedPrefetch({
				destroyedShowId: 'show-a',
				currentRouteId: '/search',
				currentShowId: ''
			})
		).toBe('clear');
	});

	it('clears when the user navigated to /play/[id] for a different show', () => {
		// User watched show-a, navigated to show-b's play page, then
		// closed PiP. The deferred cancel for show-a is still relevant
		// — show-b's mount manages its own prefetches independently.
		expect(
			decideOnPipCloseOrphanedPrefetch({
				destroyedShowId: 'show-a',
				currentRouteId: '/play/[id]',
				currentShowId: 'show-b'
			})
		).toBe('clear');
	});

	it('noops when the user is back on /play/[id] for the same show', () => {
		// User navigated away, came back, then closed PiP. The new
		// component owns the prefetches now; clearing would kill the
		// new mount's in-flight work.
		expect(
			decideOnPipCloseOrphanedPrefetch({
				destroyedShowId: 'show-a',
				currentRouteId: '/play/[id]',
				currentShowId: 'show-a'
			})
		).toBe('noop');
	});
});

describe('deferred-cancel registry', () => {
	beforeEach(() => {
		__resetDeferredCancelsForTests();
	});

	it('fireDeferredCancelsExcept fires every entry whose id differs from the given one', () => {
		const cancelA = vi.fn();
		const cancelB = vi.fn();
		const cancelC = vi.fn();
		registerDeferredCancel('show-a', cancelA);
		registerDeferredCancel('show-b', cancelB);
		registerDeferredCancel('show-c', cancelC);

		fireDeferredCancelsExcept('show-b');

		expect(cancelA).toHaveBeenCalledOnce();
		expect(cancelB).not.toHaveBeenCalled();
		expect(cancelC).toHaveBeenCalledOnce();
	});

	it('fireDeferredCancelsExcept removes the fired entries (idempotent on repeat)', () => {
		const cancelA = vi.fn();
		registerDeferredCancel('show-a', cancelA);

		fireDeferredCancelsExcept('show-other');
		fireDeferredCancelsExcept('show-other');

		expect(cancelA).toHaveBeenCalledOnce();
	});

	it('registering twice for the same id replaces the previous entry', () => {
		// A second deferred-cancel for the same show (e.g. a second
		// mount/unmount cycle while PiP is still active) supersedes
		// the first — the older listener is no longer relevant.
		const cancelOld = vi.fn();
		const cancelNew = vi.fn();
		registerDeferredCancel('show-a', cancelOld);
		registerDeferredCancel('show-a', cancelNew);

		fireDeferredCancelsExcept('show-other');

		expect(cancelOld).not.toHaveBeenCalled();
		expect(cancelNew).toHaveBeenCalledOnce();
	});

	it('unregisterDeferredCancel drops the entry without firing it', () => {
		// When a deferred-cancel listener fires through its own path
		// (PiP close), it should remove its entry rather than wait
		// for fireDeferredCancelsExcept.
		const cancelA = vi.fn();
		registerDeferredCancel('show-a', cancelA);
		unregisterDeferredCancel('show-a');

		fireDeferredCancelsExcept('show-other');

		expect(cancelA).not.toHaveBeenCalled();
	});
});
