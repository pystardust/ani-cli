import { afterEach, describe, expect, it, vi } from 'vitest';
import type { CreateSessionResponse, PlayProgress } from '$lib/api';
import { __resetPlayCacheForTests, clearForShow, getOrFire, makeKey } from './play-cache';

function fakeResp(seed: string): CreateSessionResponse {
	return {
		session_id: seed,
		media_url: `http://x/s/${seed}/master.m3u8`,
		media_kind: 'hls',
		subtitle_url: null
	};
}

/** A `fire` that doesn't emit progress — equivalent to the legacy
 *  signature. Keeps tests focused on caching behaviour. */
function plainFire(
	resp: CreateSessionResponse | Error
): (emit: (p: PlayProgress) => void) => Promise<CreateSessionResponse> {
	return () => (resp instanceof Error ? Promise.reject(resp) : Promise.resolve(resp));
}

afterEach(() => {
	__resetPlayCacheForTests();
});

describe('makeKey', () => {
	it('is stable for the same inputs', () => {
		expect(makeKey('show-1', 1, 'sub', 'best')).toBe(makeKey('show-1', 1, 'sub', 'best'));
	});

	it('differs across each axis', () => {
		const base = makeKey('show-1', 1, 'sub', 'best');
		expect(makeKey('show-2', 1, 'sub', 'best')).not.toBe(base);
		expect(makeKey('show-1', 2, 'sub', 'best')).not.toBe(base);
		expect(makeKey('show-1', 1, 'dub', 'best')).not.toBe(base);
		expect(makeKey('show-1', 1, 'sub', '1080')).not.toBe(base);
	});
});

describe('getOrFire', () => {
	it('returns the same in-flight promise for repeated calls', async () => {
		const fire = vi.fn(plainFire(fakeResp('a')));
		const k = makeKey('show-1', 1, 'sub', 'best');

		const p1 = getOrFire(k, fire);
		const p2 = getOrFire(k, fire);

		expect(p1).toBe(p2);
		await p1;
		expect(fire).toHaveBeenCalledTimes(1);
	});

	it('caches a successful resolution so a later call returns instantly', async () => {
		const fire = vi.fn(plainFire(fakeResp('a')));
		const k = makeKey('show-1', 1, 'sub', 'best');

		const first = await getOrFire(k, fire);
		const second = await getOrFire(k, fire);

		expect(second).toEqual(first);
		expect(fire).toHaveBeenCalledTimes(1);
	});

	it('drops failed entries so a retry can fire fresh', async () => {
		const fire = vi
			.fn<(emit: (p: PlayProgress) => void) => Promise<CreateSessionResponse>>()
			.mockRejectedValueOnce(new Error('boom'))
			.mockResolvedValueOnce(fakeResp('ok'));
		const k = makeKey('show-1', 1, 'sub', 'best');

		await expect(getOrFire(k, fire)).rejects.toThrow('boom');
		const recovered = await getOrFire(k, fire);

		expect(recovered.session_id).toBe('ok');
		expect(fire).toHaveBeenCalledTimes(2);
	});

	it('keeps separate entries for different keys', async () => {
		const fireA = vi.fn(plainFire(fakeResp('a')));
		const fireB = vi.fn(plainFire(fakeResp('b')));

		const a = await getOrFire(makeKey('show-1', 1, 'sub', 'best'), fireA);
		const b = await getOrFire(makeKey('show-1', 2, 'sub', 'best'), fireB);

		expect(a.session_id).toBe('a');
		expect(b.session_id).toBe('b');
		expect(fireA).toHaveBeenCalledTimes(1);
		expect(fireB).toHaveBeenCalledTimes(1);
	});

	it('broadcasts progress events to subscribers in arrival order', async () => {
		const k = makeKey('show-1', 1, 'sub', 'best');
		let emitFn: ((p: PlayProgress) => void) | null = null;
		const fire = vi.fn(
			(emit: (p: PlayProgress) => void) =>
				new Promise<CreateSessionResponse>((resolve) => {
					emitFn = emit;
					// Resolves only when test calls emitFn(done) below.
					setTimeout(() => resolve(fakeResp('a')), 0);
				})
		);

		const subA: PlayProgress[] = [];
		const subB: PlayProgress[] = [];
		const promise = getOrFire(k, fire, (p) => subA.push(p));
		// Second subscriber joins via getOrFire (typical: the prefetch
		// fired first with a no-op callback, then a click subscribes).
		getOrFire(k, fire, (p) => subB.push(p));

		expect(emitFn).toBeTruthy();
		emitFn!({ kind: 'links_fetched', provider: 'youtube' });
		emitFn!({ kind: 'links_fetched', provider: 'sharepoint' });

		await promise;
		expect(subA).toEqual([
			{ kind: 'links_fetched', provider: 'youtube' },
			{ kind: 'links_fetched', provider: 'sharepoint' }
		]);
		expect(subB).toEqual(subA);
		// fire only ran once — both subscribers share the underlying stream.
		expect(fire).toHaveBeenCalledTimes(1);
	});

	it('replays the most recent event to a late subscriber', async () => {
		const k = makeKey('show-1', 1, 'sub', 'best');
		let emitFn: ((p: PlayProgress) => void) | null = null;
		const fire = vi.fn(
			(emit: (p: PlayProgress) => void) =>
				new Promise<CreateSessionResponse>((resolve) => {
					emitFn = emit;
					setTimeout(() => resolve(fakeResp('a')), 0);
				})
		);

		// Prefetch fires with a no-op subscriber.
		getOrFire(k, fire);
		emitFn!({ kind: 'links_fetched', provider: 'youtube' });
		emitFn!({ kind: 'links_fetched', provider: 'sharepoint' });

		// Click subscribes mid-flight: should immediately receive the
		// most recent event, then any future events.
		const lateLog: PlayProgress[] = [];
		const promise = getOrFire(k, fire, (p) => lateLog.push(p));

		expect(lateLog).toEqual([{ kind: 'links_fetched', provider: 'sharepoint' }]);

		emitFn!({ kind: 'links_fetched', provider: 'wixmp' });
		await promise;
		expect(lateLog).toEqual([
			{ kind: 'links_fetched', provider: 'sharepoint' },
			{ kind: 'links_fetched', provider: 'wixmp' }
		]);
	});
});

