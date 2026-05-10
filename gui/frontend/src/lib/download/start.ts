/**
 * Glue between the download api wrapper and the shared download
 * store. Single entry point used by the confirm modal: `startDownload`
 * adds a row, opens the SSE, and feeds progress / final / error
 * events into the store. Returns the new id so callers can show a
 * targeted toast / focus the dock row.
 */

import { downloadStream, type DownloadArgs } from '$lib/api';
import { downloadStore } from './store.svelte';
import { downloadFailureStore } from './failure-store.svelte';

/** Typed-error payload shape coming off the SSE error event from
 *  the backend — see api/mod.rs's serde_json::to_value(&AniError)
 *  + injected `key`. The `kind` discriminator lets the catch
 *  handler route specific failures (today: ffmpeg_missing) to the
 *  blocking modal instead of the dock's per-row tooltip. */
function isFfmpegMissingPayload(e: unknown): boolean {
	return (
		typeof e === 'object' &&
		e !== null &&
		'kind' in e &&
		(e as { kind: unknown }).kind === 'ffmpeg_missing'
	);
}

export function startDownload(args: DownloadArgs & { destDir: string }): string {
	const id = downloadStore.add({
		title: args.title,
		episode: args.episode,
		mode: args.mode,
		quality: args.quality ?? 'best',
		destDir: args.destDir
	});

	const ctrl = new AbortController();
	downloadStore.markActive(id, ctrl);

	void downloadStream(args, (p) => downloadStore.setProgress(id, p.line), ctrl.signal)
		.then((resp) => downloadStore.markDone(id, resp.dest_dir))
		.catch((e: unknown) => {
			// ffmpeg_missing is a blocking install-required failure —
			// route it to the layout-level modal and clear the dock
			// row so the user sees one clear surface instead of the
			// bare "!" tooltip + a hidden modal.
			if (isFfmpegMissingPayload(e)) {
				downloadFailureStore.show({ kind: 'ffmpeg_missing' });
				downloadStore.dismiss(id);
				return;
			}
			const msg =
				typeof e === 'object' && e !== null && 'message' in e
					? String((e as { message: unknown }).message)
					: typeof e === 'string'
						? e
						: 'Download failed';
			downloadStore.markError(id, msg);
		});

	return id;
}
