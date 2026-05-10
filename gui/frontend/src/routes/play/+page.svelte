<!--
  /play — manual-paste test stream. Diagnostic utility for verifying
  the streaming proxy + hls.js wiring end-to-end without needing a
  full search → episode → session round-trip.

  Paste a public HLS (.m3u8) or MP4 URL; the page attaches hls.js
  for HLS, uses native <video> src for MP4. Errors surface inline.
-->
<script lang="ts">
	import { onDestroy } from 'svelte';
	import Hls from 'hls.js';
	import { m } from '$lib/paraglide/messages';

	let url = $state('');
	let videoEl = $state<HTMLVideoElement | null>(null);
	let hls = $state<Hls | null>(null);
	let error = $state<string | null>(null);

	function teardown() {
		if (hls) {
			hls.destroy();
			hls = null;
		}
		if (videoEl) videoEl.removeAttribute('src');
	}

	function play() {
		error = null;
		teardown();
		if (!videoEl || !url.trim()) return;
		const target = url.trim();
		const isHls = /\.m3u8(\?|$)/i.test(target);
		try {
			if (isHls && Hls.isSupported()) {
				const h = new Hls();
				h.attachMedia(videoEl);
				h.loadSource(target);
				h.on(Hls.Events.ERROR, (_ev, data) => {
					if (data.fatal) {
						error = `HLS fatal: ${data.type} / ${data.details}`;
					}
				});
				hls = h;
			} else {
				videoEl.src = target;
			}
			void videoEl.play().catch((e) => {
				error = `Play failed: ${(e as Error).message}`;
			});
		} catch (e) {
			error = `Setup failed: ${(e as Error).message}`;
		}
	}

	function onSubmit(e: Event) {
		e.preventDefault();
		play();
	}

	onDestroy(teardown);
</script>

<svelte:head>
	<title>{m.play_test_title()} · {m.app_brand_title()}</title>
</svelte:head>

<main class="page">
	<header>
		<p class="eyebrow">{m.play_test_section_title()}</p>
		<h1>{m.play_test_title()}</h1>
		<p class="hint">
			{m.play_test_hint()}
		</p>
	</header>

	<form onsubmit={onSubmit} class="row">
		<input
			type="text"
			bind:value={url}
			placeholder={m.play_test_url_placeholder()}
			autocomplete="off"
			spellcheck="false"
			aria-label={m.play_test_label()}
		/>
		<button type="submit" disabled={!url.trim()}>{m.play_test_submit_button()}</button>
	</form>

	{#if error}
		<p class="error" role="alert">{error}</p>
	{/if}

	<video bind:this={videoEl} controls></video>
</main>

<style>
	.page {
		max-inline-size: 64rem;
		margin-inline: auto;
		padding: var(--space-7) var(--space-6) var(--space-8);
		display: flex;
		flex-direction: column;
		gap: var(--space-5);
	}
	.eyebrow {
		margin: 0;
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	h1 {
		margin: var(--space-2) 0 0;
		font-family: var(--font-display);
		font-size: var(--type-h2);
		color: var(--bone-100);
	}
	.hint {
		margin: var(--space-3) 0 0;
		color: var(--bone-300);
		font-size: var(--type-body-s);
	}
	/* The `<code>` formerly inlined in the hint copy now lives inside
	   the localized `play_test_hint` message, so Svelte's CSS
	   scoping no longer sees it during static analysis. Keep the
	   rule but qualify with :global so it still applies. */
	.hint :global(code) {
		font-family: var(--font-mono);
		color: var(--bone-200);
	}
	.row {
		display: flex;
		gap: var(--space-2);
	}
	input {
		flex: 1 1 auto;
		min-inline-size: 0;
		padding: var(--space-2) var(--space-3);
		background: var(--ink-050);
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-sm);
		color: var(--bone-100);
		font-family: var(--font-mono);
		font-size: var(--type-body-s);
	}
	input:focus {
		outline: none;
		border-color: var(--accent);
	}
	button {
		padding: var(--space-2) var(--space-4);
		background: var(--accent);
		border: 1px solid var(--accent);
		border-radius: var(--radius-sm);
		color: var(--ink-000);
		font-family: var(--font-body);
		font-size: var(--type-body-s);
		font-weight: 600;
		cursor: pointer;
	}
	button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
	.error {
		margin: 0;
		color: var(--oxblood, #b11a1a);
		font-family: var(--font-mono);
		font-size: var(--type-meta);
	}
	video {
		inline-size: 100%;
		aspect-ratio: 16 / 9;
		background: #000;
		border-radius: var(--radius-sm);
	}
</style>
