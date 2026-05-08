<!--
  PosterCard — card used inside horizontal strips on the home page. Smaller
  cousin of the /search grid card. 5:7 poster, scroll-snap-aligned, accent
  rule at the bottom on focus/hover.
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import { imageProxyUrl, type KitsuAnimeRef } from '$lib/api';
	import { accentFor } from '$lib/design/accent';

	interface Props {
		anime: KitsuAnimeRef;
	}
	let { anime }: Props = $props();

	const accent = $derived(accentFor(anime.id));
	const poster = $derived(
		imageProxyUrl(
			// `original` last as defense — for recently-aired sequels
			// Kitsu only returns posterImage.original (a Backblaze S3
			// signed URL). The backend's eager-warm fetches those bytes
			// at cache-write time and stores them under a canonical
			// (signature-stripped) hash, so the image proxy serves the
			// cached bytes regardless of whether the embedded URL has
			// gone stale.
			anime.poster_image?.medium ??
				anime.poster_image?.large ??
				anime.poster_image?.small ??
				anime.poster_image?.original ??
				null
		)
	);
</script>

<a class="poster-card" style="--accent: {accent};" href={resolve('/anime/[id]', { id: anime.id })}>
	<span class="poster">
		{#if poster}
			<img src={poster} alt="" loading="lazy" decoding="async" />
		{:else}
			<span class="poster-placeholder" aria-hidden="true">
				<span class="poster-placeholder-title">{anime.canonical_title}</span>
			</span>
		{/if}
		<span class="accent-rule" aria-hidden="true"></span>
	</span>
	<span class="card-body">
		<span class="card-title">{anime.canonical_title}</span>
		<span class="card-meta">
			{#if anime.episode_count}
				<span class="num">{anime.episode_count}</span>
				<span class="card-meta-word">ep</span>
			{:else if anime.start_date}
				<span class="num">{anime.start_date.slice(0, 4)}</span>
			{:else}
				<span class="card-meta-word">—</span>
			{/if}
		</span>
	</span>
</a>

<style>
	.poster-card {
		scroll-snap-align: start;
		display: block;
		color: inherit;
		transition: transform var(--dur-med) var(--ease-out-elastic);
		will-change: transform;
	}
	.poster-card:hover {
		transform: translateY(-4px);
	}

	.poster {
		position: relative;
		display: block;
		aspect-ratio: var(--poster-aspect);
		background: var(--ink-100);
		border-radius: var(--radius-card);
		overflow: hidden;
		box-shadow: var(--shadow-card-rest);
		transition: box-shadow var(--dur-med) var(--ease-out-soft);
	}
	.poster-card:hover .poster {
		box-shadow: var(--shadow-card-hover);
	}
	.poster img {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
		transition: transform var(--dur-slow) var(--ease-out-soft);
	}
	.poster-card:hover .poster img {
		transform: scale(1.04);
	}

	.poster-placeholder {
		position: absolute;
		inset: 0;
		display: grid;
		place-items: center;
		padding: var(--space-3);
		background: linear-gradient(180deg, var(--ink-100) 0%, var(--ink-050) 100%);
	}
	.poster-placeholder-title {
		font-family: var(--font-body);
		font-weight: 600;
		font-size: var(--type-body-l);
		text-align: center;
		color: var(--bone-200);
		line-height: var(--leading-tight);
		overflow: hidden;
		display: -webkit-box;
		-webkit-line-clamp: 4;
		line-clamp: 4;
		-webkit-box-orient: vertical;
	}

	.accent-rule {
		position: absolute;
		inset-inline: 0;
		inset-block-end: 0;
		block-size: 2px;
		background: var(--accent);
		transform: scaleX(0);
		transform-origin: inline-start;
		transition: transform var(--dur-med) var(--ease-out-soft);
	}
	.poster-card:hover .accent-rule,
	.poster-card:focus-visible .accent-rule {
		transform: scaleX(1);
	}

	.poster-card:focus-visible {
		outline: none;
	}
	.poster-card:focus-visible .poster {
		box-shadow:
			var(--shadow-card-rest),
			0 0 0 2px var(--bone-100);
	}

	.card-body {
		display: block;
		padding-block-start: var(--space-3);
	}
	.card-title {
		display: block;
		font-family: var(--font-body);
		font-size: 0.9375rem; /* 15px */
		font-weight: 500;
		line-height: 1.3;
		color: var(--bone-100);
		overflow: hidden;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
	}
	.card-meta {
		display: inline-flex;
		align-items: baseline;
		gap: var(--space-2);
		margin-block-start: var(--space-2);
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: var(--type-meta);
		color: var(--bone-300);
		letter-spacing: var(--tracking-meta);
	}
	.card-meta .num {
		color: var(--bone-100);
	}
	.card-meta-word {
		text-transform: uppercase;
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
	}
</style>
