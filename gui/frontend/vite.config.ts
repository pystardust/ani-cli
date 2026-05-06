import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';

export default defineConfig({
	plugins: [sveltekit()],
	// Tauri dev server expects a fixed port (matches tauri.conf.json devUrl).
	server: {
		port: 5173,
		strictPort: true,
		host: '127.0.0.1'
	},
	// Vitest config — node env, no DOM needed for IPC wrapper tests.
	// Component tests would add a separate workspace later.
	test: {
		environment: 'node',
		include: ['src/**/*.{test,spec}.ts']
	}
});
