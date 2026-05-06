<!--
  Backend Status placeholder. Real wiring lands in M1.5d once the IPC
  wrapper module is in place. This file exists so SvelteKit has a route
  at `/` to render during the M1.5b project init.
-->
<script lang="ts">
	import { resolve } from '$app/paths';
</script>

<h1>ani-gui</h1>
<p>Backend Status page (M1.5d)</p>
<p><a href={resolve('/play')}>Test Stream (M1.5e)</a></p>
