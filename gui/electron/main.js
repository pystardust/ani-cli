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

"use strict";

const {
  app,
  BrowserWindow,
  Menu,
  dialog,
  ipcMain,
  net,
  protocol,
  screen,
  shell,
} = require("electron");
const { spawn } = require("node:child_process");
const path = require("node:path");
const fs = require("node:fs");
const { pathToFileURL } = require("node:url");

const IS_DEV = process.env.ELECTRON_DEV === "1";
const VITE_DEV_URL = process.env.VITE_DEV_URL || "http://localhost:5173";

// Pin the X11 WM_CLASS / Wayland app_id so GNOME matches the running
// window to our `.desktop` entry's `StartupWMClass=ani-gui`. Without
// this, the dock falls back to a generic icon — the .desktop's
// `Icon=` line is only used in the app grid, not for live-window
// matching. Must be set before app.whenReady().
app.setName("ani-gui");
process.title = "ani-gui";

// Custom scheme used in packaged builds to serve the SvelteKit
// static bundle. Loading the index.html via plain `file://` works
// for the root document but breaks SvelteKit's chunk graph — the
// runtime does dynamic `import('/_app/foo.js')` against the page's
// origin, which under `file://` resolves to filesystem-root nonsense.
// A custom scheme gives us a real origin we control, so we can
// resolve `/_app/...` against the bundle dir and SPA-fallback any
// other path to index.html.
const APP_SCHEME = "app";
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
      corsEnabled: true,
    },
  },
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
  // Windows binaries carry the .exe suffix; the rest of the platforms
  // ship the bare binary name. Cargo writes the suffix automatically
  // when targeting a Windows triple, and electron-builder copies
  // whichever shape it finds.
  const exeSuffix = process.platform === "win32" ? ".exe" : "";
  const binaryName = `ani-gui-backend${exeSuffix}`;
  if (IS_DEV) {
    const repoRoot = path.resolve(__dirname, "..", "..");
    // Debug first in dev: cargo's debug profile is what `cargo
    // build` and `cargo test` produce by default, so it stays fresh
    // as the user iterates. The release binary at
    // `target/release/` only updates when something explicitly
    // runs `cargo build --release` and was a recurring footgun:
    // `pnpm dev` would silently pick up a stale release binary
    // from a packaging run hours earlier.
    const candidates = [
      path.join(repoRoot, "gui", "backend", "target", "debug", binaryName),
      path.join(repoRoot, "gui", "backend", "target", "release", binaryName),
    ];
    for (const p of candidates) {
      if (fs.existsSync(p)) return p;
    }
    throw new Error(
      `${binaryName} not found. Build it first:\n  ` +
        `cd gui/backend && cargo build --bin ani-gui-backend`,
    );
  }
  const packaged = path.join(process.resourcesPath, binaryName);
  if (!fs.existsSync(packaged)) {
    throw new Error(
      `${binaryName} not found in packaged resources at ${packaged}. ` +
        `The electron-builder \`extraResources\` rule may be misconfigured.`,
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
    // `detached: true` puts the backend in its own process group on
    // POSIX so we can kill the entire group (backend + ani-cli +
    // aria2c + ffmpeg) at quit time via `process.kill(-pid, …)`.
    // Without it, only the Rust process gets the signal and the
    // download grandchildren get reparented to init and keep
    // running. Windows has no process groups; the tree-kill path
    // shells out to taskkill /T instead — see killBackendTree().
    const child = spawn(bin, [], {
      stdio: ["ignore", "pipe", "pipe"],
      detached: process.platform !== "win32",
    });
    let buf = "";
    let resolved = false;
    // Set when the backend emits the ANI_GUI_FATAL bash_missing
    // signal — typically a Windows machine with no Git for Windows
    // install. Promoted to a class of error rather than a generic
    // "backend exited" message so app.whenReady can show a native
    // dialog with a download link instead of just logging.
    let fatalReason = null;

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

    child.stdout.on("data", (chunk) => {
      buf += chunk.toString("utf-8");
      let nl;
      while ((nl = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, nl);
        buf = buf.slice(nl + 1);
        onLine(line);
      }
    });
    child.stderr.on("data", (chunk) => {
      const text = chunk.toString("utf-8");
      process.stderr.write(`[backend] ${text}`);
      if (text.includes("ANI_GUI_FATAL bash_missing")) {
        fatalReason = "bash_missing";
      }
    });
    child.on("exit", (code, signal) => {
      if (!resolved) {
        const err = new Error(
          `backend exited before handshake (code=${code}, signal=${signal})`,
        );
        err.fatalReason = fatalReason;
        reject(err);
      } else {
        console.error(`[backend] exited (code=${code}, signal=${signal})`);
      }
    });
  });
}

