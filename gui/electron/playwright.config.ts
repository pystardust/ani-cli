/**
 * Playwright e2e for the Electron app.
 *
 * The actual Electron launch happens inside each test via
 * `_electron.launch(...)` from `playwright`'s electron driver — this
 * config just sets defaults for test discovery, retries, and reports.
 *
 * Pre-test setup is the responsibility of a global setup hook (or the
 * `pnpm package` script run beforehand): the Rust backend binary at
 * `../backend/target/release/ani-gui-backend` and the SvelteKit static
 * bundle at `../frontend/build/index.html` must both exist.
 */
import { defineConfig } from '@playwright/test';

export default defineConfig({
	testDir: './e2e',
	timeout: 30_000,
	expect: { timeout: 5_000 },
	fullyParallel: false, // Each test launches its own Electron process; parallelism would race on the loopback port.
	retries: 0,
	reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
	use: {
		actionTimeout: 5_000,
		// Take a screenshot on failure for post-mortem debugging in CI.
		screenshot: 'only-on-failure',
		trace: 'retain-on-failure'
	}
});
