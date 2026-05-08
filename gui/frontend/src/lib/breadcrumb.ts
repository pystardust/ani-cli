/**
 * Breadcrumb path store — drives the Home › Anime › Episode trail
 * shown in the topbar. Replaces the bare BackButton: each segment is
 * a clickable hop to that level of the hierarchy, and the current
 * page renders as plain text.
 *
 * Default contributors:
 *  - The layout sets a generic two-segment trail on every navigation
 *    based on URL alone (so a hard-load lands with something visible).
 *  - Routes that have richer labels (anime title, episode number)
 *    overwrite the trail in onMount once their data loads.
 *
 * On navigation away the layout resets to defaults; this avoids a
 * stale "Naruto Shippuuden" segment lingering on /search after the
 * user navigated away.
 */

import { writable } from 'svelte/store';

export interface BreadcrumbSegment {
	label: string;
	/** When omitted, the segment renders as the current page (plain
	 *  text, non-clickable). The last segment in a trail must always
	 *  be `href`-less. */
	href?: string;
}

export const breadcrumb = writable<BreadcrumbSegment[]>([]);

/** Build the default trail from a route id alone. Used by the layout
 *  on every navigation so a page with no breadcrumb hook still shows
 *  something meaningful. Routes override this on mount with richer
 *  labels (anime title, episode number). */
export function defaultTrailFor(routeId: string | null): BreadcrumbSegment[] {
	if (!routeId || routeId === '/') {
		return [{ label: 'Home' }];
	}
	const home: BreadcrumbSegment = { label: 'Home', href: '/' };
	if (routeId.startsWith('/search')) return [home, { label: 'Search' }];
	if (routeId.startsWith('/settings')) return [home, { label: 'Settings' }];
	if (routeId.startsWith('/diagnostics')) return [home, { label: 'Diagnostics' }];
	if (routeId.startsWith('/anime')) return [home, { label: 'Anime' }];
	if (routeId.startsWith('/play')) return [home, { label: 'Watching' }];
	return [home];
}
