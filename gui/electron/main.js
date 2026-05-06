// Electron main process for ani-gui.
//
// Responsibilities:
//   1. Spawn the Rust sidecar (`ani-gui-backend`) and parse its stdout
//      to learn the localhost port it bound to.
//   2. Create a BrowserWindow whose preload script injects that URL
//      into `window.aniGui.apiBase` so the SvelteKit renderer can
//      `fetch()` against it.
//   3. Forward app lifecycle events to the sidecar — kill the
//      backend when the window closes so we don't leak a process.
//
// In dev (ELECTRON_DEV=1), points the BrowserWindow at the Vite dev
// server (default http://localhost:5173). In packaged builds, loads
// the static SvelteKit bundle from disk. M-E4 wires the packaged
// path; for M-E3 the dev path is enough to verify the wiring.

'use strict';

const { app, BrowserWindow, net, protocol, shell } = require('electron');
const { spawn } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');
const { pathToFileURL } = require('node:url');

const IS_DEV = process.env.ELECTRON_DEV === '1';
const VITE_DEV_URL = process.env.VITE_DEV_URL || 'http://localhost:5173';

// Pin the X11 WM_CLASS / Wayland app_id so GNOME matches the running
// window to our `.desktop` entry's `StartupWMClass=ani-gui`. Without
// this, the dock falls back to a generic icon — the .desktop's
// `Icon=` line is only used in the app grid, not for live-window
// matching. Must be set before app.whenReady().
app.setName('ani-gui');
process.title = 'ani-gui';

// Custom scheme used in packaged builds to serve the SvelteKit
// static bundle. Loading the index.html via plain `file://` works
// for the root document but breaks SvelteKit's chunk graph — the
// runtime does dynamic `import('/_app/foo.js')` against the page's
// origin, which under `file://` resolves to filesystem-root nonsense.
// A custom scheme gives us a real origin we control, so we can
// resolve `/_app/...` against the bundle dir and SPA-fallback any
// other path to index.html.
const APP_SCHEME = 'app';
const APP_ORIGIN = `${APP_SCHEME}://localhost`;

// Register the scheme as standard + secure BEFORE app.whenReady, so
// fetch + service-worker-style guarantees apply to assets loaded
// through it.
protocol.registerSchemesAsPrivileged([
	{
		scheme: APP_SCHEME,
		privileges: {
			standard: true,
			secure: true,
			supportFetchAPI: true,
			corsEnabled: true
		}
	}
]);

/**
 * Locate the compiled Rust backend binary.
 *
 * In dev we look in the cargo target dir (release first, then debug).
 * In packaged builds electron-builder copies the binary into
 * `process.resourcesPath/ani-gui-backend` via `extraResources` in
 * `package.json:build`. Throws with a clear message if missing.
 */
function resolveBackendBinary() {
	if (IS_DEV) {
		const repoRoot = path.resolve(__dirname, '..', '..');
		const candidates = [
			path.join(repoRoot, 'gui', 'backend', 'target', 'release', 'ani-gui-backend'),
			path.join(repoRoot, 'gui', 'backend', 'target', 'debug', 'ani-gui-backend')
		];
		for (const p of candidates) {
			if (fs.existsSync(p)) return p;
		}
		throw new Error(
			`ani-gui-backend not found. Build it first:\n  ` +
				`cd gui/backend && cargo build --bin ani-gui-backend`
		);
	}
	const packaged = path.join(process.resourcesPath, 'ani-gui-backend');
	if (!fs.existsSync(packaged)) {
		throw new Error(
			`ani-gui-backend not found in packaged resources at ${packaged}. ` +
				`The electron-builder \`extraResources\` rule may be misconfigured.`
		);
	}
	return packaged;
}

/**
 * Spawn the backend and resolve once it prints its listening URL.
 * Rejects if the process exits before the URL is observed (so the
 * Electron main process doesn't sit indefinitely on a broken sidecar).
 */
function spawnBackend() {
	return new Promise((resolve, reject) => {
		const bin = resolveBackendBinary();
		const child = spawn(bin, [], { stdio: ['ignore', 'pipe', 'pipe'] });
		let buf = '';
		let resolved = false;

		const onLine = (line) => {
			if (resolved) {
				// After handshake, downstream stdout becomes log output;
				// just echo it through so we can see it in dev.
				process.stdout.write(`[backend] ${line}\n`);
				return;
			}
			const match = line.match(/^ANI_GUI_LISTENING\s+(\S+)/);
			if (match) {
				resolved = true;
				resolve({ child, apiBase: match[1] });
			}
		};

		child.stdout.on('data', (chunk) => {
			buf += chunk.toString('utf-8');
			let nl;
			while ((nl = buf.indexOf('\n')) >= 0) {
				const line = buf.slice(0, nl);
				buf = buf.slice(nl + 1);
				onLine(line);
			}
		});
		child.stderr.on('data', (chunk) => {
			process.stderr.write(`[backend] ${chunk}`);
		});
		child.on('exit', (code, signal) => {
			if (!resolved) {
				reject(new Error(`backend exited before handshake (code=${code}, signal=${signal})`));
			} else {
				console.error(`[backend] exited (code=${code}, signal=${signal})`);
			}
		});
	});
}

