<!--
  Icon — small inline SVG glyphs hand-drawn to match the M3 hairline
  aesthetic (1.5px stroke, square line-caps, no fill except the play
  triangle which is solid by convention). Stroke uses currentColor so
  the icon picks up surrounding text color naturally.

  Deliberately NOT a third-party set: lucide / phosphor / heroicons
  default-style icons read as "generic startup" — exactly the AI tell
  we ban in AGENTS.md §7. These shapes are bespoke + restrained:
  registration-tick on the home roof, slider-with-knobs settings,
  terminal-prompt debug. Universal silhouettes, our line discipline.
-->
<script lang="ts">
	type IconName =
		| 'home'
		| 'search'
		| 'settings'
		| 'debug'
		| 'play'
		| 'download'
		| 'external'
		| 'arrow-back'
		| 'arrow-right';

	let { name, size = 18 }: { name: IconName; size?: number } = $props();
</script>

{#if name === 'home'}
	<svg
		viewBox="0 0 24 24"
		width={size}
		height={size}
		fill="none"
		stroke="currentColor"
		stroke-width="1.5"
		stroke-linecap="square"
		stroke-linejoin="miter"
		aria-hidden="true"
	>
		<path d="M3 11 L12 4 L21 11 V20 H3 Z" />
		<path d="M9 20 V14 H15 V20" />
		<!-- Registration tick on the eave — echoes the brand mark. -->
		<path d="M5.5 11.5 H7.5 V13" />
	</svg>
{:else if name === 'search'}
	<svg
		viewBox="0 0 24 24"
		width={size}
		height={size}
		fill="none"
		stroke="currentColor"
		stroke-width="1.5"
		stroke-linecap="square"
		aria-hidden="true"
	>
		<circle cx="10.5" cy="10.5" r="6" />
		<path d="M14.8 14.8 L20 20" />
	</svg>
{:else if name === 'settings'}
	<svg
		viewBox="0 0 24 24"
		width={size}
		height={size}
		fill="none"
		stroke="currentColor"
		stroke-width="1.5"
		stroke-linecap="square"
		aria-hidden="true"
	>
		<!-- Three slider rules with knobs at varying positions. -->
		<path d="M3 7 H21" />
		<path d="M3 12 H21" />
		<path d="M3 17 H21" />
		<rect x="6" y="5.5" width="3" height="3" fill="currentColor" stroke="none" />
		<rect x="13" y="10.5" width="3" height="3" fill="currentColor" stroke="none" />
		<rect x="9" y="15.5" width="3" height="3" fill="currentColor" stroke="none" />
	</svg>
{:else if name === 'debug'}
	<svg
		viewBox="0 0 24 24"
		width={size}
		height={size}
		fill="none"
		stroke="currentColor"
		stroke-width="1.5"
		stroke-linecap="square"
		stroke-linejoin="miter"
		aria-hidden="true"
	>
		<!-- Terminal prompt: chevron + underscore. -->
		<path d="M5 7 L11 12 L5 17" />
		<path d="M13 17 H19" />
	</svg>
{:else if name === 'play'}
	<svg viewBox="0 0 24 24" width={size} height={size} fill="none" aria-hidden="true">
		<!-- Solid by convention — play triangles read as outlines poorly. -->
		<path d="M7 5 L19 12 L7 19 Z" fill="currentColor" />
	</svg>
{:else if name === 'download'}
	<svg
		viewBox="0 0 24 24"
		width={size}
		height={size}
		fill="none"
		stroke="currentColor"
		stroke-width="1.5"
		stroke-linecap="square"
		stroke-linejoin="miter"
		aria-hidden="true"
	>
		<path d="M12 4 V15" />
		<path d="M7 11 L12 16 L17 11" />
		<path d="M5 20 H19" />
	</svg>
{:else if name === 'external'}
	<svg
		viewBox="0 0 24 24"
		width={size}
		height={size}
		fill="none"
		stroke="currentColor"
		stroke-width="1.5"
		stroke-linecap="square"
		stroke-linejoin="miter"
		aria-hidden="true"
	>
		<!-- Box pointing diagonally outwards: opens elsewhere. -->
		<path d="M5 5 H11 V19 H19 V13" />
		<path d="M14 5 H19 V10" />
		<path d="M19 5 L13 11" />
	</svg>
{:else if name === 'arrow-back'}
	<svg
		viewBox="0 0 24 24"
		width={size}
		height={size}
		fill="none"
		stroke="currentColor"
		stroke-width="1.5"
		stroke-linecap="square"
		stroke-linejoin="miter"
		aria-hidden="true"
	>
		<path d="M11 5 L4 12 L11 19" />
		<path d="M4 12 H20" />
	</svg>
{:else if name === 'arrow-right'}
	<svg
		viewBox="0 0 24 24"
		width={size}
		height={size}
		fill="none"
		stroke="currentColor"
		stroke-width="1.5"
		stroke-linecap="square"
		stroke-linejoin="miter"
		aria-hidden="true"
	>
		<path d="M13 5 L20 12 L13 19" />
		<path d="M4 12 H20" />
	</svg>
{/if}
