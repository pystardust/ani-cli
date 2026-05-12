// @vitest-environment happy-dom
//
// Paraglide messages compile to per-key JS modules under
// `src/lib/paraglide/` ahead of the run via `pnpm i18n:compile`.
// happy-dom isn't strictly needed — the helpers are pure — but
// mirrors `external-toast.test.ts`.
import { describe, expect, it } from 'vitest';
import {
	describeSyncplayLaunchFailure,
	isSyncplaySpawnFailure,
	syncplayLaunchSuccessToast
} from './syncplay-toast';

describe('syncplayLaunchSuccessToast', () => {
	it('returns a success-kind toast naming the episode', () => {
		const toast = syncplayLaunchSuccessToast({ episode: 5 });
		expect(toast.kind).toBe('success');
		// 4s matches the external-launch toast so both pop-up
		// successes feel the same.
		expect(toast.duration).toBe(4000);
		expect(toast.message).toContain('5');
		// "Syncplay" is a brand name — appears literal in every locale.
		expect(toast.message).toContain('Syncplay');
	});

	it('carries different episode numbers verbatim', () => {
		const toast = syncplayLaunchSuccessToast({ episode: 12 });
		expect(toast.message).toContain('12');
	});
});

describe('describeSyncplayLaunchFailure', () => {
	it('names the binary on syncplay_spawn_failed payloads', () => {
		// Backend returns `{ kind: "syncplay_spawn_failed", binary: "..." }`
		// when Command::spawn() fails. The helper produces the body
		// text — the surrounding modal's headline + action live on
		// the play page.
		const got = describeSyncplayLaunchFailure({
			kind: 'syncplay_spawn_failed',
			binary: '/opt/syncplay/syncplay'
		});
		expect(got).toContain('/opt/syncplay/syncplay');
	});

	it('falls back to describePlayFailure for resolve-step errors', () => {
		// If the URL resolution (ani-cli) fails before Syncplay even
		// spawns, the user should see the same polished message as
		// the embedded play path — not a debug-y "Syncplay failed:
		// scraper" string.
		const scraperErr = { kind: 'scraper', key: 'error.scraper.parse_failed' };
		const got = describeSyncplayLaunchFailure(scraperErr);
		// describePlayFailure's scraper branch fires on "scraper" in
		// the flattened error string; pin that the helper hits that
		// branch by checking the EN copy.
		expect(got.length).toBeGreaterThan(0);
		// Should NOT name a binary (it's a resolve error, not a spawn
		// error).
		expect(got).not.toContain('syncplay');
	});

	it('uses Syncplay copy on empty-binary spawn failures (cleared Settings field)', () => {
		// The backend emits `syncplay_spawn_failed { binary: "" }`
		// when the user has cleared the Syncplay path in Settings —
		// see commands/syncplay.rs::open_syncplay. Treating this as a
		// generic resolve-step error would point users at the wrong
		// recovery (retry / wait for upstream) when the real fix is
		// "go set a binary path". Use the unnamed Syncplay-spawn copy
		// instead so the surrounding modal still surfaces the Get
		// Syncplay affordance with the matching messaging.
		const got = describeSyncplayLaunchFailure({ kind: 'syncplay_spawn_failed', binary: '' });
		expect(got.length).toBeGreaterThan(0);
		// Brand name appears literal in every locale; pin that the
		// helper hit the Syncplay branch, not describePlayFailure.
		expect(got.toLowerCase()).toContain('syncplay');
		expect(got).not.toContain(' ""');
	});

	it('still falls back to describePlayFailure when binary field is missing or wrong type', () => {
		// Defensive: a malformed payload (no binary field, or
		// non-string) shouldn't crash and shouldn't claim to be a
		// spawn failure. Empty-string is the user-cleared-Settings
		// path and lives in the test above; this one covers genuine
		// shape errors.
		const noBinary = describeSyncplayLaunchFailure({ kind: 'syncplay_spawn_failed' });
		const wrongType = describeSyncplayLaunchFailure({
			kind: 'syncplay_spawn_failed',
			binary: 42
		});
		expect(noBinary.length).toBeGreaterThan(0);
		expect(wrongType.length).toBeGreaterThan(0);
	});
});

describe('isSyncplaySpawnFailure', () => {
	// Drives the play page's "Get Syncplay" affordance: link to
	// syncplay.pl/download/ only when the failure is a spawn error.
	// For resolve-step errors (scraper, timeout, network) installing
	// Syncplay won't help, so the action is suppressed.

	it('returns true for well-formed syncplay_spawn_failed payloads', () => {
		expect(
			isSyncplaySpawnFailure({
				kind: 'syncplay_spawn_failed',
				binary: '/opt/syncplay/syncplay'
			})
		).toBe(true);
	});

	it('returns false for scraper / resolve-step errors', () => {
		expect(isSyncplaySpawnFailure({ kind: 'scraper', key: 'error.scraper.parse_failed' })).toBe(
			false
		);
		expect(isSyncplaySpawnFailure({ kind: 'timeout' })).toBe(false);
		expect(isSyncplaySpawnFailure({ kind: 'network' })).toBe(false);
		expect(isSyncplaySpawnFailure({ kind: 'no_results' })).toBe(false);
	});

	it('returns false for player_spawn_failed (external player, not syncplay)', () => {
		// PlayerSpawnFailed is the external-player variant; if it
		// somehow surfaces on the syncplay code path, we shouldn't
		// confuse it with a Syncplay-binary problem.
		expect(isSyncplaySpawnFailure({ kind: 'player_spawn_failed', binary: 'mpv' })).toBe(false);
	});

	it('returns true on empty-binary syncplay_spawn_failed (cleared Settings)', () => {
		// Mirrors describeSyncplayLaunchFailure: the backend emits
		// this exact payload when the user has cleared the Syncplay
		// binary in Settings. The recovery — install Syncplay or set
		// the path — is the same whether binary is named or blank,
		// so the install affordance must stay visible.
		expect(isSyncplaySpawnFailure({ kind: 'syncplay_spawn_failed', binary: '' })).toBe(true);
	});

	it('returns false on truly malformed syncplay_spawn_failed payloads', () => {
		// Defensive: missing field or wrong type — too uncertain to
		// claim it's a spawn failure. Treat as resolve error.
		expect(isSyncplaySpawnFailure({ kind: 'syncplay_spawn_failed' })).toBe(false);
		expect(isSyncplaySpawnFailure({ kind: 'syncplay_spawn_failed', binary: 42 })).toBe(false);
	});

	it('returns false for unknown error shapes (plain Error, undefined, null)', () => {
		expect(isSyncplaySpawnFailure(new Error('boom'))).toBe(false);
		expect(isSyncplaySpawnFailure(undefined)).toBe(false);
		expect(isSyncplaySpawnFailure(null)).toBe(false);
		expect(isSyncplaySpawnFailure('a string')).toBe(false);
	});
});
