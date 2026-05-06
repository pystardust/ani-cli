import { describe, expect, it } from 'vitest';
import { nextEpisodeAction } from './next-episode';

describe('nextEpisodeAction', () => {
	it('starts cold — label "Play episode 1", episode 1', () => {
		expect(nextEpisodeAction({ lastPlayedEp: null, episodeCount: 26 })).toEqual({
			label: 'Play episode 1',
			episode: 1
		});
	});

	it('after playing ep 5 of 26, suggests ep 6 with the "Play next" label', () => {
		expect(nextEpisodeAction({ lastPlayedEp: 5, episodeCount: 26 })).toEqual({
			label: 'Play next: ep 6',
			episode: 6
		});
	});

	it('cycles back to ep 1 after the final episode of a finite series', () => {
		// Rollover: lastPlayedEp == episodeCount → no next, fall back
		// to "Play episode 1". Simpler than a "completed" state.
		expect(nextEpisodeAction({ lastPlayedEp: 26, episodeCount: 26 })).toEqual({
			label: 'Play episode 1',
			episode: 1
		});
	});

	it('cycles back even if lastPlayedEp ran past the count (defensive)', () => {
		// Shouldn't happen in normal flow, but if a stale state ever
		// has lastPlayedEp > episodeCount, still fall back gracefully.
		expect(nextEpisodeAction({ lastPlayedEp: 99, episodeCount: 26 })).toEqual({
			label: 'Play episode 1',
			episode: 1
		});
	});

	it('with unknown episode count, suggests the next episode without rollover protection', () => {
		// Currently-airing shows often have null episode_count from
		// Kitsu. Without a known cap we can't detect rollover; just
		// keep advancing.
		expect(nextEpisodeAction({ lastPlayedEp: 5, episodeCount: null })).toEqual({
			label: 'Play next: ep 6',
			episode: 6
		});
	});

	it('treats lastPlayedEp <= 0 as "no plays yet" (defensive)', () => {
		// If someone passes 0 or a negative number through, we should
		// behave like the cold state, not "Play next: ep 1" (which
		// would be technically correct but reads weird).
		expect(nextEpisodeAction({ lastPlayedEp: 0, episodeCount: 26 })).toEqual({
			label: 'Play episode 1',
			episode: 1
		});
	});
});
