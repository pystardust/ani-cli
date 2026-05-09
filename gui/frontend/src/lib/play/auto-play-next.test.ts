import { describe, expect, it } from 'vitest';
import { decideAutoPlayNext } from './auto-play-next';

describe('decideAutoPlayNext', () => {
	it('does not advance when the toggle is disabled', () => {
		// Disabled is the default — explicitly verify it never advances,
		// even mid-series. Otherwise a config-load race could let the
		// `ended` handler fire before settings arrive and silently start
		// auto-advancing under the user.
		expect(decideAutoPlayNext({ enabled: false, episodeNum: 1, totalEpisodes: 12 })).toEqual({
			advance: false
		});
	});

	it('advances to ep+1 when enabled and not at the last episode', () => {
		expect(decideAutoPlayNext({ enabled: true, episodeNum: 5, totalEpisodes: 12 })).toEqual({
			advance: true,
			target: 6
		});
	});

	it('does not advance past the last known episode', () => {
		// End of series. Previously the explicit range-play feature
		// would just run out of range; the toggle relies on episode
		// count to stop the chain.
		expect(decideAutoPlayNext({ enabled: true, episodeNum: 12, totalEpisodes: 12 })).toEqual({
			advance: false
		});
	});

	it('does not advance if the current episode is past total (defensive)', () => {
		// Shouldn't happen, but a stale URL or off-by-one shouldn't
		// produce ever-incrementing requests.
		expect(decideAutoPlayNext({ enabled: true, episodeNum: 13, totalEpisodes: 12 })).toEqual({
			advance: false
		});
	});

	it('advances when totalEpisodes is unknown — let upstream stop the chain', () => {
		// Kitsu's episode_count is sometimes null for sequels/specials.
		// We don't gate the chain on it; the existing prev/next buttons
		// use the same "advance if total is unknown" policy
		// (hasNext = totalEpisodes === null || episodeNum < totalEpisodes).
		// When the upstream eventually 404s, the existing playFailure
		// overlay surfaces the error.
		expect(decideAutoPlayNext({ enabled: true, episodeNum: 99, totalEpisodes: null })).toEqual({
			advance: true,
			target: 100
		});
	});

	it('disabled wins over end-of-series — both produce no-advance', () => {
		// Sanity check: the disabled branch short-circuits, so the
		// total-episodes check isn't evaluated. Two paths to the same
		// outcome; assert both reach it.
		expect(decideAutoPlayNext({ enabled: false, episodeNum: 12, totalEpisodes: 12 })).toEqual({
			advance: false
		});
	});
});
