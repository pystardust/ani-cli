import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),
	kit: {
		// SPA mode: every route falls back to index.html and SvelteKit's
		// client-side router takes over. The packaged Electron window
		// loads index.html via `file://`, where absolute paths like
		// `/_app/foo.js` resolve to filesystem-root nonsense — so we
		// emit relative paths (`./_app/foo.js`). Vite dev still resolves
		// them against `localhost:5173`.
		adapter: adapter({
			fallback: 'index.html',
			pages: 'build',
			assets: 'build',
			strict: false
		}),
		paths: {
			relative: true
		},
		alias: {
			$lib: 'src/lib'
		}
	}
};

export default config;
