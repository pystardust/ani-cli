// @vitest-environment happy-dom
//
// Svelte 5's `$state` rune compiles via the SvelteKit Vite plugin
// loaded by vitest.config.ts. The store class is a plain object —
// no component lifecycle — so we exercise it as ordinary TypeScript.
// happy-dom is required because Svelte's runtime initializes a
// `$.async_mode_flag` against `globalThis` and asserts a Document
// is available even outside components.
import { beforeEach, describe, expect, it } from 'vitest';
import { downloadStore } from './store.svelte';

describe('downloadStore', () => {
	beforeEach(() => {
		// The store is a module singleton. Reset by dismissing every
		// item so each test starts with an empty list.
		for (const item of [...downloadStore.items]) downloadStore.dismiss(item.id);
	});

	const baseArgs = {
		title: 'Demon Slayer',
		episode: '5',
		mode: 'sub',
		quality: '1080',
		destDir: '/tmp/dl'
	};

	it('add() prepends a pending row and returns its id', () => {
		const id = downloadStore.add(baseArgs);
		expect(id).toMatch(/^dl-\d+$/);
		expect(downloadStore.items).toHaveLength(1);
		const row = downloadStore.items[0];
		expect(row.id).toBe(id);
		expect(row.status).toBe('pending');
		expect(row.title).toBe('Demon Slayer');
		expect(row.episode).toBe('5');
		expect(row.rangeTotal).toBeNull();
		expect(row.currentEp).toBeNull();
	});

	it('add() infers rangeTotal from a "M-N" episode string', () => {
		// Range size is computed up front so the dock can display
		// "Episode K of N-M+1" before any progress arrives. 5..12 is 8.
		const id = downloadStore.add({ ...baseArgs, episode: '5-12' });
		const row = downloadStore.items.find((i) => i.id === id);
		expect(row?.rangeTotal).toBe(8);
	});

	it('rangeTotal stays null for non-range episode strings', () => {
		const id = downloadStore.add({ ...baseArgs, episode: '5' });
		expect(downloadStore.items.find((i) => i.id === id)?.rangeTotal).toBeNull();
	});

	it('markActive flips status, attaches the AbortController, and refreshes startedAt', () => {
		const id = downloadStore.add(baseArgs);
		const ctrl = new AbortController();
		const before = downloadStore.items[0].startedAt;
		// Force a measurable timestamp delta so the assertion is
		// meaningful even on fast machines.
		const stamp = before + 1000;
		const realNow = Date.now;
		Date.now = () => stamp;
		try {
			downloadStore.markActive(id, ctrl);
		} finally {
			Date.now = realNow;
		}
		const row = downloadStore.items[0];
		expect(row.status).toBe('active');
		expect(row.abort).toBe(ctrl);
		expect(row.startedAt).toBe(stamp);
	});

	it('setProgress stores the latest line and parses currentEp from "Playing episode N..."', () => {
		const id = downloadStore.add({ ...baseArgs, episode: '5-7' });
		downloadStore.setProgress(id, 'Playing episode 6 ...');
		expect(downloadStore.items[0].progress).toBe('Playing episode 6 ...');
		expect(downloadStore.items[0].currentEp).toBe(6);
	});

	it('setProgress preserves the last currentEp when a non-matching line arrives', () => {
		// `[download] 50%` shouldn't reset currentEp to null — only
		// `Playing episode N...` updates it. Prevents the dock label
		// flickering between "Episode 6" and an empty state on every
		// progress tick.
		const id = downloadStore.add({ ...baseArgs, episode: '5-7' });
		downloadStore.setProgress(id, 'Playing episode 6 ...');
		downloadStore.setProgress(id, '[download] 50% of 80MiB');
		expect(downloadStore.items[0].currentEp).toBe(6);
		expect(downloadStore.items[0].progress).toBe('[download] 50% of 80MiB');
	});

	it('handles half-episode progress like "Playing episode 1061.5..."', () => {
		const id = downloadStore.add(baseArgs);
		downloadStore.setProgress(id, 'Playing episode 1061.5 ...');
		expect(downloadStore.items[0].currentEp).toBe(1061.5);
	});

	it('markDone flips status, captures destDir, drops abort, marks unseen', () => {
		const id = downloadStore.add(baseArgs);
		downloadStore.markActive(id, new AbortController());
		downloadStore.markDone(id, '/var/anime');
		const row = downloadStore.items[0];
		expect(row.status).toBe('done');
		expect(row.destDir).toBe('/var/anime');
		expect(row.abort).toBeNull();
		expect(row.unseen).toBe(true);
	});

	it('markError captures the message and marks unseen for the dock badge', () => {
		const id = downloadStore.add(baseArgs);
		downloadStore.markError(id, 'upstream 503');
		const row = downloadStore.items[0];
		expect(row.status).toBe('error');
		expect(row.error).toBe('upstream 503');
		expect(row.abort).toBeNull();
		expect(row.unseen).toBe(true);
	});

	it('exposes `active` and `hasActive` derived views', () => {
		const a = downloadStore.add(baseArgs);
		const b = downloadStore.add(baseArgs);
		downloadStore.markActive(a, new AbortController());
		expect(downloadStore.hasActive).toBe(true);
		expect(downloadStore.active.map((r) => r.id).sort()).toEqual([a, b].sort());
		downloadStore.markDone(a, '/x');
		downloadStore.markDone(b, '/y');
		expect(downloadStore.active).toEqual([]);
		expect(downloadStore.hasActive).toBe(false);
	});

	it('unseenCount counts done / error rows the dock has not surfaced yet', () => {
		const a = downloadStore.add(baseArgs);
		const b = downloadStore.add(baseArgs);
		downloadStore.markDone(a, '/x');
		downloadStore.markError(b, 'boom');
		expect(downloadStore.unseenCount).toBe(2);
	});

	it('markAllSeen clears the unseen flag and is a no-op when nothing is unseen', () => {
		const a = downloadStore.add(baseArgs);
		downloadStore.markDone(a, '/x');
		downloadStore.markAllSeen();
		expect(downloadStore.unseenCount).toBe(0);
		// Second call must not allocate a new items array if every
		// row is already seen — guards a wasted state notification.
		const before = downloadStore.items;
		downloadStore.markAllSeen();
		expect(downloadStore.items).toBe(before);
	});

	it('cancel() aborts the controller and dismisses the row immediately', () => {
		const id = downloadStore.add(baseArgs);
		const ctrl = new AbortController();
		downloadStore.markActive(id, ctrl);
		const aborted = new Promise<void>((resolve) =>
			ctrl.signal.addEventListener('abort', () => resolve())
		);
		downloadStore.cancel(id);
		// Item is gone — the catch handler in start.ts will fire
		// after the abort propagates, but markError's .map finds
		// no item and is a no-op (stops the row from briefly
		// flashing red on cancel).
		expect(downloadStore.items.find((i) => i.id === id)).toBeUndefined();
		return expect(aborted).resolves.toBeUndefined();
	});

	it('cancel() of an unknown id is a safe no-op', () => {
		// The dock guards against double-clicks already, but even
		// a stale handler firing post-dismiss must not throw.
		expect(() => downloadStore.cancel('dl-nonexistent')).not.toThrow();
	});

	it('dismiss removes a row by id', () => {
		const id = downloadStore.add(baseArgs);
		downloadStore.dismiss(id);
		expect(downloadStore.items).toHaveLength(0);
	});

	it('setProgress leaves rows other than the targeted id untouched', () => {
		// Pins the identity branch in setProgress's .map — without
		// it, an event for one download would clobber the progress
		// line on every other row.
		const a = downloadStore.add(baseArgs);
		const b = downloadStore.add(baseArgs);
		downloadStore.setProgress(a, 'Playing episode 3 ...');
		const rowB = downloadStore.items.find((i) => i.id === b);
		expect(rowB?.progress).toBeNull();
		expect(rowB?.currentEp).toBeNull();
	});

	it('markAllSeen clears unseen on a mix of seen + unseen rows in one pass', () => {
		const a = downloadStore.add(baseArgs);
		const b = downloadStore.add(baseArgs);
		downloadStore.markDone(a, '/x'); // unseen=true
		downloadStore.markAllSeen(); // first pass clears a
		// b never went unseen — it's still pending. The second pass's
		// .map branch must keep it as-is.
		downloadStore.markDone(b, '/y'); // unseen=true again
		downloadStore.markAllSeen();
		expect(downloadStore.unseenCount).toBe(0);
	});

	it("cancel doesn't call .abort() when the row was still pending (no controller)", () => {
		// The early-out for `item.abort == null` exists so cancelling
		// a row that hadn't started yet doesn't crash on `.abort()`.
		const id = downloadStore.add(baseArgs);
		// Explicitly do NOT call markActive — abort stays null.
		expect(() => downloadStore.cancel(id)).not.toThrow();
		expect(downloadStore.items.find((i) => i.id === id)).toBeUndefined();
	});
});
