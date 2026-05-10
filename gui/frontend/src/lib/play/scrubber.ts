/**
 * Pure pointer-x-to-track-fraction math for the custom player
 * scrubber. The component layer wires it to both `onclick` and
 * `onpointerdown` + `onpointermove` so click-to-seek and drag-to-
 * seek can never disagree about where a given clientX lands.
 *
 * Stubbed; real impl lands with the green commit.
 */

/**
 * Map a pointer's `clientX` to a fraction along a horizontal
 * track, clamped to [0, 1].
 */
export function clientXToFraction(clientX: number, rect: { left: number; width: number }): number {
	void clientX;
	void rect;
	// Stub returns NaN so all seven behavioural tests fail in the
	// red commit. The green commit replaces this body with the
	// clamped (clientX - left) / width math.
	return Number.NaN;
}
