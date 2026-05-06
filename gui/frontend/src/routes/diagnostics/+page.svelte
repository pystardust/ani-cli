<!--
  Backend Status — proves IPC works end-to-end. Loads app_info +
  history_list on mount, exposes history_clear. Plain HTML on purpose;
  the design pass (M3) will replace this entirely.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { appInfo, historyClear, historyList, type AppInfo, type HistoryEntry } from '$lib/api';
	import BackButton from '$lib/components/BackButton.svelte';

	let info = $state<AppInfo | null>(null);
	let history = $state<HistoryEntry[] | null>(null);
	let infoError = $state<string | null>(null);
	let historyError = $state<string | null>(null);
	let busy = $state(false);

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

	function describeError(e: unknown): string {
		if (typeof e === 'object' && e !== null) {
			const obj = e as Record<string, unknown>;
			if (typeof obj.kind === 'string') return obj.kind;
		}
		return String(e);
	}

	onMount(refresh);
</script>

<BackButton fallback="/" />

<h1>ani-gui — Backend Status</h1>

<section>
	<h2>App info</h2>
	{#if infoError}
		<p>Error: {infoError}</p>
	{:else if info}
		<dl>
			<dt>Version</dt>
			<dd>{info.version}</dd>
			<dt>ani-cli script</dt>
			<dd>{info.ani_cli_path}</dd>
			<dt>History file</dt>
			<dd>{info.history_path}</dd>
			<dt>Proxy base URL</dt>
			<dd>{info.proxy_base_url}</dd>
		</dl>
	{:else}
		<p>Loading…</p>
	{/if}
</section>

<section>
	<h2>Continue Watching ({history?.length ?? '…'})</h2>
	{#if historyError}
		<p>Error: {historyError}</p>
	{:else if history === null}
		<p>Loading…</p>
	{:else if history.length === 0}
		<p>No entries yet. Watch something to populate ani-hsts.</p>
	{:else}
		<ul>
			{#each history as entry (entry.id)}
				<li>{entry.title} — episode {entry.ep_no} (id: {entry.id})</li>
			{/each}
		</ul>
	{/if}
	<p>
		<button type="button" onclick={refresh} disabled={busy}>Refresh</button>
		<button type="button" onclick={clearHistory} disabled={busy || history?.length === 0}>
			Clear history
		</button>
	</p>
</section>

<section>
	<h2>Search Kitsu</h2>
	<p>
		<a href={resolve('/search')}>Open Kitsu search</a> — find anime by title, browse posters and metadata.
	</p>
</section>

<section>
	<h2>Test Stream</h2>
	<p>
		<a href={resolve('/play')}>Open Test Stream</a> — paste a public HLS URL to verify the streaming proxy
		+ hls.js wiring end-to-end.
	</p>
</section>