describe('PREFETCH_CONCURRENCY semaphore', () => {
	it('queues fires beyond the cap and only releases when an in-flight one settles', async () => {
		// PREFETCH_CONCURRENCY = 2 (see play-cache.ts). Fire 4 entries
		// at once: the first two should run immediately, the latter
		// two queue at the `await new Promise(resolve => fireQueue.push(resolve))`
		// branch until a slot frees up.
		let runningNow = 0;
		let peakConcurrent = 0;
		const releasers: Array<() => void> = [];
		const fire = (seed: string) => () =>
			new Promise<CreateSessionResponse>((resolve) => {
				runningNow += 1;
				peakConcurrent = Math.max(peakConcurrent, runningNow);
				releasers.push(() => {
					runningNow -= 1;
					resolve(fakeResp(seed));
				});
			});

		const p1 = getOrFire(makeKey('s', 1, 'sub', 'best'), fire('a'));
		const p2 = getOrFire(makeKey('s', 2, 'sub', 'best'), fire('b'));
		const p3 = getOrFire(makeKey('s', 3, 'sub', 'best'), fire('c'));
		const p4 = getOrFire(makeKey('s', 4, 'sub', 'best'), fire('d'));

		// Yield enough microtasks for the first two to enter their fire()
		// bodies; the last two should still be waiting on the queue.
		await Promise.resolve();
		await Promise.resolve();
		expect(releasers).toHaveLength(2);
		expect(runningNow).toBe(2);

		// Drain in order. Releasing one slot should immediately admit
		// the next queued fire on the next microtask.
		releasers[0]();
		await p1;
		await Promise.resolve();
		expect(releasers).toHaveLength(3);

		releasers[1]();
		await p2;
		await Promise.resolve();
		expect(releasers).toHaveLength(4);

		releasers[2]();
		releasers[3]();
		await Promise.all([p3, p4]);

		// Cap held at 2 throughout — the queue did its job.
		expect(peakConcurrent).toBe(2);
	});
});

