<!--
  Anime detail — utilitarian. Plain HTML hero (cover or blurred-poster
  fallback), synopsis, metadata block, episode-list placeholder.
  Visual design lands in M3.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { imageProxyUrl, kitsuAnimeDetail, type KitsuAnimeRef } from '$lib/api';

	let detail = $state<KitsuAnimeRef | null>(null);
	let error = $state<string | null>(null);

	onMount(async () => {
		const id = page.params.id;
		if (!id) {
			error = 'missing anime id in URL';
			return;
		}
		try {
			detail = await kitsuAnimeDetail(id);
		} catch (e) {
			error = describeError(e);
		}
	});

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

	function bannerFor(d: KitsuAnimeRef): { url: string | null; isCover: boolean } {
		const cover = d.cover_image?.large ?? d.cover_image?.original ?? d.cover_image?.small;
		if (cover) return { url: imageProxyUrl(cover), isCover: true };
		const poster = d.poster_image?.large ?? d.poster_image?.original;
		return { url: imageProxyUrl(poster), isCover: false };
	}

	function ratingDisplay(r: number | null): string {
		if (r === null) return '—';
		return (r / 10).toFixed(1);
	}
</script>

<p><a href={resolve('/search')}>Back to search</a></p>

{#if error}
	<p>Error: {error}</p>
{:else if detail === null}
	<p>Loading…</p>
{:else}
	{@const banner = bannerFor(detail)}

	{#if banner.url}
		<div>
			<img
				src={banner.url}
				alt=""
				width="640"
				height={banner.isCover ? 152 : 320}
				loading="eager"
			/>
		</div>
	{/if}

	<h1>{detail.canonical_title}</h1>

	<dl>
		<dt>Type</dt>
		<dd>{detail.subtype ?? '—'}</dd>
		<dt>Status</dt>
		<dd>{detail.status ?? '—'}</dd>
		<dt>Rating</dt>
		<dd>{ratingDisplay(detail.average_rating)} / 10</dd>
		<dt>Started</dt>
		<dd>{detail.start_date ?? '—'}</dd>
		{#if detail.end_date}
			<dt>Ended</dt>
			<dd>{detail.end_date}</dd>
		{/if}
		{#if detail.episode_count}
			<dt>Episodes</dt>
			<dd>{detail.episode_count}</dd>
		{/if}
		{#if detail.age_rating}
			<dt>Age rating</dt>
			<dd>{detail.age_rating}</dd>
		{/if}
		{#if detail.popularity_rank}
			<dt>Popularity rank</dt>
			<dd>#{detail.popularity_rank}</dd>
		{/if}
	</dl>

	{#if detail.synopsis}
		<h2>Synopsis</h2>
		<p>{detail.synopsis}</p>
	{/if}

	<h2>Episodes</h2>
	<p>
		Episode list + per-episode play wiring lands when M2 connects allanime (via the existing Test
		Stream form for now — paste an upstream HLS URL there).
	</p>
{/if}
