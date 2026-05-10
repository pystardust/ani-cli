/**
 * Pick the default From/To values when the user enters the Download
 * confirm modal's Range mode. Range exists for multi-episode picks;
 * single-episode is what "This" already covers, so defaulting Range
 * to clicked-ep..clicked-ep (e.g. 13..13 on the last episode of a
 * 13-episode show) is a dead-end UX — the user has to retype both
 * fields to get to anything useful.
 *
 * Behaviour:
 *   - maxEpisode known → start=1, end=maxEpisode (full season; user
 *     can narrow either bound from there)
 *   - maxEpisode unknown → start=1, end=rangeFallbackCap (so a stray
 *     200 in the To input can't kick off a runaway loop; matches
 *     DownloadConfirm's existing rangeMax fallback)
 *   - maxEpisode === 1 → start=1, end=1 (single-episode show; Range
 *     and This collapse to the same arg, but Range still renders
 *     consistently)
 */
export function defaultRangeOnEnter(
	maxEpisode: number | null,
	rangeFallbackCap: number
): { start: number; end: number } {
	void maxEpisode;
	void rangeFallbackCap;
	throw new Error('test(red): defaultRangeOnEnter() lands in the paired feat(green) commit');
}
