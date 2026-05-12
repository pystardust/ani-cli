// @vitest-environment happy-dom
//
// Paraglide messages compile to per-key JS modules under
// `src/lib/paraglide/` ahead of the run via `pnpm i18n:compile`
// (chained into vitest config). happy-dom isn't strictly needed —
// the helper is pure — but mirrors the rest of `lib/play/*.test.ts`.
import { describe, expect, it } from 'vitest';
import { externalLaunchSuccessToast, playerKindLabel } from './external-toast';

describe('playerKindLabel', () => {
	// Brand names stay literal — translating "mpv" or "VLC" would be
	// wrong, they're proper nouns. Only the `custom` fallback gets a
	// localized phrase ("external player") because there's no
	// product name to surface.
	it.each([
		['mpv', 'mpv'],
		['vlc', 'VLC'],
		['iina', 'IINA']
	] as const)('returns the literal brand label for %s', (kind, expected) => {
		expect(playerKindLabel(kind)).toBe(expected);
	});

	it('returns the localized generic label for custom', () => {
		// EN copy is "external player". A translation change here
		// indicates the locale wants a different fallback — fine, but
		// flag it as a deliberate spec update.
		expect(playerKindLabel('custom')).toBe('external player');
	});
});

describe('externalLaunchSuccessToast', () => {
	it('returns a success-kind toast naming the episode + player', () => {
		const toast = externalLaunchSuccessToast({ episode: 5, kind: 'mpv' });
		expect(toast.kind).toBe('success');
		// 4s is the same duration as the previous inline banner; the
		// migration shouldn't change how long the message sits on
		// screen.
		expect(toast.duration).toBe(4000);
		expect(toast.message).toContain('5');
		expect(toast.message).toContain('mpv');
	});

	it('uses the VLC brand label for vlc kind', () => {
		const toast = externalLaunchSuccessToast({ episode: 12, kind: 'vlc' });
		expect(toast.message).toContain('VLC');
		expect(toast.message).toContain('12');
	});

	it('uses the IINA brand label for iina kind', () => {
		const toast = externalLaunchSuccessToast({ episode: 1, kind: 'iina' });
		expect(toast.message).toContain('IINA');
	});

	it('falls back to the generic label for custom kind', () => {
		const toast = externalLaunchSuccessToast({ episode: 7, kind: 'custom' });
		expect(toast.message).toContain('external player');
	});
});
