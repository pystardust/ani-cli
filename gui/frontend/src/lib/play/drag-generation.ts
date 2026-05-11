/**
 * Drag-generation guard for the scrubber's release-time callbacks.
 *
 * Each `pointerdown` on the scrubber bumps a monotonic counter; the
 * `seeked` listener and 500 ms safety timer scheduled at `pointerup`
 * capture the counter's value at the time they were scheduled. When
 * they fire, they call this helper with the captured value and the
 * live counter — a non-match means a fresh drag has started in the
 * meantime, so the stale clear must no-op rather than clobber the
 * new drag's `dragPreviewFraction`.
 *
 * Pure / time-free / no closure capture — the caller passes the two
 * numbers in. Trivial enough that the strict-equality check is the
 * whole implementation; the helper exists to give the contract a
 * named test surface so a future refactor doesn't quietly switch to
 * an asymmetric `scheduled > current` comparison that wouldn't
 * defend the same way.
 */
export function isStaleDragCallback(scheduledGen: number, currentGen: number): boolean {
	return scheduledGen !== currentGen;
}
