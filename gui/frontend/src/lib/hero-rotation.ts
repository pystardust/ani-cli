/**
 * Pure helpers for the home hero's auto-rotation timer. The
 * component owns the `setInterval` itself + the reactive index; this
 * module owns the rules.
 *
 * Why extract: the same `if (rotationLength <= 1) return; if (paused)
 * return; if (reducedMotion) return;` chain was hard to verify at a
 * glance, and adding a fourth condition (e.g. "pause when the tab
 * isn't visible") would be invisible to tests inside the component.
 */

/** Index of the next rotation slot. Wraps modulo `total`. Returns 0
 *  for an empty rotation so the caller can hand it back to a state
 *  setter without branching. */
export function nextHeroIndex(current: number, total: number): number {
	if (total <= 0) return 0;
	return (current + 1) % total;
}

/** Whether the rotation interval should be running given current
 *  conditions. Three guards, all of equal weight: a single-item
 *  rotation has nothing to rotate to; a paused rotation is paused
 *  on purpose; reduced-motion users opted out of every kind of
 *  ambient motion. */
export function shouldRunHeroRotation(opts: {
	rotationLength: number;
	paused: boolean;
	prefersReducedMotion: boolean;
}): boolean {
	if (opts.rotationLength <= 1) return false;
	if (opts.paused) return false;
	if (opts.prefersReducedMotion) return false;
	return true;
}
