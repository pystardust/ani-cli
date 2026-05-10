/**
 * Pure pointer-x-to-track-fraction math for the custom player
 * scrubber. The component layer wires it to both `onclick` and
 * `onpointerdown` + `onpointermove` so click-to-seek and drag-to-
 * seek can never disagree about where a given clientX lands.
 */

/**
 * Map a pointer's `clientX` to a fraction along a horizontal
 * track, clamped to [0, 1]. Returns 0 for zero-width rects (a
 * brief layout-transition flash) so the seek is a no-op until the
 * rect settles instead of dividing by zero.
 */
export function clientXToFraction(clientX: number, rect: { left: number; width: number }): number {
	if (rect.width <= 0) return 0;
	const raw = (clientX - rect.left) / rect.width;
	if (raw <= 0) return 0;
	if (raw >= 1) return 1;
	return raw;
}

/**
 * Decide the [0, 1] fraction the scrubber's thumb + fill should
 * render at right now.
 *
 * `dragPreviewFraction` (updated synchronously on every
 * pointermove during a drag) wins so the visuals follow the
 * pointer 1:1; otherwise we use `currentTime / duration` so the
 * thumb tracks normal playback. Always clamps to [0, 1] so
 * out-of-range inputs (a duration zero before loadedmetadata, a
 * currentTime briefly past duration on the seeking edge) can't
 * render a glitchy >100% fill or negative thumb.
 */
export function displayedScrubFraction(
	dragPreviewFraction: number | null,
	currentTime: number,
	duration: number
): number {
	const raw =
		dragPreviewFraction ?? (Number.isFinite(duration) && duration > 0 ? currentTime / duration : 0);
	if (!Number.isFinite(raw) || raw <= 0) return 0;
	if (raw >= 1) return 1;
	return raw;
}
