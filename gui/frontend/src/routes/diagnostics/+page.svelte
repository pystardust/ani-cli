<!--
  Backend Status — proves IPC works end-to-end. Loads app_info +
  history_list on mount, exposes history_clear. Plain HTML on purpose;
  the design pass (M3) will replace this entirely.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import {
		anicliUpdateLog,
		appInfo,
		historyClear,
		historyList,
		metaCacheClear,
		type AnicliUpdateOutcome,
		type AppInfo,
		type HistoryEntry
	} from '$lib/api';
	import { m } from '$lib/paraglide/messages';

	let info = $state<AppInfo | null>(null);
	let history = $state<HistoryEntry[] | null>(null);
	let infoError = $state<string | null>(null);
	let historyError = $state<string | null>(null);
	let busy = $state(false);
	let cacheStatus = $state<string | null>(null);
	let cacheError = $state<string | null>(null);
	let anicliLog = $state<AnicliUpdateOutcome[] | null>(null);
	let anicliLogLoading = $state(false);
	let anicliLogError = $state<string | null>(null);
	let anicliDialog = $state<HTMLDialogElement | null>(null);

	async function refresh() {
		infoError = null;
		historyError = null;
		try {
			info = await appInfo();
		} catch (e) {
			infoError = describeError(e);
		}
		try {
			history = await historyList();
		} catch (e) {
			historyError = describeError(e);
		}
	}

	async function openAnicliLog() {
		anicliDialog?.showModal();
		// Always re-fetch on open so the dialog reflects the latest
		// state (e.g. a -U that finished while the user was poking
		// around the rest of the page).
		anicliLogLoading = true;
		anicliLogError = null;
		try {
			anicliLog = await anicliUpdateLog();
		} catch (e) {
			anicliLogError = describeError(e);
		} finally {
			anicliLogLoading = false;
		}
	}
	function closeAnicliLog() {
		anicliDialog?.close();
	}

	async function clearHistory() {
		busy = true;
		try {
			await historyClear();
			history = [];
		} catch (e) {
			historyError = describeError(e);
		} finally {
			busy = false;
		}
	}

	async function clearMetaCache() {
		busy = true;
		cacheStatus = null;
		cacheError = null;
		try {
			await metaCacheClear();
			cacheStatus = m.diagnostics_cache_clear_success();
		} catch (e) {
			cacheError = describeError(e);
		} finally {
			busy = false;
		}
	}

	function describeError(e: unknown): string {
		if (typeof e === 'object' && e !== null) {
			const obj = e as Record<string, unknown>;
			if (typeof obj.kind === 'string') return obj.kind;
		}
		return String(e);
	}

	onMount(refresh);
</script>

