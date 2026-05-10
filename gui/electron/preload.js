// Electron preload — bridges main → renderer with contextIsolation on.
//
// Only one piece of state crosses the bridge: the localhost URL of
// the Rust backend. The main process discovered the kernel-assigned
// port at startup and passed it via `webPreferences.additionalArguments`
// as `--ani-gui-api-base=http://127.0.0.1:<port>`. We parse it here
// and surface it as `window.aniGui.apiBase`.
//
// The frontend's `lib/api.ts > apiBase()` checks `window.aniGui.apiBase`
// first; once this value is present, all `fetch()` calls land on the
// Rust sidecar.

'use strict';

const { contextBridge, ipcRenderer } = require('electron');

const flag = '--ani-gui-api-base=';
const arg = process.argv.find((a) => a.startsWith(flag));
const apiBase = arg ? arg.slice(flag.length) : null;

if (!apiBase) {
	console.error('[preload] no --ani-gui-api-base flag — renderer will fail to reach the backend');
}

contextBridge.exposeInMainWorld('aniGui', {
	apiBase,

	// Native folder picker — opens an OS dialog, returns the chosen
	// absolute path or `null` on cancel. The renderer's download
	// confirmation modal calls this when the user clicks "Browse…".
	async pickDirectory(options) {
		return ipcRenderer.invoke('ani-gui:pick-directory', options || {});
	},

	// Native file picker — same shape as pickDirectory but for a
	// single file. The settings page uses this to let the user point
	// at an external-player executable that isn't on PATH. `options`
	// supports `{ title, defaultPath, filters }` where `filters` is
	// the Electron OpenDialog filter shape:
	//   [{ name: "Executables", extensions: ["exe"] }]
	async pickFile(options) {
		return ipcRenderer.invoke('ani-gui:pick-file', options || {});
	},

	// Open the OS file manager at the given directory. Used by the
	// download dock and the completion toast's "Reveal in folder"
	// button. Returns `true` on success, `false` if the path didn't
	// resolve.
	async revealInFolder(dirPath) {
		return ipcRenderer.invoke('ani-gui:reveal-in-folder', dirPath);
	}
});
