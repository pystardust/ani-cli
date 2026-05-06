<!--
  Kitsu search — utilitarian. Plain form + plain HTML grid.
  Visual design lands in M3 (frontend-design skill, sub-agent reviewed).
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import { imageProxyUrl, kitsuSearch, type KitsuAnimeRef } from '$lib/api';

	let query = $state('');
	let results = $state<KitsuAnimeRef[] | null>(null);
	let error = $state<string | null>(null);
	let busy = $state(false);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		const q = query.trim();
		if (!q) return;
		error = null;
		busy = true;
		try {
			results = await kitsuSearch(q);
		} catch (e) {
			error = describeError(e);
			results = null;
		} finally {
			busy = false;
		}
	}

	function describeError(e: unknown): string {
		if (typeof e === 'object' && e !== null) {
			const obj = e as Record<string, unknown>;
			const kind = typeof obj.kind === 'string' ? obj.kind : null;
			const detail = typeof obj.detail === 'string' ? obj.detail : null;
			if (kind && detail) return `${kind}: ${detail}`;
			if (kind) return kind;
		}
		return String(e);
	}

	function posterFor(hit: KitsuAnimeRef): string | null {
		const url = hit.poster_image?.large ?? hit.poster_image?.medium ?? hit.poster_image?.small;
		return imageProxyUrl(url);
	}

	function ratingDisplay(r: number | null): string {
		if (r === null) return '—';
		return (r / 10).toFixed(1);
	}
</script>

<h1>Search Kitsu</h1>

<p><a href={resolve('/')}>Back to Backend Status</a></p>

<form onsubmit={submit}>
	<label>
		Title
		<input type="search" bind:value={query} required size={40} placeholder="e.g. one piece" />
	</label>
	<button type="submit" disabled={busy || query.trim().length === 0}>
		{busy ? 'Searching…' : 'Search'}
	</button>
</form>

{#if error}
	<p>Error: {error}</p>
{/if}

{#if results !== null}
	{#if results.length === 0}
		<p>No results.</p>
	{:else}
		<p>{results.length} result{results.length === 1 ? '' : 's'}</p>
		<ul>
			{#each results as hit (hit.id)}
				{@const poster = posterFor(hit)}
				<li>
					<a href={resolve('/anime/[id]', { id: hit.id })}>
						{#if poster}
							<img src={poster} alt="" width="80" height="112" loading="lazy" />
						{/if}
						<strong>{hit.canonical_title}</strong>
					</a>
					<span>
						— {hit.subtype ?? '?'}
						· {hit.status ?? '?'}
						· rating {ratingDisplay(hit.average_rating)} / 10
						{#if hit.episode_count}· {hit.episode_count} eps{/if}
					</span>
				</li>
			{/each}
		</ul>
	{/if}
{/if}
