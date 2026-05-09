/**
 * Shared download state — a Svelte 5 rune store that lives outside any
 * single component so the topbar dock, the bottom progress strip, and
 * the per-download confirm modal all observe the same list.
 *
 * Lifecycle of a download item:
 *   1. `addPending(args)` returns the new id; status = "pending"
 *      (modal opened, awaiting user confirm).
 *   2. `markActive(id)` flips to "active" and starts the SSE stream.
 *      `setProgress(id, line)` updates the latest progress line.
 *   3. `markDone(id, destDir)` flips to "done"; renderer can then
 *      offer "reveal in folder".
 *   4. `markError(id, message)` flips to "error".
 *   5. `dismiss(id)` removes the row.
 *
 * Active downloads also carry an `AbortController` so the topbar
 * dock's cancel button can abort the in-flight fetch — the SSE
 * connection closes, the backend's `kill_on_drop(true)` reaps the
 * ani-cli child.
 */

export type DownloadStatus = 'pending' | 'active' | 'done' | 'error';

export interface DownloadItem {
	id: string;
	title: string;
	episode: string;
	mode: string;
	quality: string;
	destDir: string;
	status: DownloadStatus;
	progress: string | null;
	error: string | null;
	startedAt: number;
	abort: AbortController | null;
}

let nextId = 1;

class DownloadStore {
	items = $state<DownloadItem[]>([]);

	get active(): DownloadItem[] {
		return this.items.filter((i) => i.status === 'pending' || i.status === 'active');
	}
	get hasActive(): boolean {
		return this.active.length > 0;
	}

	add(args: {
		title: string;
		episode: string;
		mode: string;
		quality: string;
		destDir: string;
	}): string {
		const id = `dl-${nextId++}`;
		this.items = [
			{
				id,
				title: args.title,
				episode: args.episode,
				mode: args.mode,
				quality: args.quality,
				destDir: args.destDir,
				status: 'pending',
				progress: null,
				error: null,
				startedAt: Date.now(),
				abort: null
			},
			...this.items
		];
		return id;
	}

	markActive(id: string, abort: AbortController) {
		this.items = this.items.map((i) =>
			i.id === id ? { ...i, status: 'active', abort, startedAt: Date.now() } : i
		);
	}

	setProgress(id: string, line: string) {
		this.items = this.items.map((i) => (i.id === id ? { ...i, progress: line } : i));
	}

	markDone(id: string, destDir: string) {
		this.items = this.items.map((i) =>
			i.id === id ? { ...i, status: 'done', destDir, abort: null } : i
		);
	}

	markError(id: string, message: string) {
		this.items = this.items.map((i) =>
			i.id === id ? { ...i, status: 'error', error: message, abort: null } : i
		);
	}

	cancel(id: string) {
		const item = this.items.find((i) => i.id === id);
		if (item?.abort) item.abort.abort();
		// markError will be called by the stream handler on abort; if
		// the abort fired before SSE wired up, drop the row directly.
		if (item?.status === 'pending') this.dismiss(id);
	}

	dismiss(id: string) {
		this.items = this.items.filter((i) => i.id !== id);
	}
}

export const downloadStore = new DownloadStore();