// Surface a Windows-friendly "Install Git for Windows" dialog when
// the backend bails out at startup with the `bash_missing` signal.
// Blocks until the user picks a button; clicking the install option
// opens gitforwindows.org in the default browser. Either way the app
// then quits — there's no recovery without bash.
async function showBashMissingDialog() {
  const result = await dialog.showMessageBox({
    type: "error",
    title: "Git for Windows required",
    message: "ani-gui needs Git for Windows to run.",
    detail:
      "ani-gui drives the upstream ani-cli script via bash, which on Windows is " +
      "shipped as part of Git for Windows. Install it from gitforwindows.org and " +
      "relaunch ani-gui — it'll find bash automatically.\n\n" +
      "(If Git for Windows is already installed, make sure bash.exe is on PATH or " +
      "reachable at the standard install path.)",
    buttons: ["Open download page", "Quit"],
    defaultId: 0,
    cancelId: 1,
    noLink: true,
  });
  if (result.response === 0) {
    await shell.openExternal("https://gitforwindows.org/");
  }
}

let backendChild = null;

/**
 * Kill the backend AND every download grandchild it spawned
 * (ani-cli, aria2c, ffmpeg, yt-dlp). On POSIX we negate the pid to
 * signal the backend's process group — spawnBackend uses
 * `detached: true` so the cascade works. On Windows there are no
 * process groups, so we shell out to taskkill with /T (kill tree).
 *
 * Idempotent — safe to call when the backend has already exited.
 */
function killBackendTree() {
  if (!backendChild || backendChild.killed) return;
  if (process.platform === "win32") {
    // /F = force, /T = include child processes. Fire-and-forget;
    // we don't await it because the close path is already winding
    // down and a stuck taskkill shouldn't block the quit.
    try {
      spawn("taskkill", ["/F", "/T", "/PID", String(backendChild.pid)], {
        stdio: "ignore",
        windowsHide: true,
      });
    } catch (e) {
      console.error("[main] taskkill failed:", e);
    }
    return;
  }
  try {
    // Negative pid = process group. SIGTERM gives the children a
    // chance to clean up; if any survives, the OS reaper will
    // eventually SIGKILL on app shutdown.
    process.kill(-backendChild.pid, "SIGTERM");
  } catch (e) {
    // ESRCH: group already gone (backend exited first). Anything
    // else is unexpected and worth logging.
    if (e && e.code !== "ESRCH") console.error("[main] killBackendTree:", e);
  }
}

