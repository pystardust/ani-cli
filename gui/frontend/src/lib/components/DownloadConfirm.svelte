<!--
  DownloadConfirm — modal that confirms a download before kicking it
  off. Shows the episode being saved + the destination directory,
  with a Browse… button that opens the OS folder picker (via the
  Electron preload bridge). Confirm starts the download via
  `startDownload` and closes; Cancel closes without doing anything.

  The default destination comes from the backend's `download_dir()`
  resolver — XDG_DOWNLOAD_DIR/ani-gui or HOME/Downloads/ani-gui — and
  is editable inline before confirming.
-->
<script lang="ts">
	import { startDownload } from '$lib/download/start';
	import type { DownloadArgs } from '$lib/api';

	interface Props {
		open: boolean;
		args: DownloadArgs | null;
		defaultDir: string;
		onClose: () => void;
	}
	let { open = $bindable(), args, defaultDir, onClose }: Props = $props();

	let dir = $state('');
	let busy = $state(false);
	// Range fields. start defaults to args.episode (the episode the
	// user clicked from); end defaults to start so a fresh open reads
	// as "single-episode download" until the user opts in. ani-cli's
	// `-e M-N` loops sequentially so the range can be arbitrary; we
	// just clamp to args.episode_count when known.
	let startEp = $state(1);
	let endEp = $state(1);

	$effect(() => {
		if (open) {
			dir = args?.download_dir && args.download_dir.length > 0 ? args.download_dir : defaultDir;
			busy = false;
			const initial = args ? Number.parseInt(args.episode, 10) : 1;
			startEp = Number.isFinite(initial) && initial > 0 ? initial : 1;
			endEp = startEp;
		}
	});

	const episodeArg = $derived.by(() => {
		const s = Math.max(1, Math.floor(startEp));
		const e = Math.max(s, Math.floor(endEp));
		return s === e ? String(s) : `${s}-${e}`;
	});
	const isRange = $derived(Math.floor(endEp) > Math.floor(startEp));
	const rangeCount = $derived(Math.max(1, Math.floor(endEp) - Math.floor(startEp) + 1));

	async function browse() {
		const picker = typeof window !== 'undefined' ? window.aniGui?.pickDirectory : null;
		if (!picker) return; // dev-mode browser without preload — leave dir as-is
		const picked = await picker({
			title: 'Choose download folder',
			defaultPath: dir
		});
		if (picked) dir = picked;
	}

	function close() {
		if (busy) return;
		open = false;
		onClose();
	}

	function confirm() {
		if (!args || !dir.trim()) return;
		// Build the episode arg as either "5" or "5-12" — ani-cli's
		// -e accepts both and loops the range sequentially with -d.
		startDownload({
			...args,
			episode: episodeArg,
			download_dir: dir,
			destDir: dir
		});
		open = false;
		onClose();
	}

	function onBackdropKey(e: KeyboardEvent) {
		if (e.key === 'Escape') close();
	}
</script>

