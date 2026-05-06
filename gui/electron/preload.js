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

const { contextBridge } = require('electron');

const flag = '--ani-gui-api-base=';
const arg = process.argv.find((a) => a.startsWith(flag));
const apiBase = arg ? arg.slice(flag.length) : null;

if (!apiBase) {
	console.error('[preload] no --ani-gui-api-base flag — renderer will fail to reach the backend');
}

contextBridge.exposeInMainWorld('aniGui', {
	apiBase
});
