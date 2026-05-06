<!--
  Home stub. The M3.6 designer agent replaces this with the real
  landing page (hero/trending, Continue Watching, Top Rated, nav menu).
  Keeping a tiny placeholder so build doesn't fail in the meantime.
-->
<script lang="ts">
	import { resolve } from '$app/paths';
</script>

<p style="padding: 2rem; font-family: var(--font-display);">
	Home is being designed in M3.6 — for now: <a href={resolve('/search')}>Search</a> ·
	<a href={resolve('/diagnostics')}>Diagnostics</a>
</p>
