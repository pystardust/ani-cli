/**
 * Glue between the download api wrapper and the shared download
 * store. Single entry point used by the confirm modal: `startDownload`
 * adds a row, opens the SSE, and feeds progress / final / error
 * events into the store. Returns the new id so callers can show a
 * targeted toast / focus the dock row.
 */

import { downloadStream, type DownloadArgs } from '$lib/api';
import { downloadStore } from './store.svelte';

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
