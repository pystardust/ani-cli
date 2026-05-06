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

const { app, BrowserWindow, shell } = require('electron');
const { spawn } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');

const IS_DEV = process.env.ELECTRON_DEV === '1';
const VITE_DEV_URL = process.env.VITE_DEV_URL || 'http://localhost:5173';

/**
 * Locate the compiled Rust backend binary. In dev we point at the
 * cargo target/debug build; packaged builds will resolve via
 * `process.resourcesPath` (M-E4 wires that). Throws with a clear
 * message if the binary isn't where we expect.
 */
function resolveBackendBinary() {
	if (IS_DEV) {
		const repoRoot = path.resolve(__dirname, '..', '..');
		const candidates = [
			path.join(repoRoot, 'gui', 'src-tauri', 'target', 'release', 'ani-gui-backend'),
			path.join(repoRoot, 'gui', 'src-tauri', 'target', 'debug', 'ani-gui-backend')
		];
		for (const p of candidates) {
			if (fs.existsSync(p)) return p;
		}
		throw new Error(
			`ani-gui-backend not found. Build it first:\n  ` +
				`cd gui/src-tauri && cargo build --bin ani-gui-backend`
		);
	}
	// Packaged path comes in M-E4. Resources dir.
	return path.join(process.resourcesPath, 'ani-gui-backend');
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

	if (IS_DEV) {
		await win.loadURL(VITE_DEV_URL);
	} else {
		// Packaged static SvelteKit bundle. M-E4 wires the path.
		const indexHtml = path.join(__dirname, '..', 'frontend', 'build', 'index.html');
		await win.loadFile(indexHtml);
	}
	return win;
}

app.whenReady().then(async () => {
	try {
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
