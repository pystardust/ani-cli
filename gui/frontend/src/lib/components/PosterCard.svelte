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
			anime.poster_image?.medium ?? anime.poster_image?.large ?? anime.poster_image?.small ?? null
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
		/* contain: paint scopes paint invalidations to this card's
		   box so a hover doesn't cascade out and force the parent
		   strip to repaint. Combined with .content's containment
		   one level up, paints are now scoped to individual cards
		   instead of strips. */
		contain: layout paint style;
		/* No transform transition. On webkit2gtk without an
		   always-on compositor layer (which we removed for memory
		   reasons), animated transforms go through paint, and 40+
		   cards × hover-in/hover-out cycles flooded the main
		   thread. Instant hover state-change keeps the lift cue
		   without the per-frame paint cost. */
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
		/* box-shadow transition removed: every hover→hover-out cycle
		   triggered a paint cascade across the visible card area.
		   With ~20 cards per strip × multiple strips, mouse movement
		   over the page was producing 60–70 ms paint events per
		   frame on a maximized window. The hover lift via
		   `transform` already gives the "card is interactive" cue;
		   the shadow stays at its rest value. */
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
		font-family: var(--font-display);
		font-style: italic;
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
		font-family: var(--font-display);
		font-size: var(--type-body);
		line-height: var(--leading-tight);
		letter-spacing: var(--tracking-display);
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
