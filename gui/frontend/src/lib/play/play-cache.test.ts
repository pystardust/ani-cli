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
