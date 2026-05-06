<!--
  Test Stream — paste an upstream HLS URL + referer (+ optional subtitle),
  hand off to the local proxy via cmd_create_session, play through hls.js.
  Validates the proxy + token + manifest-rewrite + browser playback chain.

  Plain HTML on purpose. M3 design pass owns visual treatment.
-->
<script lang="ts">
	import { onDestroy } from 'svelte';
	import Hls from 'hls.js';
	import { resolve } from '$app/paths';
	import { createSession, type CreateSessionResponse } from '$lib/api';

	let upstream = $state('');
	let referer = $state('https://allmanga.to');
	let subtitle = $state('');
	let session = $state<CreateSessionResponse | null>(null);
	let error = $state<string | null>(null);
	let busy = $state(false);
	let videoEl: HTMLVideoElement | undefined = $state();
	let hls: Hls | null = null;

	async function start(event: SubmitEvent) {
		event.preventDefault();
		error = null;
		busy = true;
		try {
			session = await createSession({
				upstream_url: upstream,
				referer,
				subtitle_url: subtitle.trim() || undefined
			});
		} catch (e) {
			error = describeError(e);
		} finally {
			busy = false;
		}
	}

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

	function teardown() {
		if (hls) {
			hls.destroy();
			hls = null;
		}
		if (videoEl) {
			videoEl.removeAttribute('src');
			videoEl.load();
		}
	}

	$effect(() => {
		if (!videoEl || !session) return;
		teardown();
		if (Hls.isSupported()) {
			// debug: true logs every fragment load + buffer event to the
			// devtools console — useful while we shake out webkit2gtk +
			// MPEG-TS seek quirks. Drop this before M3.
			hls = new Hls({ lowLatencyMode: false, debug: true });
			hls.loadSource(session.master_url);
			hls.attachMedia(videoEl);
			hls.on(Hls.Events.ERROR, (_, data) => {
				console.error('[hls error]', data);
				if (data.fatal) {
					error = `hls.js fatal: ${data.type} / ${data.details}`;
				}
			});
		} else if (videoEl.canPlayType('application/vnd.apple.mpegurl')) {
			videoEl.src = session.master_url;
		} else {
			error = 'HLS playback not supported in this webview';
		}
	});

	onDestroy(teardown);
</script>

<h1>Test Stream</h1>

<p><a href={resolve('/')}>Back to Backend Status</a></p>

<form onsubmit={start}>
	<p>
		<label>
			Upstream master.m3u8 URL
			<input type="url" required bind:value={upstream} placeholder="https://… .m3u8" size={70} />
		</label>
	</p>
	<p>
		<label>
			Referer header
			<input type="text" bind:value={referer} size={70} />
		</label>
	</p>
	<p>
		<label>
			Subtitle .vtt URL (optional)
			<input type="url" bind:value={subtitle} size={70} />
		</label>
	</p>
	<p>
		<button type="submit" disabled={busy}>Create session and play</button>
	</p>
</form>

{#if error}
	<p>Error: {error}</p>
{/if}

{#if session}
	<section>
		<h2>Session</h2>
		<dl>
			<dt>Session ID</dt>
			<dd>{session.session_id}</dd>
			<dt>Proxy master URL</dt>
			<dd><code>{session.master_url}</code></dd>
			{#if session.subtitle_url}
				<dt>Proxy subtitle URL</dt>
				<dd><code>{session.subtitle_url}</code></dd>
			{/if}
		</dl>

		<video bind:this={videoEl} controls width="800">
			{#if session.subtitle_url}
				<track kind="subtitles" src={session.subtitle_url} default label="captions" />
			{/if}
		</video>
	</section>
{/if}
