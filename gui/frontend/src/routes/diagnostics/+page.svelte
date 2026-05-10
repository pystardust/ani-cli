<!--
  Backend Status — proves IPC works end-to-end. Loads app_info +
  history_list on mount, exposes history_clear. Plain HTML on purpose;
  the design pass (M3) will replace this entirely.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import {
		anicliUpdateStatus,
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
	let anicliUpdate = $state<AnicliUpdateOutcome | null>(null);

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
		try {
			anicliUpdate = await anicliUpdateStatus();
		} catch {
			// Soft-fail: the panel renders the "never run" hint.
			anicliUpdate = null;
		}
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
		{#if !anicliUpdate}
			<p>{m.diagnostics_anicli_update_never()}</p>
		{:else}
			<dl>
				<dt>{m.diagnostics_anicli_update_status_label()}</dt>
				<dd>
					{#if anicliUpdate.status === 'no_change'}
						{m.diagnostics_anicli_update_status_no_change()}
					{:else if anicliUpdate.status === 'updated'}
						{m.diagnostics_anicli_update_status_updated()}
					{:else}
						{m.diagnostics_anicli_update_status_failed()}
					{/if}
				</dd>
				<dt>{m.diagnostics_anicli_update_finished_at_label()}</dt>
				<dd>{anicliUpdate.finished_at}</dd>
				<dt>{m.diagnostics_anicli_update_duration_label()}</dt>
				<dd>{m.diagnostics_anicli_update_duration_value({ ms: anicliUpdate.duration_ms })}</dd>
				<dt>{m.diagnostics_anicli_update_stdout_label()}</dt>
				<dd>
					<pre>{anicliUpdate.stdout || m.diagnostics_anicli_update_empty_output()}</pre>
				</dd>
				{#if anicliUpdate.stderr}
					<dt>{m.diagnostics_anicli_update_stderr_label()}</dt>
					<dd>
						<pre>{anicliUpdate.stderr}</pre>
					</dd>
				{/if}
			</dl>
		{/if}
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
</style>
