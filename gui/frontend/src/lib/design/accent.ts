/**
 * Per-anime accent picker. djb2 hash of the Kitsu id, mod the palette length,
 * gives every title a stable color that doesn't change between sessions.
 *
 * TODO(design): replace with AniList coverImage.color once the AniList client
 * lands. The 8-color palette below is the offline fallback only.
 */

export const ACCENT_PALETTE = [
	'var(--accent-oxblood)',
	'var(--accent-ink)',
	'var(--accent-persimmon)',
	'var(--accent-jade)',
	'var(--accent-dusk)',
	'var(--accent-ochre)',
	'var(--accent-plum)',
	'var(--accent-slate)'
] as const;

function djb2(s: string): number {
	let h = 5381;
	for (let i = 0; i < s.length; i++) {
		h = ((h << 5) + h + s.charCodeAt(i)) | 0;
	}
	return h >>> 0;
}

export function accentFor(id: string): string {
	if (!id) return ACCENT_PALETTE[1];
	return ACCENT_PALETTE[djb2(id) % ACCENT_PALETTE.length];
}
