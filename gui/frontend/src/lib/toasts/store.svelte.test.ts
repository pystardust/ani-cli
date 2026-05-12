// @vitest-environment happy-dom
//
// Svelte 5 runes via SvelteKit Vite plugin (vitest.config.ts).
// happy-dom for the `$state` runtime — see download/store.svelte.test.ts.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { toastStore, TOAST_MAX_STACK } from './store.svelte';

describe('toastStore', () => {
	beforeEach(() => {
		vi.useFakeTimers();
		// Module singleton — drain between tests so each starts empty.
		for (const item of [...toastStore.items]) toastStore.dismiss(item.id);
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it('push() returns an id and appends a row with the supplied fields', () => {
		const id = toastStore.push({
			kind: 'success',
			message: 'Opened in mpv.',
			duration: 4000
		});
		expect(id).toMatch(/^toast-\d+$/);
		expect(toastStore.items).toHaveLength(1);
		const row = toastStore.items[0];
		expect(row.id).toBe(id);
		expect(row.kind).toBe('success');
		expect(row.message).toBe('Opened in mpv.');
		expect(row.duration).toBe(4000);
		expect(row.actionLabel).toBeNull();
		expect(row.onAction).toBeNull();
	});

	it('push() defaults duration to 4000ms when omitted', () => {
		toastStore.push({ kind: 'info', message: 'Hello.' });
		expect(toastStore.items[0].duration).toBe(4000);
	});

	it('push() carries an optional action label + handler', () => {
		const handler = vi.fn();
		toastStore.push({
			kind: 'error',
			message: 'Boom.',
			actionLabel: 'Retry',
			onAction: handler
		});
		const row = toastStore.items[0];
		expect(row.actionLabel).toBe('Retry');
		expect(row.onAction).toBe(handler);
	});

	it('auto-dismiss timer drains the row when duration elapses', () => {
		toastStore.push({ kind: 'info', message: 'Vanishing.', duration: 2000 });
		expect(toastStore.items).toHaveLength(1);
		vi.advanceTimersByTime(1999);
		expect(toastStore.items).toHaveLength(1);
		vi.advanceTimersByTime(1);
		expect(toastStore.items).toHaveLength(0);
	});

	it('duration: null pins the toast (no auto-dismiss)', () => {
		toastStore.push({ kind: 'warning', message: 'Sticky.', duration: null });
		vi.advanceTimersByTime(60_000);
		expect(toastStore.items).toHaveLength(1);
	});

	it('dismiss(id) removes the matching row immediately', () => {
		const a = toastStore.push({ kind: 'info', message: 'A', duration: 9999 });
		const b = toastStore.push({ kind: 'info', message: 'B', duration: 9999 });
		toastStore.dismiss(a);
		expect(toastStore.items.map((r) => r.id)).toEqual([b]);
	});

	it('dismiss(unknown) is a no-op (does not throw)', () => {
		expect(() => toastStore.dismiss('toast-nope')).not.toThrow();
	});

	it('trims to TOAST_MAX_STACK when a push would exceed the cap (oldest drops)', () => {
		// Cap is 3 today. Pushing 4 should leave only the last 3 visible.
		const ids: string[] = [];
		for (let i = 0; i < TOAST_MAX_STACK + 1; i++) {
			ids.push(toastStore.push({ kind: 'info', message: `t${i}`, duration: 9999 }));
		}
		expect(toastStore.items).toHaveLength(TOAST_MAX_STACK);
		// First-pushed should have been dropped; the last TOAST_MAX_STACK survive in order.
		const survivingIds = toastStore.items.map((r) => r.id);
		expect(survivingIds).toEqual(ids.slice(1));
	});

	it('auto-dismiss timer of a trimmed-out row does not affect surviving rows', () => {
		// A pushed row that's trimmed before its timer fires should not
		// take a sibling down with it. Verifies the timer's dismiss(id)
		// path is a no-op when the id is already gone.
		for (let i = 0; i < TOAST_MAX_STACK + 1; i++) {
			toastStore.push({ kind: 'info', message: `t${i}`, duration: 2000 });
		}
		expect(toastStore.items).toHaveLength(TOAST_MAX_STACK);
		vi.advanceTimersByTime(2000);
		// All TOAST_MAX_STACK + 1 timers fire; the trimmed-out one's
		// dismiss is a no-op, the surviving three dismiss themselves.
		expect(toastStore.items).toHaveLength(0);
	});
});
