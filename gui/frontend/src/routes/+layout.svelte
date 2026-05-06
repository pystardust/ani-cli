<!--
  Minimal layout. Intentionally unstyled: M1.5 is wiring-validation only;
  the design pass (M3) will define typography, color, motion, and layout.
-->
<script lang="ts">
	let { children } = $props();
</script>

<main>
	{@render children()}
</main>
