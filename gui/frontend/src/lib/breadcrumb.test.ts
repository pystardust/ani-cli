import { describe, expect, it } from 'vitest';
import { defaultTrailFor } from './breadcrumb';
import { m } from '$lib/paraglide/messages';

describe('defaultTrailFor', () => {
	it('returns a single Home segment on the home route', () => {
		expect(defaultTrailFor('/')).toEqual([{ label: m.breadcrumb_home() }]);
	});

	it('returns a single Home segment when the route id is null', () => {
		// Hard-loaded paths before SvelteKit has resolved the route id —
		// a one-segment trail keeps the breadcrumb visible without
		// claiming a child page until we know which one it is.
		expect(defaultTrailFor(null)).toEqual([{ label: m.breadcrumb_home() }]);
	});

	it('puts each top-level route under Home with a clickable parent', () => {
		// Each known route maps to a labelled child segment. The Home
		// parent must be href-set so clicking it walks back up the
		// tree. Labels resolve through Paraglide so a locale switch
		// re-renders them.
		const cases: Array<[string, string]> = [
			['/search', m.breadcrumb_search()],
			['/search?q=foo', m.breadcrumb_search()],
			['/settings', m.breadcrumb_settings()],
			['/diagnostics', m.breadcrumb_diagnostics()],
			['/anime/[id]', m.breadcrumb_anime()],
			['/play/[id]', m.breadcrumb_watching()]
		];
		for (const [route, label] of cases) {
			const trail = defaultTrailFor(route);
			expect(trail[0]).toEqual({ label: m.breadcrumb_home(), href: '/' });
			expect(trail[1]).toEqual({ label });
		}
	});

	it('falls back to a bare Home parent for unknown routes', () => {
		// Defensive — a route added after the breadcrumb still renders
		// something rather than collapsing to "[empty]".
		expect(defaultTrailFor('/some-future-route')).toEqual([
			{ label: m.breadcrumb_home(), href: '/' }
		]);
	});
});