let backendChild = null;

async function createWindow(apiBase) {
	const win = new BrowserWindow({
		width: 1280,
		height: 800,
		minWidth: 960,
		minHeight: 600,
		// Window icon for the dev session — packaged builds get it
		// from the .desktop file electron-builder generates from
		// build-resources/icon.png. Without this, GNOME's window list
		// shows the generic Electron logo while running unpackaged.
		icon: path.join(__dirname, 'build-resources', 'icon.png'),
		// Frame/decorations match the Tauri config (system decorations).
		webPreferences: {
			preload: path.join(__dirname, 'preload.js'),
			contextIsolation: true,
			nodeIntegration: false,
			// Pass the resolved apiBase into the preload via additional
			// arguments — the preload reads them off process.argv.
			additionalArguments: [`--ani-gui-api-base=${apiBase}`]
		}
	});

	// Open external links (http/https) in the user's default browser
	// instead of inside the app window.
	win.webContents.setWindowOpenHandler(({ url }) => {
		if (url.startsWith('http')) {
			shell.openExternal(url);
			return { action: 'deny' };
		}
		return { action: 'allow' };
	});

	// Renderer-side diagnostics surface in the main process log so we
	// can tell whether the page actually loaded and whether scripts
	// are throwing. Without this, a blank window looks identical to a
	// successful load from the outside.
	win.webContents.on('did-fail-load', (_e, code, desc, url) => {
		console.error(`[renderer] did-fail-load ${url}: ${code} ${desc}`);
	});
	win.webContents.on('console-message', (_e, level, msg, line, source) => {
		const tag = ['log', 'warning', 'error'][level] || 'log';
		console.log(`[renderer:${tag}] ${msg} (${source}:${line})`);
	});

	if (IS_DEV) {
		win.webContents.openDevTools({ mode: 'detach' });
		await win.loadURL(VITE_DEV_URL);
	} else {
		// Packaged static SvelteKit bundle, served via the custom
		// `app://` scheme registered above. The bundle's chunks do
		// dynamic imports off the page's origin, so we need a real
		// origin (not `file://`) for them to resolve.
		//
		// Load the root path, not `/index.html` — SvelteKit's client
		// router reads `location.pathname` and treats `/index.html`
		// as a non-route (the app has no `routes/index.html` page).
		// The protocol handler maps `/` to the index.html file.
		await win.loadURL(`${APP_ORIGIN}/`);
	}
	return win;
}

/**
 * Wire `app://localhost/...` to the packaged SvelteKit bundle.
 *
 * Resolution rules:
 *   - `/_app/...` and other extensioned paths → file in the bundle.
 *   - any extensionless path → `index.html` (SvelteKit SPA fallback).
 *   - 404 if the resolved path escapes the bundle dir (defence in
 *     depth — the URL parser shouldn't permit `..` traversal, but
 *     it's cheap to check).
 */
function registerAppProtocol() {
	const bundleDir = path.join(process.resourcesPath, 'frontend', 'build');
	protocol.handle(APP_SCHEME, async (request) => {
		const url = new URL(request.url);
		let pathname = decodeURIComponent(url.pathname);
		if (!pathname || pathname === '/') pathname = '/index.html';
		const target = path.normalize(path.join(bundleDir, pathname));
		if (!target.startsWith(bundleDir)) {
			return new Response('forbidden', { status: 403 });
		}
		try {
			await fs.promises.access(target);
			return net.fetch(pathToFileURL(target).toString());
		} catch {
			// SPA fallback — anything routerly (no file extension)
			// hands back the index so SvelteKit's client router takes
			// over. A real missing asset (image, css) returns 404.
			if (!path.extname(pathname)) {
				return net.fetch(pathToFileURL(path.join(bundleDir, 'index.html')).toString());
			}
			return new Response('not found', { status: 404 });
		}
	});
}

app.whenReady().then(async () => {
	try {
		if (!IS_DEV) registerAppProtocol();
		const { child, apiBase } = await spawnBackend();
		backendChild = child;
		await createWindow(apiBase);
	} catch (err) {
		console.error('[main] startup failed:', err);
		app.exit(1);
	}
});

app.on('window-all-closed', () => {
	if (process.platform !== 'darwin') app.quit();
});

app.on('before-quit', () => {
	if (backendChild && !backendChild.killed) backendChild.kill('SIGTERM');
});

// Re-create a window if the user clicks the dock icon on macOS while
// the app is still running.
app.on('activate', async () => {
	if (BrowserWindow.getAllWindows().length === 0 && backendChild) {
		// Re-derive apiBase from the running backend's known origin.
		// In practice we'd persist this from spawnBackend(); for now,
		// rely on the user to relaunch.
	}
});
