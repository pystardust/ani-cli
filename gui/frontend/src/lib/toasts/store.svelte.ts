/**
 * Toast store — module-singleton rune store mirroring the shape of
 * `download/store.svelte.ts`. Holds the ephemeral notifications
 * surfaced bottom-right of the window. Auto-dismiss timers are
 * owned by the store itself so call sites (play page, settings,
 * future Syncplay entry) don't have to manage their own timeouts.
 *
 * Stack policy: at most TOAST_MAX_STACK rows on-screen. Spam-clicks
 * trim the oldest entries so the corner doesn't fill up with stale
 * "Sent to mpv." rows piling on a single retry burst.
 */

export type ToastKind = 'success' | 'info' | 'warning' | 'error';

export interface ToastItem {
	id: string;
	kind: ToastKind;
	message: string;
	/** Auto-dismiss after this many ms. `null` pins the toast — only
	 *  user dismiss removes it. Useful for action-required toasts the
	 *  user shouldn't miss. */
	duration: number | null;
	actionLabel: string | null;
	onAction: (() => void) | null;
}

export interface PushArgs {
	kind: ToastKind;
	message: string;
	/** Defaults to 4000ms when omitted. `null` to pin. */
	duration?: number | null;
	actionLabel?: string;
	onAction?: () => void;
}

export const TOAST_MAX_STACK = 3;

const DEFAULT_DURATION_MS = 4000;

let nextId = 1;

class ToastStore {
	items: ToastItem[] = $state([]);

	push(args: PushArgs): string {
		const id = `toast-${nextId++}`;
		const duration = args.duration === undefined ? DEFAULT_DURATION_MS : args.duration;
		const row: ToastItem = {
			id,
			kind: args.kind,
			message: args.message,
			duration,
			actionLabel: args.actionLabel ?? null,
			onAction: args.onAction ?? null
		};
		// Trim oldest entries if a push would exceed the cap. Slice +
		// concat rather than splice so the assignment triggers
		// reactivity (matches download/store.svelte.ts's pattern).
		const trimmed = this.items.slice(-(TOAST_MAX_STACK - 1));
		this.items = [...trimmed, row];
		if (duration !== null) {
			// dismiss(id) is a no-op when the row has already been
			// trimmed by a later push — see the store test.
			setTimeout(() => this.dismiss(id), duration);
		}
		return id;
	}

	dismiss(id: string): void {
		const next = this.items.filter((i) => i.id !== id);
		if (next.length !== this.items.length) this.items = next;
	}
}

export const toastStore = new ToastStore();
