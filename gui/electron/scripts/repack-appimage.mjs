// Post-build step: rebuild electron-builder's AppImage with two
// changes the published bundle wants but the upstream toolchain
// doesn't make easy:
//
//   1. Swap the bundled AppImage runtime (appimage-12.0.1, dlopens
//      libfuse.so.2) for the modern type2-runtime (links libfuse3,
//      which Ubuntu 24.04+ ships by default).
//   2. Patch the in-squashfs `AppRun` script to add `--no-sandbox`
//      to every Electron exec. AppImages mount their squashfs
//      read-only via FUSE, so `chrome-sandbox` inside it can't
//      carry the SUID 4755 bits Chromium's setuid sandbox demands;
//      Electron aborts at startup with a fatal "sandbox helper
//      not configured correctly" otherwise. The flag has to land
//      on argv at process spawn time — `app.commandLine.appendSwitch`
//      is too late, the check fires before main.js runs. The .deb
//      doesn't need this (its postinst sets the SUID bit on the
//      installed chrome-sandbox).
//
// We don't try to patch the runtime ELF or splice into the existing
// squashfs in place. Instead we extract the squashfs, edit AppRun,
// re-pack with mksquashfs, then concatenate the modern runtime
// header with the new squashfs payload. Same end shape as before:
//
//   new.AppImage = new-runtime-elf || mksquashfs(extract(old) + AppRun patch)

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createWriteStream } from 'node:fs';
import { pipeline } from 'node:stream/promises';
import { spawn } from 'node:child_process';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const electronDir = path.resolve(__dirname, '..');

// Continuous release of the official type2-runtime. Recent commits
// link libfuse3, which Ubuntu 24.04+ ships by default. Pinning to
// `continuous` keeps us current; for reproducible builds we can later
// swap to a tagged release commit.
const RUNTIME_URL =
	'https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-x86_64';
const cacheDir = path.join(electronDir, '.appimage-cache');
const runtimePath = path.join(cacheDir, 'runtime-x86_64');
const distDir = path.join(electronDir, 'dist');

/**
 * Download the modern runtime once, cache it under .appimage-cache/.
 * Subsequent runs are offline.
 */
async function ensureRuntime() {
	if (fs.existsSync(runtimePath) && fs.statSync(runtimePath).size > 0) return;
	fs.mkdirSync(cacheDir, { recursive: true });
	console.log(`[repack] downloading runtime: ${RUNTIME_URL}`);
	const resp = await fetch(RUNTIME_URL, { redirect: 'follow' });
	if (!resp.ok) throw new Error(`download failed: HTTP ${resp.status}`);
	await pipeline(resp.body, createWriteStream(runtimePath));
	fs.chmodSync(runtimePath, 0o755);
}

/**
 * Spawn a subprocess and resolve when it exits 0. Inherits stdio so
 * unsquashfs/mksquashfs progress and errors land in the build log.
 */
function run(cmd, args, opts = {}) {
	return new Promise((resolve, reject) => {
		const child = spawn(cmd, args, { stdio: 'inherit', ...opts });
		child.on('error', reject);
		child.on('exit', (code) => {
			if (code === 0) resolve();
			else reject(new Error(`${cmd} exited ${code}`));
		});
	});
}

/**
 * Patch the AppRun script in-place. electron-builder generates an
 * AppRun whose two exec lines are exactly:
 *
 *   exec "$BIN"
 *   exec "$BIN" "${args[@]}"
 *
 * Inject `--no-sandbox` between the binary and the user's args on
 * both branches. Idempotent — running twice is a no-op because the
 * source patterns are gone after the first pass.
 */
function patchAppRun(appRunPath) {
	const original = fs.readFileSync(appRunPath, 'utf8');
	const patched = original
		.replace(/^(\s*)exec "\$BIN"$/m, '$1exec "$BIN" --no-sandbox')
		.replace(/^(\s*)exec "\$BIN" "\$\{args\[@\]\}"$/m, '$1exec "$BIN" --no-sandbox "${args[@]}"');
	if (patched === original) {
		throw new Error(
			`AppRun patch matched nothing — has electron-builder's template changed? ${appRunPath}`
		);
	}
	fs.writeFileSync(appRunPath, patched, { mode: 0o755 });
}

