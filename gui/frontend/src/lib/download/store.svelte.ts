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
	/** Episode arg as sent to ani-cli — `"5"` for single, `"5-12"` for range. */
	episode: string;
	mode: string;
	quality: string;
	destDir: string;
	status: DownloadStatus;
	progress: string | null;
	error: string | null;
	startedAt: number;
	abort: AbortController | null;
	/** True when status flipped to "done" or "error" while the user
	 *  wasn't looking at the dock. Cleared the next time the dock
	 *  opens — drives the small completion badge on the topbar icon. */
	unseen: boolean;
	/** When the episode arg is `"M-N"`, the count of episodes in the
	 *  range — drives the dock's "Episode N of M" annotation. Null
	 *  for single-episode downloads. */
	rangeTotal: number | null;
	/** Last episode number ani-cli announced via `Playing episode N…`
	 *  on stderr. Updated by setProgress as lines arrive. Null until
	 *  the first such line is parsed. */
	currentEp: number | null;
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
	/** Items the dock hasn't surfaced yet — drives the small completion
	 *  badge on the topbar download icon. Cleared by `markAllSeen()`. */
	get unseenCount(): number {
		return this.items.reduce((n, i) => n + (i.unseen ? 1 : 0), 0);
	}

	add(args: {
		title: string;
		episode: string;
		mode: string;
		quality: string;
		destDir: string;
	}): string {
		const id = `dl-${nextId++}`;
		// Parse `"M-N"` to compute the range size up front so the dock
		// can show "Episode K of N-M+1" before any progress arrives.
		const rangeMatch = args.episode.match(/^(\d+)-(\d+)$/);
		const rangeTotal = rangeMatch
			? Math.max(1, Number.parseInt(rangeMatch[2], 10) - Number.parseInt(rangeMatch[1], 10) + 1)
			: null;
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
				abort: null,
				unseen: false,
				rangeTotal,
				currentEp: null
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
		// ani-cli prints `Playing episode N...` on each iteration of a
		// range download (line 448 of upstream). Parse it so the dock
		// can show "Episode N of M" without surfacing the raw line.
		const match = line.match(/^Playing episode\s+(\d+(?:\.\d+)?)/i);
		const currentEp = match ? Number.parseFloat(match[1]) : null;
		this.items = this.items.map((i) =>
			i.id === id ? { ...i, progress: line, currentEp: currentEp ?? i.currentEp } : i
		);
	}

	markDone(id: string, destDir: string) {
		this.items = this.items.map((i) =>
			i.id === id ? { ...i, status: 'done', destDir, abort: null, unseen: true } : i
		);
	}

	markError(id: string, message: string) {
		this.items = this.items.map((i) =>
			i.id === id ? { ...i, status: 'error', error: message, abort: null, unseen: true } : i
		);
	}

	/** Called when the dock opens — clears the unseen flag on every
	 *  done/errored item so the topbar dot fades. */
	markAllSeen() {
		if (this.items.every((i) => !i.unseen)) return;
		this.items = this.items.map((i) => (i.unseen ? { ...i, unseen: false } : i));
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
