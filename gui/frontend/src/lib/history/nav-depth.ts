/**
 * SPA back-stack depth tracker. Pure computation extracted from
 * +layout.svelte so we can unit-test the rules without mounting the
 * full SvelteKit runtime.
 *
 * The layout calls `nextDepth(...)` from inside `afterNavigate`, then
 * stamps the result onto the new history entry's state via
 * `history.replaceState({ ...state, aniGuiDepth: depth }, '')`. On
 * popstate the previous entry's stamped depth is read back, so
 * forward+back through the SPA history works without us tracking
 * direction.
 *
 * Why this is its own module:
 *   - The original tracker lived inline in the layout effect, which
 *     made it functionally untested. The fix that hid the BackButton
 *     correctly went through several false starts before this one
 *     stuck — all of which could have been caught by a 10-line unit
 *     test if the rules had been a function.
 *   - Extracting the rules also makes the layout effect a thin
 *     adapter: it pulls type + stampedDepth out of the SvelteKit
 *     event, hands them to nextDepth, writes the result back to
 *     history.state. No conditional logic in the component file.
 */

/** Subset of SvelteKit's NavigationType we react to. Other variants
 *  (e.g. 'leave', 'replaceState') don't change the depth. */
export type NavType = 'enter' | 'popstate' | 'goto' | 'link' | 'form' | 'leave' | 'replaceState';

export interface NavStep {
	/** SvelteKit afterNavigate's `type` field. */
	type: NavType;
	/** Depth read from `window.history.state?.aniGuiDepth` (the value
	 *  stamped on the entry we're moving TO). Null when not stamped —
	 *  pre-stamp entries from earlier sessions, or types that don't
	 *  carry our state. */
	stampedDepth: number | null;
	/** Depth tracked in component state right before this navigation. */
	prevDepth: number;
}

/** Compute the new depth for a single navigation event. Pure. */
export function nextDepth(step: NavStep): number {
	switch (step.type) {
		case 'enter':
			// Fresh app load (or hard reload). Reset to root regardless of
			// any leftover history.state from a prior session — Tauri's
			// WebView will sometimes preserve state across launches and
			// without this reset the BackButton showed up immediately on
			// open.
			return 0;
		case 'popstate':
			// Read the new entry's stamped depth. If absent (the very
			// first home entry of a session never gets stamped), fall
			// back to decrementing — popstate in this app is always a
			// back-press because there's no forward affordance in the UI.
			return step.stampedDepth ?? Math.max(0, step.prevDepth - 1);
		case 'goto':
		case 'link':
		case 'form':
			// Forward navigation. Layout caller is responsible for
			// stamping the resulting depth onto the new entry's state via
			// history.replaceState.
			return step.prevDepth + 1;
		case 'leave':
		case 'replaceState':
			// Neither of these changes our position in the back stack.
			return step.prevDepth;
	}
}

/** Whether the BackButton should render at the given depth. */
export function shouldShowBackButton(depth: number): boolean {
	return depth > 0;
}
