import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { KitsuAnimeRef } from '$lib/api';

const apiMock = vi.hoisted(() => ({
	availabilityBatch: vi.fn(),
	availabilityWarm: vi.fn(),
	checkAvailability: vi.fn(),
	altTitlesFromKitsu: vi.fn((ref: { id: string } | null | undefined) =>
		ref ? [`alt-${ref.id}`] : []
	)
}));
vi.mock('$lib/api', () => apiMock);

import { filterAvailable, filterAvailableStrict } from './filter';

function ref(id: string, overrides: Partial<KitsuAnimeRef> = {}): KitsuAnimeRef {
	return {
		id,
		canonical_title: `Title ${id}`,
		slug: null,
		synopsis: null,
		start_date: null,
		end_date: null,
		episode_count: null,
		average_rating: null,
		subtype: null,
		status: null,
		age_rating: null,
		popularity_rank: null,
		poster_image: null,
		cover_image: null,
		...overrides
	};
}

describe('filterAvailable (lazy / fire-and-forget warm)', () => {
	beforeEach(() => {
		apiMock.availabilityBatch.mockReset();
		apiMock.availabilityWarm.mockReset();
		apiMock.checkAvailability.mockReset();
	});
	afterEach(() => vi.useRealTimers());

	it('returns empty list unchanged without hitting the API', async () => {
		const out = await filterAvailable([], 'sub');
		expect(out).toEqual([]);
		expect(apiMock.availabilityBatch).not.toHaveBeenCalled();
	});

	it('drops cards the cache marks unavailable, keeps cached-true and uncached', async () => {
		const items = [ref('a'), ref('b'), ref('c')];
		apiMock.availabilityBatch.mockResolvedValueOnce({
			cached: { a: true, b: false /* c uncached */ }
		});
		apiMock.availabilityWarm.mockResolvedValueOnce(undefined);
		const out = await filterAvailable(items, 'sub');
		// b drops; a (true) and c (uncached, unknown) survive — the
		// home strip's "render now, prune later" UX requirement.
		expect(out.map((r) => r.id)).toEqual(['a', 'c']);
	});

	it('warms only the uncached items and forwards mode + alt titles', async () => {
		const items = [ref('a', { episode_count: 12, status: 'finished' }), ref('b')];
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: { a: true } });
		apiMock.availabilityWarm.mockResolvedValueOnce(undefined);
		await filterAvailable(items, 'dub');
		expect(apiMock.availabilityWarm).toHaveBeenCalledTimes(1);
		const warmArg = apiMock.availabilityWarm.mock.calls[0][0];
		expect(warmArg).toHaveLength(1);
		expect(warmArg[0]).toMatchObject({
			title: 'Title b',
			mode: 'dub',
			alt_titles: ['alt-b'],
			kitsu_id: 'b'
		});
	});

	it('skips the warm call when nothing is uncached', async () => {
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: { a: true, b: false } });
		await filterAvailable([ref('a'), ref('b')], 'sub');
		expect(apiMock.availabilityWarm).not.toHaveBeenCalled();
	});

	it('falls back to rendering all items when the batch call throws', async () => {
		// Network failure shouldn't blank the home page — the lazy
		// click path will surface real errors when the user actually
		// picks a show.
		apiMock.availabilityBatch.mockRejectedValueOnce(new Error('offline'));
		const items = [ref('a'), ref('b')];
		const out = await filterAvailable(items, 'sub');
		expect(out).toEqual(items);
		expect(apiMock.availabilityWarm).not.toHaveBeenCalled();
	});

	it('ignores warm-call rejections (fire-and-forget contract)', async () => {
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: {} });
		apiMock.availabilityWarm.mockRejectedValueOnce(new Error('warm failed'));
		// The function awaits the batch call, then kicks off warm
		// without await. The rejection must not propagate.
		await expect(filterAvailable([ref('a')], 'sub')).resolves.toBeDefined();
	});
});

describe('filterAvailableStrict (search / inline probe)', () => {
	beforeEach(() => {
		apiMock.availabilityBatch.mockReset();
		apiMock.availabilityWarm.mockReset();
		apiMock.checkAvailability.mockReset();
	});

	it('inline-probes uncached items and applies their results', async () => {
		// b is cached false → drops. a is cached true → kept. c is
		// uncached → probed inline → kept (probe says available).
		// d is uncached → probed inline → dropped (probe says not).
		apiMock.availabilityBatch.mockResolvedValueOnce({
			cached: { a: true, b: false }
		});
		apiMock.checkAvailability.mockImplementation(async (args) =>
			args.kitsu_id === 'c' ? { available: true } : { available: false }
		);
		const out = await filterAvailableStrict([ref('a'), ref('b'), ref('c'), ref('d')], 'sub', 2);
		expect(out.map((r) => r.id)).toEqual(['a', 'c']);
		// Two probes, one per uncached id.
		expect(apiMock.checkAvailability).toHaveBeenCalledTimes(2);
	});

	it('keeps an item when its inline probe throws (defer to lazy path)', async () => {
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: {} });
		apiMock.checkAvailability.mockRejectedValue(new Error('upstream 503'));
		const out = await filterAvailableStrict([ref('a')], 'sub');
		expect(out.map((r) => r.id)).toEqual(['a']);
	});

	it('returns items unchanged when the batch call itself throws', async () => {
		apiMock.availabilityBatch.mockRejectedValueOnce(new Error('offline'));
		const items = [ref('a'), ref('b')];
		const out = await filterAvailableStrict(items, 'sub');
		expect(out).toEqual(items);
	});

	it('returns empty list unchanged without hitting the API', async () => {
		const out = await filterAvailableStrict([], 'sub');
		expect(out).toEqual([]);
		expect(apiMock.availabilityBatch).not.toHaveBeenCalled();
	});
});