{#if open && args}
	<!-- Backdrop. role="dialog" + aria-modal=true keeps screen readers
	     trapped inside until close. Click on backdrop dismisses. -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<div
		class="dl-backdrop"
		onclick={close}
		onkeydown={onBackdropKey}
		role="dialog"
		aria-modal="true"
		aria-labelledby="dl-confirm-title"
		tabindex="-1"
	>
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<div class="dl-card" onclick={(e) => e.stopPropagation()}>
			<header class="dl-head">
				<p class="dl-eyebrow">
					<span class="dl-eyebrow-key">Download</span>
					<span class="dl-eyebrow-rule" aria-hidden="true"></span>
					<span class="dl-eyebrow-value">
						{isRange
							? `episodes ${Math.floor(startEp)}–${Math.floor(endEp)}`
							: `episode ${Math.floor(startEp)}`}
					</span>
				</p>
				<h2 id="dl-confirm-title" class="dl-title">{args.title}</h2>
			</header>

			<div class="dl-body">
				<label class="dl-label" for="dl-start-ep">Episodes</label>
				<div class="dl-row dl-row-eps">
					<input
						id="dl-start-ep"
						class="dl-input dl-input-num"
						type="number"
						min="1"
						max={args.episode_count ?? undefined}
						bind:value={startEp}
						aria-label="From episode"
					/>
					<span class="dl-range-sep" aria-hidden="true">–</span>
					<input
						class="dl-input dl-input-num"
						type="number"
						min={Math.floor(startEp)}
						max={args.episode_count ?? undefined}
						bind:value={endEp}
						aria-label="To episode"
					/>
					{#if args.episode_count}
						<span class="dl-range-total">of {args.episode_count}</span>
					{/if}
				</div>
				<p class="dl-hint">
					{#if isRange}
						Downloads {rangeCount} episodes sequentially via ani-cli's range mode.
					{:else}
						Single episode. Set "to" higher than "from" to download a range.
					{/if}
				</p>

				<label class="dl-label" for="dl-dir-input">Save to</label>
				<div class="dl-row">
					<input
						id="dl-dir-input"
						class="dl-input"
						type="text"
						bind:value={dir}
						spellcheck="false"
						autocomplete="off"
					/>
					<button type="button" class="dl-btn dl-btn-quiet" onclick={browse}>
						<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
							<path
								d="M3 7h6l2 2h10v10H3z"
								fill="none"
								stroke="currentColor"
								stroke-width="2"
								stroke-linecap="round"
								stroke-linejoin="round"
							/>
						</svg>
						<span>Browse…</span>
					</button>
				</div>
				<p class="dl-hint">
					Files land as <code>{args.title} Episode N.mp4</code> per ani-cli's naming.
				</p>
			</div>

			<footer class="dl-foot">
				<button type="button" class="dl-btn dl-btn-quiet" onclick={close} disabled={busy}>
					Cancel
				</button>
				<button
					type="button"
					class="dl-btn dl-btn-accent"
					onclick={confirm}
					disabled={busy || !dir.trim()}
				>
					{busy ? 'Starting…' : 'Start download'}
				</button>
			</footer>
		</div>
	</div>
{/if}

<style>
	.dl-backdrop {
		position: fixed;
		inset: 0;
		background: color-mix(in oklab, var(--ink-000) 70%, transparent);
		backdrop-filter: blur(4px);
		display: grid;
		place-items: center;
		z-index: 100;
		animation: dl-fade var(--dur-fast) var(--ease-out-soft) both;
	}
	@keyframes dl-fade {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}
	.dl-card {
		inline-size: min(32rem, calc(100vw - var(--space-6)));
		background: var(--ink-050);
		border: 1px solid var(--ink-200);
		border-radius: var(--radius-card);
		box-shadow: var(--shadow-card-hover);
		padding: var(--space-5);
		animation: dl-rise var(--dur-med) var(--ease-out-elastic) both;
	}
	@keyframes dl-rise {
		from {
			opacity: 0;
			transform: translateY(8px) scale(0.98);
		}
		to {
			opacity: 1;
			transform: none;
		}
	}
	.dl-head {
		margin-block-end: var(--space-4);
	}
	.dl-eyebrow {
		margin: 0 0 var(--space-2);
		display: inline-flex;
		align-items: center;
		gap: var(--space-3);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.dl-eyebrow-key {
		color: var(--accent);
	}
	.dl-eyebrow-rule {
		display: inline-block;
		inline-size: 2rem;
		block-size: 1px;
		background: var(--accent);
	}
	.dl-title {
		margin: 0;
		font-family: var(--font-display);
		font-size: var(--type-h3);
		font-weight: 600;
		color: var(--bone-100);
		line-height: var(--leading-tight);
	}
	.dl-body {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin-block-end: var(--space-4);
	}
	.dl-label {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.dl-row {
		display: flex;
		gap: var(--space-2);
	}
	.dl-row-eps {
		align-items: baseline;
		gap: var(--space-3);
	}
	.dl-input-num {
		flex: 0 0 5rem;
		text-align: center;
		font-variant-numeric: tabular-nums;
	}
	.dl-input-num::-webkit-outer-spin-button,
	.dl-input-num::-webkit-inner-spin-button {
		appearance: auto;
	}
	.dl-range-sep {
		color: var(--bone-300);
		font-family: var(--font-mono);
	}
	.dl-range-total {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.dl-input {
		flex: 1 1 auto;
		min-inline-size: 0;
		padding: var(--space-2) var(--space-3);
		background: var(--ink-000);
		border: 1px solid var(--ink-200);
		border-radius: var(--radius-sm);
		color: var(--bone-100);
		font-family: var(--font-mono);
		font-size: var(--type-body-s);
	}
	.dl-input:focus-visible {
		outline: none;
		border-color: var(--accent);
		box-shadow: 0 0 0 2px color-mix(in oklab, var(--accent) 35%, transparent);
	}
	.dl-hint {
		margin: 0;
		font-family: var(--font-body);
		font-size: var(--type-meta);
		color: var(--bone-300);
	}
	.dl-hint code {
		font-family: var(--font-mono);
		color: var(--bone-200);
	}
	.dl-foot {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
	}
	.dl-btn {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-4);
		border-radius: var(--radius-sm);
		font-family: var(--font-body);
		font-size: var(--type-body-s);
		font-weight: 500;
		cursor: pointer;
		transition: background var(--dur-fast) var(--ease-out-soft);
	}
	.dl-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
	.dl-btn-quiet {
		background: transparent;
		border: 1px solid var(--ink-200);
		color: var(--bone-200);
	}
	.dl-btn-quiet:hover:not(:disabled) {
		background: color-mix(in oklab, var(--bone-100) 6%, transparent);
		color: var(--bone-100);
	}
	.dl-btn-accent {
		background: var(--accent);
		border: 1px solid var(--accent);
		color: var(--ink-000);
	}
	.dl-btn-accent:hover:not(:disabled) {
		background: color-mix(in oklab, var(--accent) 80%, var(--bone-100));
	}
</style>