async function createWindow(apiBase) {
  // Pre-compute the work area so the window opens at the maximized
  // size in one shot. Setting the constructor width/height to the
  // work-area size avoids the "open at 1280×800, then animate to
  // maximized" flash some WMs (mutter, kwin) animate when you call
  // `win.maximize()` against a smaller initial size. After the
  // window is up we still call `maximize()` so the WM tracks the
  // maximized state — that lets the user un-maximize back to a
  // sensible default and lets a snap shortcut work.
  const { width: workW, height: workH } = screen.getPrimaryDisplay().workAreaSize;
  const win = new BrowserWindow({
    width: workW,
    height: workH,
    minWidth: 960,
    minHeight: 600,
    show: false,
    // Window icon for the dev session — packaged builds get it
    // from the .desktop file electron-builder generates from
    // build-resources/icon.png. Without this, GNOME's window list
    // shows the generic Electron logo while running unpackaged.
    icon: path.join(__dirname, "build-resources", "icon.png"),
    // Frame/decorations match the Tauri config (system decorations).
    webPreferences: {
      preload: path.join(__dirname, "preload.js"),
      contextIsolation: true,
      nodeIntegration: false,
      // Pass the resolved apiBase into the preload via additional
      // arguments — the preload reads them off process.argv.
      additionalArguments: [`--ani-gui-api-base=${apiBase}`],
    },
  });

  // Open external links (http/https) in the user's default browser
  // instead of inside the app window.
  win.webContents.setWindowOpenHandler(({ url }) => {
    if (url.startsWith("http")) {
      shell.openExternal(url);
      return { action: "deny" };
    }
    return { action: "allow" };
  });

  // Renderer-side diagnostics surface in the main process log so we
  // can tell whether the page actually loaded and whether scripts
  // are throwing. Without this, a blank window looks identical to a
  // successful load from the outside.
  win.webContents.on("did-fail-load", (_e, code, desc, url) => {
    console.error(`[renderer] did-fail-load ${url}: ${code} ${desc}`);
  });
  win.webContents.on("console-message", (_e, level, msg, line, source) => {
    const tag = ["log", "warning", "error"][level] || "log";
    console.log(`[renderer:${tag}] ${msg} (${source}:${line})`);
  });

  // "Open maximized" is a two-step belt-and-suspenders pattern.
  //
  // Step 1 — pre-show hint (electron/electron #45815, #834):
  // call `maximize()` synchronously while the window is still
  // hidden, so the underlying surface is created with the maximize
  // hint already set. Most launches end here — the WM honors the
  // hint at map time and the window appears maximized in one shot.
  //
  // Step 2 — post-show fallback: ~1 launch in 5 mutter drops the
  // pre-show hint under load and the window comes up at workArea
  // size but un-flagged. The `show` event fires deterministically
  // *after* the surface is mapped, so re-firing `maximize()` there
  // acts on a fully tracked window and is reliable. We only re-fire
  // if the WM didn't already pick up the hint, so there's no
  // animation on the happy path. Event-driven (NOT setTimeout) —
  // late timers race the WM and were the cause of five earlier
  // failed iterations.
  //
  // The post-show maximize triggers a brief WM-rendered animation
  // on the bad path. We attempted to hide it behind setOpacity(0)
  // → setOpacity(1), but the maximize animation is on the window
  // frame (compositor-rendered), which Electron cannot suppress.
  win.maximize();
  win.once("ready-to-show", () => {
    win.show();
  });
  win.once("show", () => {
    if (!win.isMaximized()) win.maximize();
  });

  // Catch the X-button / window-close path. The before-quit hook
  // below covers Cmd+Q / dock-quit / OS shutdown; both reuse the
  // same guard so wording stays consistent across exit paths.
  win.on("close", (e) => maybePromptOnClose(win, e));

  if (IS_DEV) {
    win.webContents.openDevTools({ mode: "detach" });
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
  const bundleDir = path.join(process.resourcesPath, "frontend", "build");
  protocol.handle(APP_SCHEME, async (request) => {
    const url = new URL(request.url);
    let pathname = decodeURIComponent(url.pathname);
    if (!pathname || pathname === "/") pathname = "/index.html";
    const target = path.normalize(path.join(bundleDir, pathname));
    if (!target.startsWith(bundleDir)) {
      return new Response("forbidden", { status: 403 });
    }
    try {
      await fs.promises.access(target);
      return net.fetch(pathToFileURL(target).toString());
    } catch {
      // SPA fallback — anything routerly (no file extension)
      // hands back the index so SvelteKit's client router takes
      // over. A real missing asset (image, css) returns 404.
      if (!path.extname(pathname)) {
        return net.fetch(
          pathToFileURL(path.join(bundleDir, "index.html")).toString(),
        );
      }
      return new Response("not found", { status: 404 });
    }
  });
}

// IPC handlers for the renderer's preload bridge. Exposed as
// window.aniGui.pickDirectory() / .pickFile() / .revealInFolder(path).
ipcMain.handle("ani-gui:pick-directory", async (_event, options) => {
  const result = await dialog.showOpenDialog({
    properties: ["openDirectory", "createDirectory"],
    title: options?.title || "Choose download folder",
    defaultPath: options?.defaultPath || undefined,
  });
  if (result.canceled || !result.filePaths?.[0]) return null;
  return result.filePaths[0];
});

// Native file picker — used by the settings page to let the user
// point at an external-player executable that isn't on PATH (typical
// on Windows where mpv.exe is often installed under
// `C:\Program Files\mpv\` outside the system PATH). Mirrors
// pick-directory: returns the absolute path or null on cancel.
ipcMain.handle("ani-gui:pick-file", async (_event, options) => {
  const filters = Array.isArray(options?.filters) ? options.filters : undefined;
  const result = await dialog.showOpenDialog({
    properties: ["openFile"],
    title: options?.title || "Choose file",
    defaultPath: options?.defaultPath || undefined,
    filters,
  });
  if (result.canceled || !result.filePaths?.[0]) return null;
  return result.filePaths[0];
});

ipcMain.handle("ani-gui:reveal-in-folder", async (_event, dirPath) => {
  if (typeof dirPath !== "string" || !dirPath) return false;
  // Use openPath for directories (showItemInFolder targets a file
  // and selects it; for our case the renderer always passes a
  // directory). Returns '' on success per Electron docs.
  const err = await shell.openPath(dirPath);
  return err === "";
});

// Latest active-download count pushed by the renderer (see preload's
// notifyActiveDownloads). Main reads this synchronously at close
// time to decide whether to prompt the user before quitting.
let activeDownloadCount = 0;
// Set true once the user has confirmed "Quit anyway" so the
// follow-up close (which we synthesise via win.close()) skips the
// prompt. Without this we'd recurse.
let confirmedQuit = false;

ipcMain.on("ani-gui:active-downloads", (_event, count) => {
  if (typeof count !== "number" || !Number.isFinite(count) || count < 0) return;
  activeDownloadCount = Math.floor(count);
});

/**
 * Close-path guard. Called from window 'close' and app 'before-quit';
 * intercepts both X-button and Cmd+Q / dock-quit / OS shutdown. Uses
 * showMessageBoxSync so the close-handler's preventDefault sticks —
 * the async variant returns after the close has already been
 * committed.
 */
function maybePromptOnClose(win, event) {
  if (confirmedQuit) return;
  if (activeDownloadCount <= 0) return;
  event.preventDefault();
  const plural = activeDownloadCount === 1 ? "" : "s";
  const choice = dialog.showMessageBoxSync(win, {
    type: "question",
    buttons: ["Cancel", "Quit anyway"],
    defaultId: 0,
    cancelId: 0,
    title: "Active downloads",
    message: `${activeDownloadCount} download${plural} in progress.`,
    detail: "They will be cancelled if you quit. Continue?",
  });
  if (choice === 1) {
    confirmedQuit = true;
    // Re-trigger the close path. The flag above makes this no-op
    // through the guard so the window actually closes this time.
    if (win && !win.isDestroyed()) win.close();
    else app.quit();
  }
}

app.whenReady().then(async () => {
  try {
    // Drop Electron's default app menu (File / Edit / View / Window /
    // Help) — the in-window topbar + rail are the navigation surface;
    // the platform menu was just adding a strip of system chrome the
    // app doesn't use.
    Menu.setApplicationMenu(null);
    if (!IS_DEV) registerAppProtocol();
    const { child, apiBase } = await spawnBackend();
    backendChild = child;
    await createWindow(apiBase);
  } catch (err) {
    console.error("[main] startup failed:", err);
    // Surface a friendly install dialog when the backend bailed out
    // because Git for Windows wasn't reachable. The bash_missing
    // signal flows up via spawnBackend's rejected error.
    if (err && err.fatalReason === "bash_missing") {
      try {
        await showBashMissingDialog();
      } catch (dialogErr) {
        console.error("[main] bash-missing dialog failed:", dialogErr);
      }
    }
    app.exit(1);
  }
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});

app.on("before-quit", (e) => {
  // Prompt on Cmd+Q / dock-quit / OS-shutdown if downloads are
  // active. The window-level `close` handler above covers the
  // X-button path; both reuse the same guard.
  const focused = BrowserWindow.getFocusedWindow();
  const win = focused || BrowserWindow.getAllWindows()[0];
  if (win) maybePromptOnClose(win, e);
  // Tree-kill so ani-cli + aria2c + ffmpeg actually stop. A bare
  // backendChild.kill() only signals the Rust process and orphans
  // the download grandchildren to init.
  killBackendTree();
});

// Re-create a window if the user clicks the dock icon on macOS while
// the app is still running.
app.on("activate", async () => {
  if (BrowserWindow.getAllWindows().length === 0 && backendChild) {
    // Re-derive apiBase from the running backend's known origin.
    // In practice we'd persist this from spawnBackend(); for now,
    // rely on the user to relaunch.
  }
});
