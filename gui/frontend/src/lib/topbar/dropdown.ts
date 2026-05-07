/**
 * Pure helpers for the layout topbar's live-results dropdown +
 * recent-searches list. Extracted from `+layout.svelte` so the rules
 * are testable without mounting a Svelte component against
 * SvelteKit's runtime.
 *
 * What stays in the component: the reactive state ($state vars), the
 * setTimeout-based debounce + blur-dismiss timers, and the keyboard
 * event wiring. What lives here: the per-event computations the
 * component glues together.
 */

/** localStorage key for the last-N user submissions. Public so the
 *  component imports it instead of redeclaring the literal. */
export const RECENT_STORAGE_KEY = 'ani-gui:recent-searches';

/** Default cap for how many recent submissions we surface. */
export const RECENT_LIMIT = 5;

/** Wrap-around index cycling for ↑/↓ navigation through a result
 *  list. Zero-length lists return `-1` (no selection).
 *
 *  Semantics for the "no current selection" case (`current = -1`):
 *    - First ↓ lands on the first item (idx 0).
 *    - First ↑ lands on the last item (idx `total - 1`).
 *  This matches the dropdown convention users expect — the keyboard
 *  navigation feels like cycling around the edge.
 *
 *  For an existing selection, modular arithmetic with the sentinel
 *  fixed up: shift `-1` to `total` for the "↓ first" case and to
 *  `-1` for the natural backward step. */
export function cycleSelectedIdx(current: number, direction: 1 | -1, total: number): number {
	if (total <= 0) return -1;
	// Treat current=-1 (no selection) as "just past the boundary"
	// in whichever direction the user pressed. With direction=+1 we
	// land on idx 0 (modular `(-1 + 1 + total) % total = 0`). With
	// direction=-1 we want idx total-1; adjust the starting position
	// so the modulo lands there.
	const start = current < 0 ? (direction === 1 ? -1 : total) : current;
	return (start + direction + total) % total;
}

/** Prepend `query` to `existing`, deduplicate (keeping the freshest
 *  occurrence first), cap at `max`. Pure — caller is responsible for
 *  persisting the result.
 *
 *  Deduplication uses a Set rather than a single `!==` filter against
 *  `query`, so duplicates already present in `existing` (a corrupted
 *  localStorage blob, or a freshly-written list that hadn't been
 *  cleaned) are also collapsed. The earliest occurrence wins. */
export function mergeRecents(
	existing: readonly string[],
	query: string,
	max: number = RECENT_LIMIT
): string[] {
	const seen = new Set<string>([query]);
	const out: string[] = [query];
	for (const x of existing) {
		if (seen.has(x)) continue;
		seen.add(x);
		out.push(x);
	}
	return out.slice(0, max);
}

/** Defensive parse of a localStorage payload that should be a JSON
 *  array of strings. Anything else (null, throwing JSON, wrong
 *  shape, mixed types) collapses to `[]`. */
export function parseStoredRecents(raw: string | null, max: number = RECENT_LIMIT): string[] {
	if (raw === null) return [];
	let parsed: unknown;
	try {
		parsed = JSON.parse(raw);
	} catch {
		return [];
	}
	if (!Array.isArray(parsed)) return [];
	return parsed.filter((x): x is string => typeof x === 'string').slice(0, max);
}

/** Outcome the component's submit handler should run. Mapped 1:1 to
 *  caller-side actions: `navigate-to-hit` → goto the highlighted
 *  result; `submit-query` → goto /search?q=…; `noop` → do nothing
 *  (empty input, nothing selected). */
export type EnterAction =
	| { type: 'navigate-to-hit'; idx: number }
	| { type: 'submit-query' }
	| { type: 'noop' };

/** Decide what Enter does given the current dropdown state. */
export function decideEnterAction(
	selectedIdx: number,
	resultsLength: number,
	query: string
): EnterAction {
	if (selectedIdx >= 0 && selectedIdx < resultsLength) {
		return { type: 'navigate-to-hit', idx: selectedIdx };
	}
	if (query.trim().length > 0) {
		return { type: 'submit-query' };
	}
	return { type: 'noop' };
}

/** State the visibility decision needs from the calling component. */
export interface DropdownVisibilityState {
	/** Whether the user has the input focused / dropdown should be open. */
	dropdownOpen: boolean;
	/** Live results from the in-flight Kitsu search; `null` until first
	 *  reply, `[]` when the query produced no matches. */
	liveResults: readonly unknown[] | null;
	/** Truthy when the most recent live-search request errored. */
	liveError: unknown;
	/** The trimmed query — caller is responsible for `.trim()`. */
	queryTrimmed: string;
	/** Number of remembered recent searches in localStorage. */
	recentsCount: number;
}

/** Whether the dropdown card has any content to show. The component
 *  uses this to gate rendering of the wrapper div, so an empty card
 *  never appears for an empty query with no recent searches. The
 *  branches mirror the order the markup uses for its inner content
 *  — keep them in sync if either changes. */
export function shouldRenderDropdown(
	state: DropdownVisibilityState,
	opts: { liveMinChars: number }
): boolean {
	if (!state.dropdownOpen) return false;
	if (state.liveResults && state.liveResults.length > 0) return true;
	if (state.liveError) return true;
	if (state.queryTrimmed.length >= opts.liveMinChars && state.liveResults?.length === 0) {
		return true;
	}
	if (state.queryTrimmed.length === 0 && state.recentsCount > 0) return true;
	return false;
}
