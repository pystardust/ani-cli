<!--
  Breadcrumb trail — Home › Anime › Episode style.
  Reads from $lib/breadcrumb (a writable store) so each route
  contributes its own segments after data loads. The leading chevron
  preserves the back-affordance affordance the standalone BackButton
  used to provide; clicking it takes the user one hop up the trail
  (the second-to-last segment, if any).

  Visual: pill-shaped container, chevron + segments separated by a
  middot (·) for compact spacing. Last segment is plain text in
  bone-100 weight 600 — it's the "you are here" cue.
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import { goto } from '$app/navigation';
	import type { BreadcrumbSegment } from '$lib/breadcrumb';
	import { m } from '$lib/paraglide/messages';

	let { segments }: { segments: BreadcrumbSegment[] } = $props();

	// The chevron walks "up" one level — to the second-to-last
	// segment. When there's only one segment (the current page), it
	// links home as a safe default.
	const upHref = $derived<string>(
		segments.length >= 2 ? (segments[segments.length - 2].href ?? '/') : '/'
	);

	function goUp(e: MouseEvent) {
		e.preventDefault();
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		void goto(upHref);
	}
</script>

{#if segments.length > 0}
	<nav class="crumbs" aria-label="Breadcrumb">
		<!-- Chevron walks the trail "up" one level. href is the parent
		     segment's URL — already an app-relative path (resolve()-d
		     by the contributing route), so the no-resolve rule's
		     pattern matcher trips on the dynamic value. -->
		<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
		<a class="up" href={upHref} onclick={goUp} aria-label={m.breadcrumb_up_aria_label()}>
			<svg viewBox="0 0 16 16" aria-hidden="true">
				<path
					d="M10.5 3.5 6 8l4.5 4.5"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
		</a>
		<ol>
			{#each segments as seg, i (i)}
				{#if i > 0}
					<li class="sep" aria-hidden="true">·</li>
				{/if}
				<li class:current={!seg.href}>
					{#if seg.href}
						<!-- href is `/` literal or `/anime/[id]` etc; the no-resolve
						     rule pattern-matches a literal `resolve(...)` and trips
						     on the conditional, so wrap with a disable. -->
						<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
						<a href={seg.href === '/' ? resolve('/') : seg.href}>{seg.label}</a>
					{:else}
						<span aria-current="page">{seg.label}</span>
					{/if}
				</li>
			{/each}
		</ol>
	</nav>
{/if}

<style>
	.crumbs {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		padding: 0.4rem 0.6rem 0.4rem 0.4rem;
		margin-inline-start: calc(-1 * var(--space-2));
		border-radius: var(--radius-pill);
	}
	.up {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		inline-size: 1.6rem;
		block-size: 1.6rem;
		color: var(--bone-300);
		border-radius: var(--radius-pill);
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft);
	}
	.up:hover {
		color: var(--bone-100);
		background: color-mix(in oklab, var(--ink-100) 80%, transparent);
	}
	.up svg {
		inline-size: 16px;
		block-size: 16px;
	}
	ol {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		list-style: none;
		margin: 0;
		padding: 0;
		font-family: var(--font-mono);
		/* Bumped from --type-meta (12px) to --type-body-s (14px) — the
		   prior size felt swallowed by the topbar's other elements. */
		font-size: var(--type-body-s);
		letter-spacing: var(--tracking-meta);
		min-inline-size: 0;
	}
	li {
		display: inline-flex;
		align-items: center;
		min-inline-size: 0;
	}
	li.sep {
		color: var(--bone-400);
		font-weight: 400;
	}
	/* Ellipsis applies to the inline text node, not the flex `<li>`
	   wrapper — `text-overflow` is silently a no-op on flex
	   containers. inline-block + a max width gives the browser the
	   block context it needs to clamp. 28ch fits "Naruto:
	   Shippuuden" verbatim and softly trims anything longer
	   ("JoJo no Kimyou na Bouken Part 6: Stone Ocean" → "JoJo no
	   Kimyou na Bouken Part 6:…"). */
	li a,
	li.current span {
		display: inline-block;
		max-inline-size: 28ch;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		vertical-align: bottom;
	}
	li a {
		color: var(--bone-300);
		font-weight: 500;
		text-decoration: none;
		transition: color var(--dur-fast) var(--ease-out-soft);
	}
	li a:hover {
		color: var(--bone-100);
	}
	li.current span {
		color: var(--bone-100);
		font-weight: 600;
	}
	/* Tighter cap on narrow viewports — breadcrumbs share row with
	   the search pill which gets the dominant width allocation. */
	@media (max-inline-size: 720px) {
		li a,
		li.current span {
			max-inline-size: 16ch;
		}
	}
</style>
