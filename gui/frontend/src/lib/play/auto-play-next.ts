/**
 * Decide whether to auto-advance to the next episode after the current
 * one finishes playing (the `<video>` `ended` event). The /play page
 * wires this to the user's `auto_play_next` setting + the current
 * episode position; the function itself is pure so the policy is
 * unit-testable without a DOM.
 *
 * The chain stops naturally at the last known episode. When
 * `totalEpisodes` is null (Kitsu lacks an episode count for some
 * sequels/specials), advance anyway and let the upstream
 * cmd_create_session failure surface a play-failure overlay — same
 * "best effort" stance as the existing prev/next buttons' hasNext
 * derivation.
 */
export interface AutoPlayInput {
	enabled: boolean;
	episodeNum: number;
	totalEpisodes: number | null;
}

export type AutoPlayDecision = { advance: true; target: number } | { advance: false };

/**
 * STUB (red commit). Implementation lands in the green commit; tests
 * in this module's `.test.ts` file assert the contract.
 */
export function decideAutoPlayNext(input: AutoPlayInput): AutoPlayDecision {
	void input;
	return { advance: false };
}
