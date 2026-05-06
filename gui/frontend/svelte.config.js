import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),
	kit: {
		// SPA mode: every route falls back to index.html and SvelteKit's
		// client-side router takes over. Tauri's webview serves the
		// prerendered index.html out of `frontendDist`.
		adapter: adapter({
			fallback: 'index.html',
			pages: 'build',
			assets: 'build',
			strict: false
		}),
		alias: {
			$lib: 'src/lib'
		}
	}
};

export default config;