describe('priority subscriber promotion', () => {
	it('cuts a queued entry to the head when a click subscribes', async () => {
		// PREFETCH_CONCURRENCY = 2. Two prefetches saturate the cap; a
		// third queues. A user click on the third (passing an onProgress
		// callback) should promote it to run immediately rather than
		// wait for one of the active two to drain — otherwise the
		// loading overlay sits idle while ani-cli's queue clears.
		const releasers: Array<() => void> = [];
		const fire = (seed: string) => () =>
			new Promise<CreateSessionResponse>((resolve) => {
				releasers.push(() => resolve(fakeResp(seed)));
			});

		// Two prefetches saturate the cap.
		void getOrFire(makeKey('s', 1, 'sub', 'best'), fire('a'));
		void getOrFire(makeKey('s', 2, 'sub', 'best'), fire('b'));
		await Promise.resolve();
		await Promise.resolve();
		expect(releasers).toHaveLength(2);

		// Third prefetch — queues, doesn't run.
		void getOrFire(makeKey('s', 3, 'sub', 'best'), fire('c'));
		await Promise.resolve();
		await Promise.resolve();
		expect(releasers).toHaveLength(2);

		// User clicks ep 3 → attach an onProgress subscriber. The cache
		// should detect this is a click and start the queued fire now.
		void getOrFire(makeKey('s', 3, 'sub', 'best'), fire('c'), () => {});
		await Promise.resolve();
		await Promise.resolve();
		expect(releasers).toHaveLength(3); // ep 3 is running

		// Drain so the test exits cleanly.
		releasers.forEach((r) => r());
	});

	it('is a no-op when the entry has already started running', async () => {
		// A click on an actively-running entry should not double-fire.
		const fire = vi.fn(plainFire(fakeResp('a')));
		const k = makeKey('show', 1, 'sub', 'best');
		const promise = getOrFire(k, fire);
		await promise; // entry now resolved
		void getOrFire(k, fire, () => {}); // subscribe later
		await Promise.resolve();
		expect(fire).toHaveBeenCalledTimes(1);
	});
});

describe('clearForShow — abort in-flight', () => {
	it('aborts the signal passed to fire, propagating cancellation downstream', async () => {
		// fire receives an AbortSignal. clearForShow flips it. The
		// playStream layer (real fire) closes its EventSource on the
		// signal so abandoned prefetches stop streaming.
		let receivedSignal: AbortSignal | null = null;
		const fire = (
			_emit: (p: PlayProgress) => void,
			signal: AbortSignal
		): Promise<CreateSessionResponse> => {
			receivedSignal = signal;
			return new Promise<CreateSessionResponse>((_resolve, reject) => {
				signal.addEventListener('abort', () => reject(new Error('aborted')));
			});
		};

		const k = makeKey('show-x', 1, 'sub', 'best');
		const promise = getOrFire(k, fire);
		await Promise.resolve();
		expect(receivedSignal).not.toBeNull();
		expect(receivedSignal!.aborted).toBe(false);

		clearForShow('show-x');
		expect(receivedSignal!.aborted).toBe(true);
		await expect(promise).rejects.toThrow('aborted');
	});
});

describe('clearForShow', () => {
	it('drops every entry whose key starts with the given show id', async () => {
		const fire = vi.fn(plainFire(fakeResp('x')));
		await getOrFire(makeKey('show-1', 1, 'sub', 'best'), fire);
		await getOrFire(makeKey('show-1', 2, 'sub', 'best'), fire);
		await getOrFire(makeKey('show-2', 1, 'sub', 'best'), fire);

		clearForShow('show-1');

		await getOrFire(makeKey('show-1', 1, 'sub', 'best'), fire);
		await getOrFire(makeKey('show-2', 1, 'sub', 'best'), fire);

		expect(fire).toHaveBeenCalledTimes(4);
	});
});
