/**
 * Hermetic Kitsu / history fixtures used by Playwright tests.
 *
 * The real Rust backend at runtime hits Kitsu over the network and
 * returns shapes our frontend understands. For tests we don't want
 * that — we stub the renderer's `fetch()` calls via `page.route()`
 * so the assertions run against a fixed-shape response and don't
 * depend on Kitsu's availability or schema changes.
 *
 * Each fixture matches the shape of `KitsuAnimeRef` / `HistoryEntry`
 * in `gui/frontend/src/lib/api.ts` — keep them in sync if those
 * types ever change.
 */

export const trending = [
	{
		id: '1',
		canonical_title: 'Cowboy Bebop',
		english_title: 'Cowboy Bebop',
		synopsis: 'Spike Spiegel and his crew chase bounties across the solar system.',
		episode_count: 26,
		average_rating: '85.34',
		start_date: '1998-04-03',
		status: 'finished',
		poster_image: {
			tiny: 'https://media.kitsu.app/anime/poster_images/1/tiny.jpg',
			small: 'https://media.kitsu.app/anime/poster_images/1/small.jpg',
			medium: 'https://media.kitsu.app/anime/poster_images/1/medium.jpg',
			large: 'https://media.kitsu.app/anime/poster_images/1/large.jpg',
			original: 'https://media.kitsu.app/anime/poster_images/1/original.jpg'
		},
		cover_image: {
			tiny: null,
			small: null,
			large: 'https://media.kitsu.app/anime/cover_images/1/large.jpg',
			original: 'https://media.kitsu.app/anime/cover_images/1/original.jpg'
		}
	}
];

export const topRated = [
	{
		id: '11',
		canonical_title: 'Fullmetal Alchemist: Brotherhood',
		english_title: 'Fullmetal Alchemist: Brotherhood',
		synopsis: 'Two brothers search for the Philosopher’s Stone.',
		episode_count: 64,
		average_rating: '90.86',
		start_date: '2009-04-05',
		status: 'finished',
		poster_image: {
			tiny: 'https://media.kitsu.app/anime/poster_images/11/tiny.jpg',
			small: 'https://media.kitsu.app/anime/poster_images/11/small.jpg',
			medium: 'https://media.kitsu.app/anime/poster_images/11/medium.jpg',
			large: 'https://media.kitsu.app/anime/poster_images/11/large.jpg',
			original: 'https://media.kitsu.app/anime/poster_images/11/original.jpg'
		},
		cover_image: null
	}
];

/** Empty history — the home page should render the "no history yet" empty state. */
export const emptyHistory: unknown[] = [];

/** App info fixture — minimal shape `getAppInfo` consumers need. */
export const appInfo = {
	version: '0.1.0-test',
	ani_cli_path: '/usr/bin/ani-cli',
	history_path: '/tmp/ani-hsts',
	proxy_base_url: 'http://127.0.0.1:0'
};