/**
 * Repack one AppImage: extract via `--appimage-extract` (uses the
 * file's own runtime to read its squashfs), patch AppRun, mksquashfs
 * the result, then prepend the modern runtime header.
 */
async function repack(appimage) {
	const tmpRoot = path.join(distDir, '.repack-tmp-' + path.basename(appimage));
	if (fs.existsSync(tmpRoot)) fs.rmSync(tmpRoot, { recursive: true, force: true });
	fs.mkdirSync(tmpRoot, { recursive: true });

	// 1. Extract via the AppImage's own runtime. `--appimage-extract`
	//    drops a `squashfs-root/` next to cwd, so we cwd into our
	//    tmpRoot to keep the dist/ dir tidy.
	console.log(`[repack] extracting ${path.basename(appimage)}`);
	await run(appimage, ['--appimage-extract'], { cwd: tmpRoot });
	const appDir = path.join(tmpRoot, 'squashfs-root');

	// 2. Patch AppRun.
	const appRunPath = path.join(appDir, 'AppRun');
	if (!fs.existsSync(appRunPath)) {
		throw new Error(`expected AppRun at ${appRunPath} after extract`);
	}
	patchAppRun(appRunPath);
	console.log(`[repack] patched AppRun: --no-sandbox added to exec lines`);

	// 3. Repack with mksquashfs. type2-runtime only links zlib + zstd
	//    decompressors; xz (mksquashfs' default) errors out at mount
	//    time with "uses xz compression, this version supports only
	//    zlib, zstd." zstd matches what AppImageKit ships these days
	//    and is denser than zlib at default level. -no-xattrs +
	//    -all-root mirror what electron-builder's AppImage tooling
	//    uses; without `-all-root` mksquashfs preserves the build-
	//    host's uid which can't be read on the user's machine.
	const squashfsFile = path.join(tmpRoot, 'payload.squashfs');
	console.log(`[repack] mksquashfs → ${path.basename(squashfsFile)}`);
	await run('mksquashfs', [
		appDir,
		squashfsFile,
		'-comp',
		'zstd',
		'-no-xattrs',
		'-all-root',
		'-noappend',
		'-quiet'
	]);

	// 4. Final AppImage = modern runtime || new squashfs.
	const tmpFinal = appimage + '.repack-tmp';
	const out = fs.createWriteStream(tmpFinal);
	out.write(fs.readFileSync(runtimePath));
	const reader = fs.createReadStream(squashfsFile);
	await pipeline(reader, out);
	fs.chmodSync(tmpFinal, 0o755);
	fs.renameSync(tmpFinal, appimage);

	// 5. Cleanup the extract scratch.
	fs.rmSync(tmpRoot, { recursive: true, force: true });

	const finalSize = fs.statSync(appimage).size;
	console.log(`[repack] ${path.basename(appimage)} ready (${finalSize} bytes)`);
}

async function main() {
	if (!fs.existsSync(distDir)) {
		throw new Error(`dist/ not found — run electron-builder first`);
	}
	await ensureRuntime();
	// Only consider top-level regular files. Filter out our own
	// `.repack-tmp-…` scratch directories (they're hidden and end with
	// `.AppImage` — a previous run that bailed mid-extract leaves one
	// behind, and the next run would otherwise try to spawn the
	// directory as an executable and fail with EACCES).
	const candidates = fs
		.readdirSync(distDir, { withFileTypes: true })
		.filter((d) => d.isFile() && !d.name.startsWith('.') && d.name.endsWith('.AppImage'))
		.map((d) => path.join(distDir, d.name));
	if (candidates.length === 0) {
		throw new Error(`no .AppImage files in ${distDir}`);
	}
	for (const c of candidates) await repack(c);
	console.log(
		`[repack] done — ${candidates.length} AppImage(s) repacked with type2-runtime + AppRun --no-sandbox patch`
	);
}

main().catch((err) => {
	console.error('[repack] failed:', err.message);
	process.exit(1);
});
