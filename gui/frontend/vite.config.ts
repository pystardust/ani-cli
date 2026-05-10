import { paraglideVitePlugin } from '@inlang/paraglide-js';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';

export default defineConfig({
	plugins: [
		// Paraglide.js — compiles `messages/<locale>.json` into typed
		// TS message helpers under `src/lib/paraglide/`. Watching the
		// project + bundle dirs means edits to either get picked up
		// without a manual `paraglide-js compile` step.
		paraglideVitePlugin({
			project: './project.inlang',
			outdir: './src/lib/paraglide',
			strategy: ['localStorage', 'preferredLanguage', 'baseLocale']
		}),
		sveltekit()
	],
	// Electron's main.js loads VITE_DEV_URL || http://localhost:5173 in dev,
	// so this port has to be deterministic.
	server: {
		port: 5173,
		strictPort: true,
		host: '127.0.0.1'
	},
	// Vitest config — node env, no DOM needed for IPC wrapper tests.
	// Component tests would add a separate workspace later.
	test: {
		environment: 'node',
		include: ['src/**/*.{test,spec}.ts'],
		coverage: {
			provider: 'v8',
			reporter: ['text', 'json-summary', 'lcov'],
			reportsDirectory: 'coverage',
			// Scope: pure-TS modules under `src/lib/`. Svelte
			// components and route files are out of scope until
			// component testing lands (separate test runner setup,
			// own coverage bar). Holding 0% components against a
			// baseline would just be theater.
			include: ['src/lib/**/*.ts'],
			exclude: ['src/lib/**/*.{test,spec}.ts', 'src/lib/**/*.d.ts']
		}
	}
});
