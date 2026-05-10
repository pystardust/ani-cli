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
	import { m } from '$lib/paraglide/messages';

	interface Props {
		open: boolean;
		args: DownloadArgs | null;
		defaultDir: string;
		/** Overrides args.episode_count for the "All" + Range upper
		 *  bound when Kitsu has indexed fewer episodes than the show
		 *  announces (e.g. currently-airing seasons: episode_count=19
		 *  but only 5 aired). Detail page passes its
		 *  knownAvailableEpisodes; otherwise null. */
		availableEpisodes?: number | null;
		/** Whether to offer the "This episode" segment. The /play page
		 *  passes true (the playing episode is the obvious default);
		 *  the detail page passes false — there's no current episode
		 *  there, so Range with equal start/end covers the single-ep
		 *  case. When false, default mode is "range". */
		showThisEpisode?: boolean;
		onClose: () => void;
	}
	let {
		open = $bindable(),
		args,
		defaultDir,
		availableEpisodes = null,
		showThisEpisode = true,
		onClose
	}: Props = $props();

	// Effective max — what "All" actually targets and what Range
	// inputs clamp to. Falls back to the announced count when we
	// don't have a tighter bound.
	const maxEpisode = $derived(availableEpisodes ?? args?.episode_count ?? null);
	const announcedEpisode = $derived(args?.episode_count ?? null);
	const hasGap = $derived(
		availableEpisodes != null && announcedEpisode != null && availableEpisodes < announcedEpisode
	);
	// Range upper bound. When the episode count is unknown (Kitsu
	// gap, ova, currently-airing without count), cap at 10 so a
	// stray "200" in the To input can't kick off a runaway loop.
	const RANGE_FALLBACK_CAP = 10;
	const rangeMax = $derived(maxEpisode ?? RANGE_FALLBACK_CAP);

	let dir = $state('');
	let busy = $state(false);
	// Mode picker — three explicit choices instead of asking the user
	// to derive "I want all episodes" from typing 1..N into From/To.
	// "this" defaults to args.episode (the episode the user clicked
	// from); "range" surfaces From/To inputs; "all" expands 1..episode_count.
	type Mode = 'this' | 'all' | 'range';
	let mode = $state<Mode>('this');
	let startEp = $state(1);
	let endEp = $state(1);

	$effect(() => {
		if (open) {
			dir = args?.download_dir && args.download_dir.length > 0 ? args.download_dir : defaultDir;
			busy = false;
			const initial = args ? Number.parseInt(args.episode, 10) : 1;
			startEp = Number.isFinite(initial) && initial > 0 ? initial : 1;
			endEp = startEp;
			// On /play: This is the obvious default (current ep). On the
			// detail page where This is hidden, prefer All when we know
			// the count (most common intent — grab the season); fall
			// back to Range when count is unknown.
			mode = showThisEpisode ? 'this' : maxEpisode ? 'all' : 'range';
		}
	});

	const episodeArg = $derived.by(() => {
		if (mode === 'this') return args ? args.episode : '1';
		if (mode === 'all') {
			const last = maxEpisode ?? Math.floor(endEp);
			return `1-${Math.max(1, last)}`;
		}
		// range — clamp end to the rangeMax so the unknown-count case
		// can't ask ani-cli for episodes 1-200.
		const s = Math.max(1, Math.floor(startEp));
		const eRaw = Math.floor(endEp);
		const e = Math.max(s, Math.min(rangeMax, eRaw));
		return s === e ? String(s) : `${s}-${e}`;
	});
	const rangeCount = $derived.by(() => {
		if (mode === 'this') return 1;
		if (mode === 'all') return Math.max(1, maxEpisode ?? 1);
		return Math.max(1, Math.floor(endEp) - Math.floor(startEp) + 1);
	});

	// Range validation. Returns null when valid, a human-readable
	// message when not. Used to disable Confirm and surface an inline
	// error row beneath the inputs. Only checked while in `range`
	// mode — This and All build their episode arg from constants.
	const rangeError = $derived.by(() => {
		if (mode !== 'range') return null;
		const s = Math.floor(startEp);
		const e = Math.floor(endEp);
		if (!Number.isFinite(startEp) || !Number.isFinite(endEp)) {
			return 'Enter a number for both From and To.';
		}
		if (s < 1 || e < 1) return 'Episode numbers must be at least 1.';
		if (s > rangeMax) {
			return maxEpisode
				? `Only ${maxEpisode} episode${maxEpisode === 1 ? '' : 's'} available — From can't exceed ${maxEpisode}.`
				: `You can't download more than ${rangeMax} episodes for this show.`;
		}
		if (e > rangeMax) {
			return maxEpisode
				? `Only ${maxEpisode} episode${maxEpisode === 1 ? '' : 's'} available — To can't exceed ${maxEpisode}.`
				: `You can't download more than ${rangeMax} episodes for this show.`;
		}
		if (e < s) return 'To must be greater than or equal to From.';
		return null;
	});
	const startInvalid = $derived(
		mode === 'range' &&
			(!Number.isFinite(startEp) || Math.floor(startEp) < 1 || Math.floor(startEp) > rangeMax)
	);
	const endInvalid = $derived(
		mode === 'range' &&
			(!Number.isFinite(endEp) ||
				Math.floor(endEp) < 1 ||
				Math.floor(endEp) > rangeMax ||
				Math.floor(endEp) < Math.floor(startEp))
	);

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
				<!-- Eyebrow is purely a label here. The earlier "episode N"
				     value made the modal read as fixed-on-one-episode even
				     though the body's From/To inputs let the user pick a
				     range; the body's hint line surfaces the live count
				     and is the source of truth. -->
				<p class="dl-eyebrow">
					<span class="dl-eyebrow-key">{m.download_eyebrow_key()}</span>
					<span class="dl-eyebrow-rule" aria-hidden="true"></span>
					<span class="dl-eyebrow-value">
						{mode === 'this'
							? m.download_eyebrow_value_this({ episode: args.episode })
							: mode === 'all'
								? m.download_eyebrow_value_all({ count: maxEpisode ?? '?' })
								: m.download_eyebrow_value_range({
										start: Math.floor(startEp),
										end: Math.floor(endEp)
									})}
					</span>
				</p>
				<h2 id="dl-confirm-title" class="dl-title">{args.title}</h2>
			</header>

			<div class="dl-body">
				<span class="dl-label">{m.download_label_episodes()}</span>
				<!-- Order varies by surface: /play has a clear "current"
				     episode so This → Range → All keeps the obvious
				     default first. The detail page has no current
				     referent and the user is more likely to grab the
				     whole season than to dial in a range, so put All
				     ahead of Range there. -->
				<div class="dl-mode" role="group" aria-label={m.download_label_episodes()}>
					{#if showThisEpisode}
						<button
							type="button"
							class="dl-mode-btn"
							class:active={mode === 'this'}
							aria-pressed={mode === 'this'}
							onclick={() => (mode = 'this')}
						>
							{m.download_mode_this_episode()}
							<span class="dl-mode-num">{args.episode}</span>
						</button>
						<button
							type="button"
							class="dl-mode-btn"
							class:active={mode === 'range'}
							aria-pressed={mode === 'range'}
							onclick={() => (mode = 'range')}
						>
							{m.download_mode_range()}
						</button>
						<!-- aria-disabled keeps the button hoverable so the
						     custom tooltip fires immediately on hover. The
						     wrapper carries the tooltip; native title is
						     dropped because Chromium delays it ~600ms. -->
						<span class="dl-mode-tip-wrap">
							<button
								type="button"
								class="dl-mode-btn"
								class:active={mode === 'all'}
								class:dl-mode-btn-disabled={!maxEpisode}
								aria-pressed={mode === 'all'}
								aria-disabled={!maxEpisode}
								onclick={() => maxEpisode && (mode = 'all')}
							>
								{m.download_mode_all()}
								{#if maxEpisode}
									<span class="dl-mode-num">{maxEpisode}</span>
								{/if}
							</button>
							{#if !maxEpisode}
								<span class="dl-tip" role="tooltip">
									{m.download_disabled_all_tooltip()}
								</span>
							{/if}
						</span>
					{:else}
						<!-- aria-disabled keeps the button hoverable so the
						     custom tooltip fires immediately on hover. The
						     wrapper carries the tooltip; native title is
						     dropped because Chromium delays it ~600ms. -->
						<span class="dl-mode-tip-wrap">
							<button
								type="button"
								class="dl-mode-btn"
								class:active={mode === 'all'}
								class:dl-mode-btn-disabled={!maxEpisode}
								aria-pressed={mode === 'all'}
								aria-disabled={!maxEpisode}
								onclick={() => maxEpisode && (mode = 'all')}
							>
								{m.download_mode_all()}
								{#if maxEpisode}
									<span class="dl-mode-num">{maxEpisode}</span>
								{/if}
							</button>
							{#if !maxEpisode}
								<span class="dl-tip" role="tooltip">
									{m.download_disabled_all_tooltip()}
								</span>
							{/if}
						</span>
						<button
							type="button"
							class="dl-mode-btn"
							class:active={mode === 'range'}
							aria-pressed={mode === 'range'}
							onclick={() => (mode = 'range')}
						>
							{m.download_mode_range()}
						</button>
					{/if}
				</div>

				{#if mode === 'range'}
					<div class="dl-row dl-row-eps">
						<input
							id="dl-start-ep"
							class="dl-input dl-input-num"
							class:dl-input-error={startInvalid}
							type="number"
							min="1"
							max={rangeMax}
							bind:value={startEp}
							aria-label={m.download_range_from_aria_label()}
							aria-invalid={startInvalid}
						/>
						<span class="dl-range-sep" aria-hidden="true">{m.download_range_separator()}</span>
						<input
							class="dl-input dl-input-num"
							class:dl-input-error={endInvalid}
							type="number"
							min={Math.floor(startEp)}
							max={rangeMax}
							bind:value={endEp}
							aria-label={m.download_range_to_aria_label()}
							aria-invalid={endInvalid}
						/>
						{#if maxEpisode}
							<span class="dl-range-total">
								{m.download_range_of({ total: maxEpisode })}
								{#if hasGap && announcedEpisode != null}
									<span class="dl-range-faint"
										>{m.download_range_announced_suffix({ count: announcedEpisode })}</span
									>
								{/if}
							</span>
						{:else}
							<span class="dl-range-total">
								{m.download_range_max_prefix({ max: rangeMax })}
								<span class="dl-range-faint">{m.download_range_unknown_suffix()}</span>
							</span>
						{/if}
					</div>
					{#if rangeError}
						<p class="dl-error" role="alert" aria-live="polite">{rangeError}</p>
					{/if}
				{/if}

				<p class="dl-hint">
					{#if mode === 'this'}
						{m.download_hint_this({ episode: args.episode })}
					{:else if mode === 'all'}
						{#if maxEpisode}
							{m.download_hint_all_prefix_total({ count: maxEpisode })}
							{hasGap
								? m.download_hint_all_aired()
								: ''}{m.download_hint_all_episodes_suffix()}{#if hasGap && announcedEpisode != null}{m.download_hint_all_gap_suffix(
									{
										announced: announcedEpisode,
										available: maxEpisode
									}
								)}{:else}{m.download_hint_all_nogap_suffix()}{/if}
						{:else}
							{m.download_hint_all_unknown()}
						{/if}
					{:else}
						{m.download_hint_range({ count: rangeCount })}
					{/if}
				</p>

				<label class="dl-label" for="dl-dir-input">{m.download_label_directory()}</label>
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
						<span>{m.download_button_browse()}</span>
					</button>
				</div>
				<p class="dl-hint">
					{m.download_hint_filename_prefix()}<code
						>{m.download_hint_filename_template({ title: args.title })}</code
					>{m.download_hint_filename_suffix()}
				</p>
			</div>

			<footer class="dl-foot">
				<button type="button" class="dl-btn dl-btn-quiet" onclick={close} disabled={busy}>
					{m.download_button_cancel()}
				</button>
				<button
					type="button"
					class="dl-btn dl-btn-accent"
					onclick={confirm}
					disabled={busy || !dir.trim() || rangeError !== null}
				>
					{busy ? m.download_button_confirming() : m.download_button_confirm()}
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
		margin-block-start: var(--space-2);
	}
	.dl-mode {
		display: flex;
		gap: 1px;
		padding: 2px;
		background: var(--ink-000);
		border: 1px solid var(--ink-200);
		border-radius: var(--radius-sm);
		margin-block-start: var(--space-2);
	}
	.dl-mode-btn {
		flex: 1 1 0;
		display: inline-flex;
		align-items: baseline;
		justify-content: center;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-3);
		background: transparent;
		border: 1px solid transparent;
		border-radius: calc(var(--radius-sm) - 2px);
		color: var(--bone-200);
		font-family: var(--font-body);
		font-size: var(--type-body-s);
		font-weight: 500;
		cursor: pointer;
		transition:
			background var(--dur-fast) var(--ease-out-soft),
			color var(--dur-fast) var(--ease-out-soft);
	}
	.dl-mode-btn:hover:not(:disabled):not(.active) {
		background: color-mix(in oklab, var(--bone-100) 4%, transparent);
		color: var(--bone-100);
	}
	.dl-mode-btn.active {
		background: color-mix(in oklab, var(--accent) 22%, var(--ink-050));
		color: var(--bone-100);
		border-color: color-mix(in oklab, var(--accent) 50%, var(--bone-400));
	}
	.dl-mode-btn:disabled,
	.dl-mode-btn-disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}
	/* aria-disabled buttons stay hover-target so the custom tooltip
	   fires; visual disabled and hover suppression match :disabled. */
	.dl-mode-btn-disabled:hover:not(.active) {
		background: transparent;
		color: var(--bone-200);
	}
	/* Wrapper around an aria-disabled button so the tooltip can sit
	   above the segmented row without affecting flex layout. */
	.dl-mode-tip-wrap {
		position: relative;
		flex: 1 1 0;
		display: flex;
	}
	.dl-mode-tip-wrap > .dl-mode-btn {
		flex: 1 1 auto;
		inline-size: 100%;
	}
	/* Custom tooltip — fires immediately on hover (no native delay).
	   Anchored above the button with a small triangle pointer. */
	.dl-tip {
		position: absolute;
		inset-block-end: calc(100% + 6px);
		inset-inline: 50% auto auto auto;
		transform: translateX(-50%);
		left: 50%;
		min-inline-size: 12rem;
		max-inline-size: 16rem;
		padding: var(--space-2) var(--space-3);
		background: var(--ink-000);
		color: var(--bone-100);
		border: 1px solid var(--ink-200);
		border-radius: var(--radius-sm);
		box-shadow: 0 8px 18px -6px rgb(0 0 0 / 0.55);
		font-family: var(--font-body);
		font-size: var(--type-meta);
		line-height: 1.35;
		text-align: center;
		opacity: 0;
		pointer-events: none;
		transition: opacity 80ms var(--ease-out-soft);
		z-index: 50;
	}
	.dl-tip::after {
		content: '';
		position: absolute;
		inset-block-start: 100%;
		left: 50%;
		transform: translateX(-50%);
		border: 5px solid transparent;
		border-block-start-color: var(--ink-200);
	}
	.dl-mode-tip-wrap:hover > .dl-tip,
	.dl-mode-tip-wrap:focus-within > .dl-tip {
		opacity: 1;
	}
	.dl-mode-num {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		color: var(--bone-300);
	}
	.dl-mode-btn.active .dl-mode-num {
		color: color-mix(in oklab, var(--accent) 80%, var(--bone-100));
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
	.dl-range-faint {
		color: color-mix(in oklab, var(--bone-300) 60%, transparent);
		margin-inline-start: var(--space-1);
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
	/* Invalid Range input — oxblood border, faint background tint;
	   keeps the focus halo readable on top via box-shadow stack. */
	.dl-input-error {
		border-color: var(--oxblood, #b11a1a);
		background: color-mix(in oklab, var(--oxblood, #b11a1a) 10%, var(--ink-000));
	}
	.dl-input-error:focus-visible {
		border-color: var(--oxblood, #b11a1a);
		box-shadow: 0 0 0 2px color-mix(in oklab, var(--oxblood, #b11a1a) 35%, transparent);
	}
	.dl-error {
		margin: var(--space-2) 0 0;
		font-family: var(--font-body);
		font-size: var(--type-meta);
		color: var(--oxblood, #b11a1a);
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
