/**
 * Reactive store for download failures that warrant a blocking modal
 * (e.g. ffmpeg missing on Windows). The dock's per-row error state
 * stays for transient failures the user can retry; this store is
 * for failures that need a clear "do this to fix it" surface, like
 * the play-page's ErrorOverlay pattern.
 *
 * Held separately from `downloadStore` so the dock stays focused on
 * per-row state and the layout has a single place to subscribe for
 * the modal. `show()` overrides any existing payload — the latest
 * failure wins, since two failures back-to-back on the same dock
 * are almost always the same root cause.
 */

/** Discriminated payload identifying the failure kind. Future kinds
 *  (e.g. `aria2c_missing`, `disk_full`) extend this union. */
export type DownloadFailurePayload = { kind: 'ffmpeg_missing' };

class DownloadFailureStore {
	current = $state<DownloadFailurePayload | null>(null);

	/** Open the modal with the given payload. Replaces any existing
	 *  modal — the most-recent failure is the one the user sees. */
	show(payload: DownloadFailurePayload): void {
		this.current = payload;
	}

	/** Close the modal. Idempotent — safe to call when nothing is open. */
	dismiss(): void {
		this.current = null;
	}
}

export const downloadFailureStore = new DownloadFailureStore();
