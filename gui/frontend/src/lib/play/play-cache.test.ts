import { afterEach, describe, expect, it, vi } from 'vitest';
import type { CreateSessionResponse } from '$lib/api';
import { __resetPlayCacheForTests, clearForShow, getOrFire, makeKey } from './play-cache';

function fakeResp(seed: string): CreateSessionResponse {
	return {
		session_id: seed,
		media_url: `http://x/s/${seed}/master.m3u8`,
		media_kind: 'hls',
		subtitle_url: null
	};
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
		const fire = vi.fn(async () => fakeResp('a'));
		const k = makeKey('show-1', 1, 'sub', 'best');

		const p1 = getOrFire(k, fire);
		const p2 = getOrFire(k, fire);

		expect(p1).toBe(p2);
		await p1;
		expect(fire).toHaveBeenCalledTimes(1);
	});

	it('caches a successful resolution so a later call returns instantly', async () => {
		const fire = vi.fn(async () => fakeResp('a'));
		const k = makeKey('show-1', 1, 'sub', 'best');

		const first = await getOrFire(k, fire);
		const second = await getOrFire(k, fire);

		expect(second).toEqual(first);
		// Caller-provided fire fn should only run on the first hit.
		expect(fire).toHaveBeenCalledTimes(1);
	});

	it('drops failed entries so a retry can fire fresh', async () => {
		const fire = vi
			.fn<() => Promise<CreateSessionResponse>>()
			.mockRejectedValueOnce(new Error('boom'))
			.mockResolvedValueOnce(fakeResp('ok'));
		const k = makeKey('show-1', 1, 'sub', 'best');

		await expect(getOrFire(k, fire)).rejects.toThrow('boom');
		const recovered = await getOrFire(k, fire);

		expect(recovered.session_id).toBe('ok');
		expect(fire).toHaveBeenCalledTimes(2);
	});

	it('keeps separate entries for different keys', async () => {
		const fireA = vi.fn(async () => fakeResp('a'));
		const fireB = vi.fn(async () => fakeResp('b'));

		const a = await getOrFire(makeKey('show-1', 1, 'sub', 'best'), fireA);
		const b = await getOrFire(makeKey('show-1', 2, 'sub', 'best'), fireB);

		expect(a.session_id).toBe('a');
		expect(b.session_id).toBe('b');
		expect(fireA).toHaveBeenCalledTimes(1);
		expect(fireB).toHaveBeenCalledTimes(1);
	});
});

describe('clearForShow', () => {
	it('drops every entry whose key starts with the given show id', async () => {
		const fire = vi.fn(async () => fakeResp('x'));
		await getOrFire(makeKey('show-1', 1, 'sub', 'best'), fire);
		await getOrFire(makeKey('show-1', 2, 'sub', 'best'), fire);
		await getOrFire(makeKey('show-2', 1, 'sub', 'best'), fire);

		clearForShow('show-1');

		// show-1 entries gone — re-firing runs the fn again.
		await getOrFire(makeKey('show-1', 1, 'sub', 'best'), fire);
		// show-2 entry survived — re-firing is a cache hit.
		await getOrFire(makeKey('show-2', 1, 'sub', 'best'), fire);

		// Original 3 + one re-fire for show-1/ep1, but show-2 stayed cached.
		expect(fire).toHaveBeenCalledTimes(4);
	});
});