<main class="page">
	<h1>{m.diagnostics_title()}</h1>

	<section>
		<h2>{m.diagnostics_section_app_info()}</h2>
		{#if infoError}
			<p>{m.diagnostics_app_info_error()} {infoError}</p>
		{:else if info}
			<dl>
				<dt>{m.diagnostics_app_info_version()}</dt>
				<dd>{info.version}</dd>
				<dt>{m.diagnostics_app_info_ani_cli()}</dt>
				<dd>{info.ani_cli_path}</dd>
				<dt>{m.diagnostics_app_info_history_file()}</dt>
				<dd>{info.history_path}</dd>
				<dt>{m.diagnostics_app_info_proxy_base()}</dt>
				<dd>{info.proxy_base_url}</dd>
			</dl>
		{:else}
			<p>{m.diagnostics_app_info_loading()}</p>
		{/if}
	</section>

	<section>
		<h2>
			{#if history === null}
				{m.diagnostics_section_history_title_loading()}
			{:else}
				{m.diagnostics_section_history_title({ count: history.length })}
			{/if}
		</h2>
		{#if historyError}
			<p>{m.diagnostics_history_error()} {historyError}</p>
		{:else if history === null}
			<p>{m.diagnostics_history_loading()}</p>
		{:else if history.length === 0}
			<p>{m.diagnostics_history_empty()}</p>
		{:else}
			<ul>
				{#each history as entry (entry.id)}
					<li>
						{m.diagnostics_history_entry_format({
							title: entry.title,
							epNo: entry.ep_no,
							id: entry.id
						})}
					</li>
				{/each}
			</ul>
		{/if}
		<p>
			<button type="button" onclick={refresh} disabled={busy}
				>{m.diagnostics_refresh_button()}</button
			>
			<button type="button" onclick={clearHistory} disabled={busy || history?.length === 0}>
				{m.diagnostics_clear_history_button()}
			</button>
		</p>
	</section>

	<section>
		<h2>{m.diagnostics_section_anicli_update()}</h2>
		<p>
			<button type="button" onclick={openAnicliLog}>
				{m.diagnostics_anicli_update_show_log()}
			</button>
		</p>
		<dialog bind:this={anicliDialog} class="anicli-dialog">
			<header class="anicli-dialog-head">
				<h3>{m.diagnostics_section_anicli_update()}</h3>
				<button
					type="button"
					class="anicli-dialog-close"
					onclick={closeAnicliLog}
					aria-label={m.diagnostics_anicli_update_hide_log()}>×</button
				>
			</header>
			<div class="anicli-dialog-body">
				{#if anicliLogLoading}
					<p>{m.diagnostics_anicli_update_loading()}</p>
				{:else if anicliLogError}
					<p>{m.diagnostics_anicli_update_load_error()} {anicliLogError}</p>
				{:else if anicliLog && anicliLog.length === 0}
					<p>{m.diagnostics_anicli_update_never()}</p>
				{:else if anicliLog}
					<div class="anicli-log">
						{#each anicliLog as entry, i (i)}
							<details class="anicli-row">
								<summary>
									<span class="anicli-row-status anicli-row-status-{entry.status}">
										{#if entry.status === 'no_change'}
											{m.diagnostics_anicli_update_status_no_change()}
										{:else if entry.status === 'updated'}
											{m.diagnostics_anicli_update_status_updated()}
										{:else}
											{m.diagnostics_anicli_update_status_failed()}
										{/if}
									</span>
									<span class="anicli-row-time">{entry.finished_at}</span>
									<span class="anicli-row-dur"
										>{m.diagnostics_anicli_update_duration_value({
											ms: entry.duration_ms
										})}</span
									>
								</summary>
								<div class="anicli-row-body">
									<div class="anicli-row-pair">
										<span class="anicli-row-label"
											>{m.diagnostics_anicli_update_stdout_label()}</span
										>
										<pre>{entry.stdout || m.diagnostics_anicli_update_empty_output()}</pre>
									</div>
									{#if entry.stderr}
										<div class="anicli-row-pair">
											<span class="anicli-row-label"
												>{m.diagnostics_anicli_update_stderr_label()}</span
											>
											<pre>{entry.stderr}</pre>
										</div>
									{/if}
								</div>
							</details>
						{/each}
					</div>
				{/if}
			</div>
		</dialog>
	</section>

	<section>
		<h2>{m.diagnostics_section_metadata_cache()}</h2>
		<p>
			{m.diagnostics_metadata_cache_description()}
		</p>
		<p>
			<button type="button" onclick={clearMetaCache} disabled={busy}
				>{m.diagnostics_clear_metadata_cache_button()}</button
			>
		</p>
		{#if cacheStatus}<p>{cacheStatus}</p>{/if}
		{#if cacheError}<p>{m.diagnostics_cache_clear_error()} {cacheError}</p>{/if}
	</section>

	<section>
		<h2>{m.diagnostics_section_search_kitsu()}</h2>
		<p>
			<a href={resolve('/search')}>{m.diagnostics_open_search_link_text()}</a
			>{m.diagnostics_open_search_link_hint()}
		</p>
	</section>

	<section>
		<h2>{m.diagnostics_section_test_stream()}</h2>
		<p>
			<a href={resolve('/play')}>{m.diagnostics_open_test_stream_link_text()}</a
			>{m.diagnostics_open_test_stream_link_hint()}
		</p>
	</section>
</main>

<style>
	/* Diagnostics is a utilitarian, design-pass-exempt page. Just
	   enough wrapping to keep the back button and prose off the rail
	   edge — same gutter the rest of the app uses. */
	.page {
		max-inline-size: var(--content-max);
		padding: var(--space-5) var(--space-8) var(--space-8);
	}
	.page section {
		margin-block-start: var(--space-5);
	}

	/* Modal dialog hosting the ani-cli update log. Keeps the
	   diagnostics page compact (just the launch button) and lets the
	   user scan history in a focused surface that closes on Escape
	   or the close button. */
	.anicli-dialog {
		inline-size: min(48rem, 90vw);
		max-block-size: 80vh;
		padding: 0;
		border: 1px solid var(--ink-200);
		border-radius: 8px;
		background: var(--ink-000);
		color: var(--bone-100);
	}
	.anicli-dialog::backdrop {
		background: rgba(0, 0, 0, 0.6);
	}
	.anicli-dialog-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.75rem 1rem;
		border-block-end: 1px solid var(--ink-200);
	}
	.anicli-dialog-head h3 {
		margin: 0;
		font-size: 1rem;
	}
	.anicli-dialog-close {
		font-size: 1.25rem;
		line-height: 1;
		padding: 0.1rem 0.5rem;
		background: transparent;
		border: 0;
		color: var(--bone-300);
		cursor: pointer;
	}
	.anicli-dialog-close:hover {
		color: var(--bone-100);
	}
	.anicli-dialog-body {
		padding: 0.75rem 1rem 1rem;
		overflow-y: auto;
		max-block-size: calc(80vh - 3rem);
	}

	/* Rolling log of recent ani-cli -U attempts. Each row is a
	   <details> so the user can fold the captured stdout/stderr.
	   Scrolling lives on the dialog body, not on the list itself. */
	.anicli-log {
		border: 1px solid var(--ink-200);
		border-radius: 4px;
	}
	.anicli-row {
		border-block-end: 1px solid var(--ink-200);
	}
	.anicli-row:last-child {
		border-block-end: 0;
	}
	.anicli-row > summary {
		display: grid;
		grid-template-columns: 12rem 1fr 6rem;
		gap: 0.75rem;
		padding: 0.4rem 0.6rem;
		cursor: pointer;
		font-family: var(--font-mono);
		font-size: 0.85rem;
	}
	.anicli-row > summary:hover {
		background: var(--ink-050);
	}
	.anicli-row-status {
		text-transform: uppercase;
		letter-spacing: 0.06em;
		font-weight: 600;
	}
	.anicli-row-status-no_change {
		color: var(--bone-300);
	}
	.anicli-row-status-updated {
		color: var(--accent, var(--bone-100));
	}
	.anicli-row-status-failed {
		color: var(--accent-oxblood, #d04848);
	}
	.anicli-row-time {
		color: var(--bone-200);
	}
	.anicli-row-dur {
		color: var(--bone-300);
		text-align: end;
	}
	.anicli-row-body {
		padding: 0.4rem 0.6rem 0.6rem;
		background: var(--ink-050);
	}
	.anicli-row-pair + .anicli-row-pair {
		margin-block-start: 0.5rem;
	}
	.anicli-row-label {
		display: block;
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: var(--bone-300);
		text-transform: uppercase;
		letter-spacing: 0.06em;
	}
	.anicli-row-body pre {
		margin: 0.2rem 0 0;
		font-family: var(--font-mono);
		font-size: 0.8rem;
		white-space: pre-wrap;
		word-break: break-word;
	}
</style>
