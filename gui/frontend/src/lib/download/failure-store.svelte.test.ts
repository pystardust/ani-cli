// @vitest-environment happy-dom
//
// Svelte 5 runes via SvelteKit Vite plugin (vitest.config.ts).
// happy-dom for the `$state` runtime — see store.svelte.test.ts.
import { beforeEach, describe, expect, it } from 'vitest';
import { downloadFailureStore } from './failure-store.svelte';

describe('downloadFailureStore', () => {
	beforeEach(() => {
		// Module singleton — reset by dismissing any current payload.
		downloadFailureStore.dismiss();
	});

	it('starts with no current payload', () => {
		expect(downloadFailureStore.current).toBeNull();
	});

	it('show() sets current to the supplied payload', () => {
		downloadFailureStore.show({ kind: 'ffmpeg_missing' });
		expect(downloadFailureStore.current).toEqual({ kind: 'ffmpeg_missing' });
	});

	it('dismiss() clears current to null', () => {
		downloadFailureStore.show({ kind: 'ffmpeg_missing' });
		downloadFailureStore.dismiss();
		expect(downloadFailureStore.current).toBeNull();
	});

	it('show() replaces an existing payload — latest failure wins', () => {
		// Two ffmpeg_missing back-to-back are almost always the same
		// root cause. Don't queue them; just keep the latest.
		downloadFailureStore.show({ kind: 'ffmpeg_missing' });
		downloadFailureStore.show({ kind: 'ffmpeg_missing' });
		expect(downloadFailureStore.current).toEqual({ kind: 'ffmpeg_missing' });
	});

	it('dismiss() is idempotent when nothing is open', () => {
		expect(() => downloadFailureStore.dismiss()).not.toThrow();
		expect(downloadFailureStore.current).toBeNull();
	});
});
