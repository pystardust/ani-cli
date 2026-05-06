<!--
  Root layout — imports the design tokens once so every route shares
  the warm-ink baseline (no white flash on the diagnostic Backend
  Status page, no leakage of styles when navigating back).
-->
<script lang="ts">
	import '$lib/design/tokens.css';

	let { children } = $props();
</script>

<main>
	{@render children()}
</main>
